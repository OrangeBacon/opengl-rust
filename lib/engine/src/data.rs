use gl::types::*;

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct f32_f32_f32 {
    pub d0: f32,
    pub d1: f32,
    pub d2: f32,
}

impl f32_f32_f32 {
    pub fn new(d0: f32, d1: f32, d2: f32) -> f32_f32_f32 {
        f32_f32_f32 { d0, d1, d2 }
    }

    pub fn attrib_pointer(stride: usize, location: usize, offset: usize) {
        unsafe {
            gl::EnableVertexAttribArray(location as GLuint);
            gl::VertexAttribPointer(
                location as GLuint,
                3,
                gl::FLOAT,
                gl::FALSE,
                stride as GLint,
                offset as *const GLvoid,
            );
        }
    }
}

impl From<(f32, f32, f32)> for f32_f32_f32 {
    fn from(other: (f32, f32, f32)) -> Self {
        f32_f32_f32::new(other.0, other.1, other.2)
    }
}
