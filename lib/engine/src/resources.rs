use std::{
    ffi, fs,
    io::{self, Read},
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("File system error")]
    Io(#[from] io::Error),

    #[error("File loaded contains null byte")]
    FileContainsNil,

    #[error("Failed to load current executable path")]
    FailedToGetExePath,
}

pub struct Resources {
    root_path: PathBuf,
}

impl Resources {
    pub fn from_exe_path(rel_path: &Path) -> Result<Resources, Error> {
        let file_name = std::env::current_exe().map_err(|_| Error::FailedToGetExePath)?;

        let exe_path = file_name.parent().ok_or(Error::FailedToGetExePath)?;

        Ok(Resources {
            root_path: exe_path.join(rel_path),
        })
    }

    pub fn extend(&self, name: &Path) -> Self {
        let mut path = self.root_path.clone();
        path.extend(name);

        Resources { root_path: path }
    }

    pub fn load_cstring(&self, resource_name: &str) -> Result<ffi::CString, Error> {
        let mut file = fs::File::open(resource_name_to_path(&self.root_path, resource_name))?;

        let mut buffer: Vec<u8> = Vec::with_capacity(file.metadata()?.len() as usize + 1);
        file.read_to_end(&mut buffer)?;

        if buffer.iter().any(|i| *i == 0) {
            return Err(Error::FileContainsNil);
        }

        // unchecked only checks that there are no null ('\0') bytes in the
        // buffer, which is checked above
        Ok(unsafe { ffi::CString::from_vec_unchecked(buffer) })
    }

    pub fn load_bytes(&self, resource_name: &str) -> Result<Vec<u8>, Error> {
        let mut file = fs::File::open(resource_name_to_path(&self.root_path, resource_name))?;

        let mut buffer: Vec<u8> = Vec::with_capacity(file.metadata()?.len() as usize + 1);
        file.read_to_end(&mut buffer)?;

        Ok(buffer)
    }

    pub fn load_string(&self, resource_name: &str) -> Result<String, Error> {
        let file = fs::read_to_string(resource_name_to_path(&self.root_path, resource_name))?;

        Ok(file)
    }
}

// normalise path differences between windows and linux
fn resource_name_to_path(root_dir: &Path, location: &str) -> PathBuf {
    let mut path: PathBuf = root_dir.into();

    for part in location.split('/') {
        path = path.join(part);
    }

    path
}
