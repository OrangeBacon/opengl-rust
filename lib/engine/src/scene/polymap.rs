use std::{
    any::Any,
    borrow::Borrow,
    collections::{hash_map::RandomState, HashMap},
    hash::{BuildHasher, Hash},
};

#[derive(Debug, Default)]
pub struct PolyMap<K, S = RandomState>
where
    K: Eq + Hash + Send,
    S: BuildHasher + Send,
{
    map: HashMap<K, Box<dyn Any + Send>, S>,
}

impl<K> PolyMap<K, RandomState>
where
    K: Eq + Hash + Send,
{
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn with_capacity(n: usize) -> Self {
        Self {
            map: HashMap::with_capacity(n),
        }
    }
}

impl<K, S> PolyMap<K, S>
where
    K: Eq + Hash + Send,
    S: BuildHasher + Send,
{
    pub fn with_hasher(hash_builder: S) -> Self {
        Self {
            map: HashMap::with_hasher(hash_builder),
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash + Send + ?Sized,
    {
        self.map.contains_key(key)
    }

    pub fn contains_key_of<Q, T>(&self, key: &Q) -> bool
    where
        T: Any + Send,
        K: Borrow<Q>,
        Q: Eq + Hash + Send + ?Sized,
    {
        self.map.get(key).map_or(false, |v| v.is::<T>())
    }

    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }

    pub fn get<Q, T>(&self, k: &Q) -> Option<&T>
    where
        T: Any + Send,
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        if let Some(v) = self.map.get(k) {
            if let Some(v) = v.downcast_ref() {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_mut<Q, T>(&mut self, k: &Q) -> Option<&mut T>
    where
        T: Any + Send,
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        if let Some(v) = self.map.get_mut(k) {
            if let Some(v) = v.downcast_mut() {
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn insert<T>(&mut self, key: K, t: T) -> Option<T>
    where
        T: Any + Send,
    {
        let old = self.map.insert(key, Box::new(t));

        if let Some(value) = old {
            if let Ok(value) = value.downcast() {
                Some(*value)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn remove<Q, T>(&mut self, key: &Q) -> Option<T>
    where
        T: Any + Send,
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        let v = self.map.remove(key);

        if let Some(v) = v {
            if let Ok(v) = v.downcast() {
                Some(*v)
            } else {
                panic!("remove value of different type");
            }
        } else {
            None
        }
    }
}
