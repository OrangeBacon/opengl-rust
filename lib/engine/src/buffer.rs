use gl::types::*;
use std::mem::size_of;

pub struct Buffer<const TYPE: u32> {
    vbo: GLuint,
}

pub type ArrayBuffer = Buffer<{ gl::ARRAY_BUFFER }>;
pub type ElementArrayBuffer = Buffer<{ gl::ELEMENT_ARRAY_BUFFER }>;

impl<const TYPE: u32> Buffer<TYPE> {
    pub fn new() -> Buffer<TYPE> {
        let mut vbo = 0;
        unsafe {
            gl::GenBuffers(1, &mut vbo);
        }

        Buffer {
            vbo,
        }
    }

    pub fn bind(&self) {
        unsafe {
            gl::BindBuffer(TYPE, self.vbo);
        }
    }

    pub fn unbind(&self) {
        unsafe {
            gl::BindBuffer(TYPE, 0);
        }
    }

    pub fn static_draw_data<T>(&self, data: &[T]) {
        unsafe {
            gl::BufferData(
                TYPE,
                (data.len() * size_of::<T>()) as GLsizeiptr,
                data.as_ptr() as *const GLvoid,
                gl::STATIC_DRAW,
            );
        }
    }
}

impl<const TYPE: u32> Drop for Buffer<TYPE> {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &mut self.vbo);
        }
    }
}

pub struct VertexArray {
    vao: GLuint,
}

impl VertexArray {
    pub fn new() -> VertexArray {
        let mut vao = 0;
        unsafe { gl::GenVertexArrays(1, &mut vao) }

        VertexArray {
            vao,
        }
    }

    pub fn bind(&self) {
        unsafe {
            gl::BindVertexArray(self.vao);
        }
    }

    pub fn unbind(&self) {
        unsafe {
            gl::BindVertexArray(0);
        }
    }
}

impl Drop for VertexArray {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &mut self.vao);
        }
    }
}
