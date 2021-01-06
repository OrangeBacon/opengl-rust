use super::{utils, Shader};
use gl::types::*;
use std::{error::Error, ffi::CString, fs};

pub struct Program {
    id: GLuint,
}

impl Program {
    pub fn from_shaders(shaders: &[Shader]) -> Result<Program, utils::GLError> {
        let program_id = unsafe { gl::CreateProgram() };

        for shader in shaders {
            unsafe {
                gl::AttachShader(program_id, shader.id());
            }
        }

        unsafe {
            gl::LinkProgram(program_id);
        }

        let mut success: GLint = 1;
        unsafe {
            gl::GetProgramiv(program_id, gl::LINK_STATUS, &mut success);
        }

        if success == 0 {
            return Err(utils::get_gl_error(program_id));
        }

        for shader in shaders {
            unsafe {
                gl::DetachShader(program_id, shader.id());
            }
        }

        Ok(Program { id: program_id })
    }

    pub fn set_used(&self) {
        unsafe {
            gl::UseProgram(self.id);
        }
    }

    pub fn set_int(&self, name: &str, val: i32) {
        unsafe {
            gl::Uniform1i(
                gl::GetUniformLocation(self.id, name.as_ptr() as *const i8),
                val,
            );
        }
    }

    pub fn from_files(shaders: &[&str]) -> Result<Program, Box<dyn Error>> {
        let shaders: Result<Vec<_>, Box<dyn Error>> = shaders
            .iter()
            .map(|&p| {
                let data = fs::read_to_string(p)?;
                let data = CString::new(data)?;
                Ok((p, data))
            })
            .collect();

        let shaders: Result<Vec<_>, _> = shaders?
            .iter()
            .map(|(path, data)| {
                if path.ends_with("vert") {
                    Shader::from_vert(&data)
                } else {
                    Shader::from_frag(&data)
                }
            })
            .collect();

        Ok(Program::from_shaders(&shaders.unwrap()).unwrap())
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.id);
        }
    }
}
