use crate::resources::{Error as ResourceError, Resources};
use anyhow::Result;
use gl::types::*;
use image::{ImageBuffer, Rgb};
use thiserror::Error;

/// Errors representing issues loading and decoding images
#[derive(Debug, Error)]
pub enum Error {
    /// wrapper error from engine's resource loader
    #[error("Error loading {name}: {inner}")]
    ResourceLoad {
        name: String,
        #[source]
        inner: ResourceError,
    },

    /// image decoding errors
    #[error("Error decoding image: {inner}")]
    Decode {
        #[from]
        inner: image::ImageError,
    },
}

/// OpenGL filtering and wrapping properties
#[derive(Debug)]
pub struct Sampler {
    pub(crate) wrap_s: GLint,
    pub(crate) wrap_t: GLint,
    pub(crate) min_filter: GLint,
    pub(crate) mag_filter: GLint,
}

impl Default for Sampler {
    fn default() -> Self {
        Sampler {
            wrap_s: gl::REPEAT as _,
            wrap_t: gl::REPEAT as _,
            min_filter: gl::NEAREST as _,
            mag_filter: gl::NEAREST as _,
        }
    }
}

/// A loaded texture, stored on the GPU.  When dropped, the vram is released.
#[derive(Debug)]
pub struct Texture {
    image: ImageBuffer<Rgb<u8>, Vec<u8>>,
    sampler: Sampler,
}

impl Texture {
    pub fn from_res(res: &Resources, name: &str, sampler: Sampler) -> Result<Self, Error> {
        let data = res.load_bytes(name).map_err(|e| Error::ResourceLoad {
            name: name.to_string(),
            inner: e,
        })?;

        Self::load_from_bytes(&data, sampler)
    }

    pub fn load_from_bytes(data: &[u8], sampler: Sampler) -> Result<Self, Error> {
        let image = image::load_from_memory(data)?.into_rgb8();

        Ok(Texture { image, sampler })
    }

    pub fn width(&self) -> u32 {
        self.image.width()
    }

    pub fn height(&self) -> u32 {
        self.image.height()
    }

    pub fn img_ptr(&self) -> *const u8 {
        self.image.as_ptr()
    }

    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }
}

pub struct GlTexture {
    gl: gl::Gl,
    id: GLuint,
}

impl GlTexture {
    pub fn new(gl: &gl::Gl, tex: &Texture, index: GLuint) -> Self {
        let sampler = tex.sampler();

        let mut texture = 0;
        unsafe {
            gl.ActiveTexture(gl::TEXTURE0 + index);
            gl.GenTextures(1, &mut texture);
            gl.BindTexture(gl::TEXTURE_2D, texture);

            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, sampler.wrap_s);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, sampler.wrap_t);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, sampler.min_filter);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, sampler.mag_filter);

            gl.TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGB as _,
                tex.width() as _,
                tex.height() as _,
                0,
                gl::RGB,
                gl::UNSIGNED_BYTE,
                tex.img_ptr() as _,
            );

            gl.GenerateMipmap(gl::TEXTURE_2D);
        }

        Self {
            gl: gl.clone(),
            id: texture,
        }
    }

    /// Bind this texture to the current shader program.
    pub fn bind(&self, index: GLuint) -> BoundGlTexture {
        BoundGlTexture::new(&self, index)
    }
}

impl Drop for GlTexture {
    /// deletes the texture from vram
    fn drop(&mut self) {
        unsafe {
            self.gl.DeleteTextures(1, &self.id);
        }
    }
}

pub struct BoundGlTexture<'a> {
    tex: &'a GlTexture,
    index: GLuint,
}

impl<'a> BoundGlTexture<'a> {
    fn new(tex: &'a GlTexture, index: GLuint) -> Self {
        unsafe {
            tex.gl.ActiveTexture(gl::TEXTURE0 + index);
            tex.gl.BindTexture(gl::TEXTURE_2D, tex.id);
        }

        Self { tex, index }
    }

    /// Get the index of the texture's active texture unit.
    pub fn index(&self) -> GLuint {
        self.index
    }
}

impl<'a> Drop for BoundGlTexture<'a> {
    fn drop(&mut self) {
        unsafe {
            self.tex.gl.ActiveTexture(gl::TEXTURE0 + self.index);
            self.tex.gl.BindTexture(gl::TEXTURE_2D, 0);
        }
    }
}
