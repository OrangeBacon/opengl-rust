use std::{
    fmt::Debug,
    sync::{
        atomic::{AtomicU32, Ordering::SeqCst},
        Arc, Mutex, MutexGuard,
    },
};
use thiserror::Error;

/// The various types of error that can be emitted from the triple buffer
/// Todo: make the errors more descriptive
#[derive(Debug, Error)]
pub enum TripleError {
    #[error("Triple buffer mutex poisoned: {name}")]
    PoisonError { name: &'static str },

    #[error("Unable to aquire triple read state")]
    Read,

    #[error("Triple read guard lock dropped")]
    ReadLock,

    #[error("Triple read update lock error")]
    ReadUpdate,

    #[error("Unable to aquire triple write state")]
    Write,

    #[error("Triple write guard lock dropped")]
    WriteLock,

    #[error("Triple write update lock error")]
    WriteUpdate,
}

/// The internal configuration of the triple buffer
/// Each element in the struct is an index into the triple buffer's state
/// array.  As the array has two elements, the index can only be 0, 1, 2.
/// Using a two bit data type to store the indicies, this leaves the value 3
/// to reperesent invalid/None where required.
///
/// Five indicies into the triple buffer's state are used, so with 2 bits per
/// index, a total of 10 bits are required. AtomicU32 is used as 16 bit atomics
/// are significantly less efficient that 32 bit.  (See [`performance`])
///
/// The read and write indicies could be removed, however it seems like a
/// cleaner solution to use more of the atomic value, rather than calling
/// try_lock on muticies
///
/// [`performance`] https://stackoverflow.com/questions/29322218/performance-comparison-of-atomic-operations-on-different-sizes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TripleConfig {
    /// The index of the most recently written state
    idx1: u32,

    /// The index of the second most recently written state
    idx2: u32,

    /// The index of the oldest state
    idx3: u32,

    /// The index of the state that is currently locked for reading.  If no
    /// state is locked, contains 3
    read: u32,

    /// The index of the state that is currently locked for writing.  If no
    /// state is locked, contains 3
    write: u32,
}

impl TripleConfig {
    /// Get the value of index 1 from a 32 bit config value
    fn idx1(x: u32) -> u32 {
        (x & 0b00000000_11) >> 0
    }

    /// Get the value of index 2 from a 32 bit config value
    fn idx2(x: u32) -> u32 {
        (x & 0b000000_1100) >> 2
    }

    /// Get the value of index 3 from a 32 bit config value
    fn idx3(x: u32) -> u32 {
        (x & 0b0000_110000) >> 4
    }

    // The mask used when accessing the read index
    const READ_MASK: u32 = 0b00_11000000;

    /// Get the current read lock index from a 32 bit config value
    fn read(x: u32) -> u32 {
        (x & Self::READ_MASK) >> 6
    }

    /// Get the current rwrite lock index from a 32 bit config value
    fn write(x: u32) -> u32 {
        (x & 0b_1100000000) >> 8
    }

    /// Atomically try to get the read lock.
    /// Takes the reference to the config to atomically update
    /// Returns the index of the state that was acquired.
    /// Tries to get the states in order of most recently updated to least
    /// recently.
    /// Returns an error if the read lock is already acquired or if it is
    /// unable to find an unlocked state.
    fn acquire_read(config: &AtomicU32) -> Result<usize, TripleError> {
        // The variable storing the aquired read lock index
        // The initial value is an invalid index, on all paths that this
        // value is not set in the fetch_update, an error should be returned
        let mut read_lock = 3;

        config
            .fetch_update(SeqCst, SeqCst, |x| {
                let mut cfg: TripleConfig = x.into();

                // one state already has a read lock, so don't try to acquire
                // another lock
                if cfg.read != 3 {
                    return None;
                }

                // try to get the most recently written, unlocked state
                let read = if cfg.write != cfg.idx1 {
                    cfg.idx1
                } else if cfg.write != cfg.idx2 {
                    cfg.idx2
                } else if cfg.write != cfg.idx3 {
                    cfg.idx3
                } else {
                    // should not get here, but handle it anyway
                    return None;
                };

                // set the config to have the state chosen in the read lock
                // index
                cfg.read = read;

                // set the variable that says which lock was aquired to return
                // from this aquire function
                read_lock = read;

                // convert the state back into a single number to be stored
                Some(cfg.into())
            })
            .map_err(|_| TripleError::ReadUpdate)?;

        // read the aquired lock
        Ok(read_lock as usize)
    }

    /// Atomically try to get the write lock
    /// Takes the reference to the config to atomically update
    /// Returns the index of the state that was acquired.
    /// Tries to get the states in order of least recently updated to most
    /// recently.
    /// Returns an error if the write lock is already acquired or if it is
    /// unable to find an unlocked state.
    fn aquire_write(config: &AtomicU32) -> Result<usize, TripleError> {
        // The variable storing the aquired write lock index
        // The initial value is an invalid index, on all paths that this
        // value is not set in the fetch_update, an error should be returned
        let mut write_lock = 3;

        config
            .fetch_update(SeqCst, SeqCst, |x| {
                let mut cfg: TripleConfig = x.into();

                // returns if the write lock has already been acquired, so it
                // does not aquire multiple write locks
                if cfg.write != 3 {
                    return None;
                }

                // try to get the least recently updated state
                let write = if cfg.read != cfg.idx3 {
                    cfg.idx3
                } else if cfg.read != cfg.idx2 {
                    cfg.idx2
                } else if cfg.read != cfg.idx1 {
                    cfg.idx1
                } else {
                    // should not be possible to get here, but handle it anyway
                    return None;
                };

                // update the new config
                cfg.write = write;

                // set the return value
                write_lock = write;

                // convert the config back into a single number
                Some(cfg.into())
            })
            .map_err(|_| TripleError::WriteUpdate)?;

        // return the index of the aquired lock
        Ok(write_lock as usize)
    }

    /// Remove the read lock index
    /// Safety: Before calling this function the mutex for the state being
    /// read must be released so that nothing blocks on acquiring the mutex
    fn clear_read(config: &AtomicU32) {
        // bitwise or with the read mask, that sets the current read lock
        // to the value of the mask 0b11, which is 3, so represents no
        // read lock is present
        config.fetch_or(Self::READ_MASK, SeqCst);
    }

    /// Remove the write lock index.
    /// Updates idx1/idx2/idx3 so that the new state is set as the most
    /// recently updated
    /// Safety: Before calling this function the mutex for the state being
    /// written must be released so that nothing blocks on acquiring the mutex
    fn clear_write(config: &AtomicU32, write_buffer_idx: usize) {
        // the index of the buffer that was written to
        let write_buffer_idx = write_buffer_idx as u32;
        config
            .fetch_update(SeqCst, SeqCst, |x| {
                // decompose the current config
                let mut cfg: TripleConfig = x.into();

                // the list of state indicies in most recent to least recently
                // updated, ignoring the current update
                let states = [cfg.idx1, cfg.idx2, cfg.idx3];

                // remove the state that was most recently updates
                // so it can be re-inserted at the start
                let states: Vec<_> = states.iter().filter(|x| **x != write_buffer_idx).collect();

                // set the config to have the newly updated state first, while
                // maintaining the order of the other two states
                cfg.idx1 = write_buffer_idx;
                cfg.idx2 = *states[0];
                cfg.idx3 = *states[1];

                // set that no write lock is currently held
                cfg.write = 3;

                Some(cfg.into())
            })
            // fetch update should only return an error if the function
            // provided returns None, which cannot happen
            .expect("Clear write unreachable failure");
    }
}

impl Into<u32> for TripleConfig {
    /// combine the parts of a configuration back into a single number
    fn into(self) -> u32 {
        // the shift indicies here should be the same as in the construction
        // methods (TripleConfig::idx1, etc)
        let idx1 = (self.idx1 as u32) << 0;
        let idx2 = (self.idx2 as u32) << 2;
        let idx3 = (self.idx3 as u32) << 4;
        let read = (self.read as u32) << 6;
        let writ = (self.write as u32) << 8;

        idx1 | idx2 | idx3 | read | writ
    }
}

impl Into<TripleConfig> for u32 {
    /// deconstruct a u32 into its component indicies
    fn into(self) -> TripleConfig {
        TripleConfig {
            idx1: TripleConfig::idx1(self),
            idx2: TripleConfig::idx2(self),
            idx3: TripleConfig::idx3(self),
            read: TripleConfig::read(self),
            write: TripleConfig::write(self),
        }
    }
}

impl Default for TripleConfig {
    /// Construct a default TripleConfig
    /// No read or write locks are held, the indicies are just in a default
    /// order, it would not be valid for all of the indicies to be 0 as
    /// that would lead to only state 0 being used, so significant waiting
    /// on muticies would occur
    fn default() -> Self {
        TripleConfig {
            idx1: 0,
            idx2: 1,
            idx3: 2,
            read: 3,
            write: 3,
        }
    }
}

/// A triple buffer. Allows reading from one thread and writing from another,
/// where read reads the most recently written state, write writes to the oldest
/// one.  Three states are used to stop the read thread from holding onto a
/// buffer to long causing the update thread to block. The same logic applies
/// the same other way round.
///
/// T: the type of the data stored inside the buffer, must be able to be
/// sent between threads, as muticies are used sync is not required.
#[derive(Debug)]
pub struct TripleBuffer<T: Send> {
    /// The data held in the buffer, stored in no particular order.
    /// Always use the indicies stored in the config to access this array.
    states: [Mutex<T>; 3],

    /// The configuration data.
    /// Its format is described in TripleConfig, however it is stored as a
    /// single atomic value so that the configuration is consistent between
    /// the read and write threads.
    config: AtomicU32,
}

impl<'a, T: Send> TripleBuffer<T> {
    /// Construct a new triple buffer using the default values of the
    /// contained type.
    pub fn new() -> (TripleBufferReader<T>, TripleBufferWriter<T>)
    where
        T: Default,
    {
        Self::from_states(Default::default(), Default::default(), Default::default())
    }

    /// Construct a new triple buffer where the initial states are all cloned
    /// from the passed in data.
    pub fn new_clone(data: &T) -> (TripleBufferReader<T>, TripleBufferWriter<T>)
    where
        T: Clone,
    {
        Self::from_states(data.clone(), data.clone(), data.clone())
    }

    /// Construct a new triple buffer where the initial states are all
    /// copies of the passed in data.
    pub fn new_copy(data: T) -> (TripleBufferReader<T>, TripleBufferWriter<T>)
    where
        T: Copy,
    {
        Self::from_states(data, data, data)
    }

    /// Construct a new triple buffer from three different initial states.
    /// There is no guarantee as to which order the states passed
    /// will be used in, so it is recomended that the states are all
    /// identical
    pub fn from_states(s1: T, s2: T, s3: T) -> (TripleBufferReader<T>, TripleBufferWriter<T>) {
        // construct the actual buffer
        let state = Self {
            states: [Mutex::new(s1), Mutex::new(s2), Mutex::new(s3)],
            config: AtomicU32::new(TripleConfig::default().into()),
        };

        let state = Arc::new(state);

        // create a reader that can be passed to the read thread
        let read = TripleBufferReader {
            inner: Arc::clone(&state),
        };

        // create a writer that can be passed to the write thread
        let write = TripleBufferWriter {
            inner: Arc::clone(&state),
        };

        // only return the reader and writer, so that the different threads
        // will not be able to perform the wrong operation
        // the buffer will be dropped when both the reader and the writer
        // are dropped

        (read, write)
    }
}

/// RAII wrapper around a triple buffer that only allows aquiring read access
pub struct TripleBufferReader<T: Send> {
    inner: Arc<TripleBuffer<T>>,
}

impl<T: Send> TripleBufferReader<T> {
    /// Try to get the most recently written state for reading
    /// This function will fail if another state is still open for reading,
    /// the previous one should be dropped first
    pub fn get_read<'a>(&'a self) -> Result<TripleBufferReadGuard<'a, T>, TripleError> {
        let read = TripleConfig::acquire_read(&self.inner.config)?;

        let state = self.inner.states[read]
            .try_lock()
            .map_err(|_| TripleError::PoisonError {
                name: "GetReadLock",
            })?;

        Ok(TripleBufferReadGuard {
            source: &self.inner,
            inner: Some(state),
        })
    }
}

/// RAII wrapper around a single read state
/// When dropped returns the read state to the triple buffer
/// Is mainly a wrapper around a MutexGuard
pub struct TripleBufferReadGuard<'a, T: Send> {
    /// the triple buffer that this struct was allocated from
    source: &'a TripleBuffer<T>,

    /// The actual state that is stored.
    /// It is wrapped in an option as in the destructor, the lock needs to be
    /// released before the updates can be applied, so that the configuration
    /// does not say a state can be accessed when it is still locked. The option
    /// allows dropping the lock using drop(option.take()), before the
    /// config (accessed through self.source) is modified.  As an option is
    /// used, this means that deref could fail, which is why the struct uses
    /// a failable state method, rather than implementing Deref.
    ///
    /// An alternative could be to use MaybeUninit, however that would require
    /// introducing unsafe rust into the buffer, so it is not done, even though
    /// it would have better usability.
    inner: Option<MutexGuard<'a, T>>,
}

impl<'a, T: Send> TripleBufferReadGuard<'a, T> {
    /// Try to get a reference to the data stored.
    /// Should only fail after this struct is dropped, so if it does fail
    /// then it is weird.
    pub fn state(&self) -> Result<&T, TripleError> {
        Ok(self.inner.as_deref().ok_or(TripleError::ReadLock)?)
    }
}

impl<'a, T: Send> Drop for TripleBufferReadGuard<'a, T> {
    /// Return the state to the triple buffer
    fn drop(&mut self) {
        // remove the lock on the state
        drop(self.inner.take());

        // update the config
        TripleConfig::clear_read(&self.source.config);
    }
}

/// RAII wrapper allowing getting write access to the triple buffer.
pub struct TripleBufferWriter<T: Send> {
    /// The triple buffer the states are aquired from.
    inner: Arc<TripleBuffer<T>>,
}

impl<T: Send> TripleBufferWriter<T> {
    /// Get a new state that can be written to
    /// Should be called each time a write state is required.  Only one write
    /// access is valid at a time, if another write is open then this call
    /// will fail.
    pub fn get_write<'a>(&'a self) -> Result<TripleBufferWriteGuard<'a, T>, TripleError> {
        let write = TripleConfig::aquire_write(&self.inner.config)?;

        let state = self.inner.states[write]
            .try_lock()
            .map_err(|_| TripleError::PoisonError {
                name: "GetWriteLock",
            })?;

        Ok(TripleBufferWriteGuard {
            write_buffer_idx: write,
            source: &self.inner,
            inner: Some(state),
        })
    }
}

/// RAII wrapper around a writable triple buffer state.
/// This wrapper has the same drawbacks as in TripleBufferReadGuard, see its
/// documentation for more infomation.
pub struct TripleBufferWriteGuard<'a, T: Send> {
    /// The triple buffer this state is allocated in.
    source: &'a TripleBuffer<T>,

    /// The index into the buffer's state array that this buffer holds
    write_buffer_idx: usize,

    /// The data stored
    inner: Option<MutexGuard<'a, T>>,
}

impl<'a, T: Send> TripleBufferWriteGuard<'a, T> {
    /// try to get a reference to the stored state
    pub fn state(&self) -> Result<&T, TripleError> {
        Ok(self.inner.as_deref().ok_or(TripleError::WriteLock)?)
    }

    /// try to get a mutable reference to the stored state
    pub fn state_mut(&mut self) -> Result<&mut T, TripleError> {
        Ok(self.inner.as_deref_mut().ok_or(TripleError::WriteLock)?)
    }
}

impl<'a, T: Send> Drop for TripleBufferWriteGuard<'a, T> {
    /// release the write state and set it as the most recently written to
    fn drop(&mut self) {
        // the lock needs to be dropped before updating the config
        drop(self.inner.take());

        // update the config to say that the lock was released
        TripleConfig::clear_write(&self.source.config, self.write_buffer_idx);
    }
}

#[cfg(test)]
mod triple_buffer {
    use anyhow::Result;
    use std::{thread::JoinHandle, time::Duration};

    use super::{TripleBuffer, TripleConfig};

    #[test]
    fn test_bitwise() {
        // test conversions to and from the config using the bitwise conversions
        let first = TripleConfig::default();
        let first_num: u32 = first.into();
        let second: TripleConfig = first_num.into();

        assert_ne!(first_num, 0);
        assert_eq!(first, second);
    }

    #[test]
    fn test_read_write() -> Result<()> {
        // simple test of using the triple buffer
        let (reader, writer) = TripleBuffer::<u32>::new();

        // create an update thread
        let t: JoinHandle<Result<()>> = std::thread::spawn(move || {
            let mut state = 0;

            loop {
                let mut write = writer.get_write()?;
                let write = write.state_mut()?;

                if state > 50_000 {
                    break;
                }

                state += 1;
                *write = state;

                std::thread::sleep(Duration::from_nanos(0));
            }

            Ok(())
        });

        let mut last_read;

        // loop through reading the state
        loop {
            let read = reader.get_read()?;
            let read = *read.state()?;

            last_read = read;

            if read >= 50_000 {
                break;
            }

            std::thread::sleep(Duration::from_nanos(0));
        }

        // ensure that the thread has exited, it should have for the loop to
        // have left, but is better to make sure
        t.join().expect("Unable to join thread")?;

        // read should only have broken out of the loop if this is true
        // most bugs in the buffer would be likely to cause this test to
        // infinite loop
        assert_eq!(last_read, 50_000);

        Ok(())
    }
}
