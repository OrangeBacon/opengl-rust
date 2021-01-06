use image::io::Reader as ImageReader;
use gl::types::*;

pub struct Image {
    data: image::RgbImage,
    id: GLuint,
}

impl Image {
    pub fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut img = ImageReader::open(path)?
            .decode()?
            .into_rgb8();
        image::imageops::flip_vertical_in_place(&mut img);

        let mut ret = Image {data: img, id: 0};
        ret.gen_buffer();
        Ok(ret)
    }

    pub fn gen_buffer(&mut self) {
        unsafe {
            gl::GenTextures(1, &mut self.id);
            gl::BindTexture(gl::TEXTURE_2D, self.id);

            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::REPEAT as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::REPEAT as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGB as GLint,
                self.data.width() as GLint,
                self.data.height() as GLint,
                0,
                gl::RGB,
                gl::UNSIGNED_BYTE,
                self.data.as_raw().as_ptr() as *const GLvoid,
            );
            gl::GenerateMipmap(gl::TEXTURE_2D);
        }
    }

    pub fn id(&self) -> GLuint {
        self.id
    }
}