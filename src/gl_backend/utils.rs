use std::ffi::CString;
use gl::types::*;
use super::utils;

pub fn create_empty_cstring(len: usize) -> CString {
    let mut buffer: Vec<u8> = Vec::with_capacity(len + 1);

    buffer.extend([b' '].iter().cycle().take(len));
    unsafe { CString::from_vec_unchecked(buffer) }
}

pub fn get_gl_error(id: GLuint) -> String {
    let mut len: GLint = 0;
    unsafe {
        gl::GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut len);
    }

    let error = utils::create_empty_cstring(len as usize);

    unsafe {
        gl::GetShaderInfoLog(id, len, std::ptr::null_mut(), error.as_ptr() as *mut GLchar);
    }

    error.to_string_lossy().into_owned()
}
