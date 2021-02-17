use std::{
    ffi, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    string::FromUtf8Error,
};
use thiserror::Error;

use crate::data_uri::{self, DataURI};

#[derive(Error, Debug)]
pub enum Error {
    #[error("File system error: {0}")]
    Io(#[from] io::Error),

    #[error("Error encoding utf8: {0}")]
    UTF8(#[from] FromUtf8Error),

    #[error("Data Uri error: {0}")]
    DataUri(#[from] data_uri::Error),

    #[error("File loaded contains null byte")]
    FileContainsNil,

    #[error("Failed to load current executable path")]
    FailedToGetExePath,

    #[error("Failed to get path's parent")]
    Parent,
}

pub struct Resources {
    root_path: PathBuf,
}

impl Resources {
    pub fn new(path: PathBuf) -> Resources {
        Resources { root_path: path }
    }

    pub fn from_path<T: AsRef<Path>>(path: T) -> Self {
        Resources {
            root_path: path.as_ref().to_path_buf(),
        }
    }

    pub fn from_exe_path<T: AsRef<Path>>(rel_path: T) -> Result<Resources, Error> {
        let file_name = std::env::current_exe().map_err(|_| Error::FailedToGetExePath)?;

        let exe_path = file_name.parent().ok_or(Error::FailedToGetExePath)?;

        Ok(Resources {
            root_path: exe_path.join(rel_path),
        })
    }

    pub fn extend_file_root(&self, path: &str) -> Option<Self> {
        let path = resource_name_to_path(&self.root_path, path);
        let path = path.parent()?;
        Some(Resources {
            root_path: path.to_path_buf(),
        })
    }

    pub fn load_cstring(&self, resource_name: &str) -> Result<ffi::CString, Error> {
        let buffer = self.load_bytes(resource_name)?;

        if buffer.iter().any(|i| *i == 0) {
            return Err(Error::FileContainsNil);
        }

        // unchecked only checks that there are no null ('\0') bytes in the
        // buffer, which is checked above
        Ok(unsafe { ffi::CString::from_vec_unchecked(buffer) })
    }

    pub fn load_bytes(&self, resource_name: &str) -> Result<Vec<u8>, Error> {
        let buffer = if DataURI::is_data_uri(resource_name) {
            let uri = DataURI::new(resource_name)?;
            uri.get_data()?
        } else {
            let mut file = fs::File::open(resource_name_to_path(&self.root_path, resource_name))?;

            let mut buffer: Vec<u8> = Vec::with_capacity(file.metadata()?.len() as usize + 1);
            file.read_to_end(&mut buffer)?;

            buffer
        };

        Ok(buffer)
    }

    pub fn load_string(&self, resource_name: &str) -> Result<String, Error> {
        if DataURI::is_data_uri(resource_name) {
            let uri = DataURI::new(resource_name)?;
            return Ok(String::from_utf8(uri.get_data()?)?);
        }

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
