use super::utils;
use gl::types::*;
use std::{
    ffi::CString,
    error::Error,
    fmt,
};

pub fn create_empty_cstring(len: usize) -> CString {
    let mut buffer: Vec<u8> = Vec::with_capacity(len + 1);

    buffer.extend([b' '].iter().cycle().take(len));
    unsafe { CString::from_vec_unchecked(buffer) }
}

#[derive(Debug)]
pub struct GLError(String);
impl Error for GLError {}

impl fmt::Display for GLError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "GLError: {}", self.0)
    }
}

pub fn get_gl_error(id: GLuint) -> GLError {
    let mut len: GLint = 0;
    unsafe {
        gl::GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut len);
    }

    let error = utils::create_empty_cstring(len as usize);

    unsafe {
        gl::GetShaderInfoLog(id, len, std::ptr::null_mut(), error.as_ptr() as *mut GLchar);
    }

   GLError(error.to_string_lossy().into_owned())
}
