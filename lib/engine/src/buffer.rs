use gl::types::*;
use std::mem::size_of;

#[derive(Debug)]
pub struct Buffer {
    gl: gl::Gl,
    vbo: GLuint,
    pub buffer_type: GLenum,
}

impl Buffer {
    pub fn new(gl: &gl::Gl, buffer_type: GLenum) -> Buffer {
        let mut vbo = 0;
        unsafe {
            gl.GenBuffers(1, &mut vbo);
        }

        Buffer {
            gl: gl.clone(),
            vbo,
            buffer_type,
        }
    }

    pub fn bind(&self) {
        unsafe {
            self.gl.BindBuffer(self.buffer_type, self.vbo);
        }
    }

    pub fn unbind(&self) {
        unsafe {
            self.gl.BindBuffer(self.buffer_type, 0);
        }
    }

    pub fn static_draw_data<T>(&self, data: &[T]) {
        unsafe {
            self.gl.BufferData(
                self.buffer_type,
                (data.len() * size_of::<T>()) as GLsizeiptr,
                data.as_ptr() as *const GLvoid,
                gl::STATIC_DRAW,
            );
        }
    }

    pub fn id(&self) -> GLuint {
        self.vbo
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            self.gl.DeleteBuffers(1, &self.vbo);
        }
    }
}

#[derive(Debug)]
pub struct VertexArray {
    gl: gl::Gl,
    vao: GLuint,
}

impl VertexArray {
    pub fn new(gl: &gl::Gl) -> VertexArray {
        let mut vao = 0;
        unsafe { gl.GenVertexArrays(1, &mut vao) }

        VertexArray {
            gl: gl.clone(),
            vao,
        }
    }

    pub fn bind(&self) {
        unsafe {
            self.gl.BindVertexArray(self.vao);
        }
    }

    pub fn unbind(&self) {
        unsafe {
            self.gl.BindVertexArray(0);
        }
    }
}

impl Drop for VertexArray {
    fn drop(&mut self) {
        unsafe {
            self.gl.DeleteVertexArrays(1, &self.vao);
        }
    }
}
