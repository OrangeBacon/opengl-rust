use crate::resources::{Error as ResourceError, Resources};
use gl::types::*;
use std::{
    ffi::{CStr, CString},
    ptr,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error loading {name}: {inner}")]
    ResourceLoad {
        name: String,
        #[source]
        inner: ResourceError,
    },

    #[error("Unable to infer shader type for {name}")]
    NoShaderType { name: String },

    #[error("Shader compilation error in {name}: \n{message}")]
    CompileError { name: String, message: String },

    #[error("Shader link error in {name}: \n{message}")]
    LinkError { name: String, message: String },
}

pub struct Program {
    id: GLuint,
}

impl Program {
    pub fn from_res(res: &Resources, name: &str) -> Result<Program, Error> {
        const POSSIBLE_EXT: [&str; 2] = [".vert", ".frag"];

        let shaders = POSSIBLE_EXT
            .iter()
            .map(|file_extension| Shader::from_res(res, &format!("{}{}", name, file_extension)))
            .collect::<Result<Vec<_>, _>>()?;

        Program::from_shaders(&shaders).map_err(|e| Error::LinkError {
            name: name.to_string(),
            message: e,
        })
    }

    pub fn from_shaders(shaders: &[Shader]) -> Result<Program, String> {
        let program_id = unsafe { gl::CreateProgram() };

        for shader in shaders {
            unsafe { gl::AttachShader(program_id, shader.id()) }
        }

        unsafe { gl::LinkProgram(program_id) };

        let mut success = 1;
        unsafe {
            gl::GetProgramiv(program_id, gl::LINK_STATUS, &mut success);
        }

        if success == 0 {
            let mut len = 0;
            unsafe {
                gl::GetProgramiv(program_id, gl::INFO_LOG_LENGTH, &mut len);
            }

            let error = create_whitespace_cstring_with_len(len as usize);

            unsafe {
                gl::GetProgramInfoLog(
                    program_id,
                    len,
                    ptr::null_mut(),
                    error.as_ptr() as *mut GLchar,
                )
            }

            return Err(error.to_string_lossy().into_owned());
        }

        for shader in shaders {
            unsafe { gl::DetachShader(program_id, shader.id()) }
        }

        Ok(Program {
            id: program_id,
        })
    }

    pub fn set_used(&self) {
        unsafe {
            gl::UseProgram(self.id);
        }
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.id);
        }
    }
}

fn shader_from_source(source: &CStr, kind: GLuint) -> Result<GLuint, String> {
    let id = unsafe { gl::CreateShader(kind) };

    unsafe {
        gl::ShaderSource(id, 1, &source.as_ptr(), ptr::null());
        gl::CompileShader(id);
    }

    let mut success: GLint = 1;
    unsafe {
        gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut success);
    }

    if success == 0 {
        let mut len = 0;
        unsafe {
            gl::GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut len);
        }

        let error = create_whitespace_cstring_with_len(len as usize);

        unsafe {
            gl::GetShaderInfoLog(id, len, ptr::null_mut(), error.as_ptr() as *mut GLchar);
        }

        return Err(error.to_string_lossy().into_owned());
    }

    Ok(id)
}

fn create_whitespace_cstring_with_len(len: usize) -> CString {
    let mut buffer: Vec<u8> = Vec::with_capacity(len + 1);
    buffer.extend([b' '].iter().cycle().take(len));
    unsafe { CString::from_vec_unchecked(buffer) }
}

pub struct Shader {
    id: GLuint,
}

impl Shader {
    pub fn from_res(res: &Resources, name: &str) -> Result<Shader, Error> {
        const POSSIBLE_EXT: [(&str, GLenum); 2] =
            [(".vert", gl::VERTEX_SHADER), (".frag", gl::FRAGMENT_SHADER)];

        let shader_kind = POSSIBLE_EXT
            .iter()
            .find(|&&(file_extension, _)| name.ends_with(file_extension))
            .map(|&(_, kind)| kind)
            .ok_or_else(|| Error::NoShaderType {
                name: name.to_string(),
            })?;

        let source = res.load_cstring(name).map_err(|e| Error::ResourceLoad {
            name: name.to_string(),
            inner: e,
        })?;

        Shader::from_source(&source, shader_kind).map_err(|e| Error::CompileError {
            name: name.to_string(),
            message: e,
        })
    }

    pub fn from_source(source: &CStr, kind: GLenum) -> Result<Shader, String> {
        let id = shader_from_source(source, kind)?;
        Ok(Shader { id })
    }

    pub fn from_vert(source: &CStr) -> Result<Shader, String> {
        Shader::from_source(source, gl::VERTEX_SHADER)
    }

    pub fn from_frag(source: &CStr) -> Result<Shader, String> {
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
