use gl::types::*;
use std::ffi::CStr;

use super::utils;

pub struct Shader {
    id: gl::types::GLuint,
}

impl Shader {
    fn from_source(source: &CStr, kind: GLenum) -> Result<Shader, utils::GLError> {
        let id = shader_from_source(source, kind)?;
        Ok(Shader { id })
    }

    pub fn from_vert(source: &CStr) -> Result<Shader, utils::GLError> {
        Shader::from_source(source, gl::VERTEX_SHADER)
    }

    pub fn from_frag(source: &CStr) -> Result<Shader, utils::GLError> {
        Shader::from_source(source, gl::FRAGMENT_SHADER)
    }

    pub fn id(&self) -> GLuint {
        self.id
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteShader(self.id);
        }
    }
}

fn shader_from_source(source: &CStr, kind: GLuint) -> Result<GLuint, utils::GLError> {
    let id = unsafe { gl::CreateShader(kind) };

    unsafe {
        gl::ShaderSource(id, 1, &source.as_ptr(), std::ptr::null());
        gl::CompileShader(id);
    }

    let mut success: GLint = 1;
    unsafe {
        gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut success);
    }

    if success == 0 {
        return Err(utils::get_gl_error(id));
    }

    Ok(id)
}
