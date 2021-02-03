use std::collections::HashMap;

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
    #[error("Error decoding image {name}: {inner}")]
    Decode {
        name: String,
        #[source]
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
pub struct TextureData {
    image: ImageBuffer<Rgb<u8>, Vec<u8>>,
    sampler: Sampler,
    id: u64,
    index: GLuint,
}

impl TextureData {
    /// Read an image from a file and load it into the GPU
    /// gl: current OpenGL context to load using
    /// res: current resource loader
    /// name: path to texture relative to the current resource loader
    /// index: active texture unit number that gets used during texture loading
    ///    and when binding the texture
    pub fn from_res(
        res: &Resources,
        name: &str,
        sampler: Sampler,
        id: u64,
        index: GLuint,
    ) -> Result<Self, Error> {
        let data = res.load_bytes(name).map_err(|e| Error::ResourceLoad {
            name: name.to_string(),
            inner: e,
        })?;

        TextureData::load_from_bytes(&data, name, sampler, id, index)
    }

    pub fn load_from_bytes(
        data: &[u8],
        name: &str,
        sampler: Sampler,
        id: u64,
        index: GLuint,
    ) -> Result<Self, Error> {
        let image = image::load_from_memory(data);

        let image = image
            .map_err(|e| Error::Decode {
                name: name.to_string(),
                inner: e,
            })?
            .into_rgb8();

        Ok(TextureData {
            image,
            sampler,
            id,
            index,
        })
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

pub struct TextureRender {
    gl: gl::Gl,
    index: GLenum,
    id: GLuint,
}

impl TextureRender {
    fn new(gl: &gl::Gl, image: &TextureData) -> Self {
        let sampler = image.sampler();
        let index = image.index;

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
                image.width() as _,
                image.height() as _,
                0,
                gl::RGB,
                gl::UNSIGNED_BYTE,
                image.img_ptr() as _,
            );

            gl.GenerateMipmap(gl::TEXTURE_2D);
        }

        TextureRender {
            index,
            gl: gl.clone(),
            id: texture,
        }
    }

    pub fn index(&self) -> GLenum {
        self.index
    }

    /// Bind this texture to the current shader program.
    pub fn bind(&self) {
        unsafe {
            self.gl.ActiveTexture(gl::TEXTURE0 + self.index);
            self.gl.BindTexture(gl::TEXTURE_2D, self.id);
        }
    }

    /// Unbind this texture, is normally not needed as binding a different
    /// texture will override the previously bound one.
    pub fn unbind(&self) {
        unsafe {
            self.gl.ActiveTexture(gl::TEXTURE0 + self.index);
            self.gl.BindTexture(gl::TEXTURE_2D, 0);
        }
    }
}

impl Drop for TextureRender {
    /// deletes the texture from vram
    fn drop(&mut self) {
        unsafe {
            self.unbind();
            self.gl.DeleteTextures(1, &self.id);
        }
    }
}

pub struct TextureCache {
    texures: HashMap<u64, TextureRender>,
}

impl TextureCache {
    pub fn new() -> Self {
        TextureCache {
            texures: HashMap::new(),
        }
    }

    pub fn load(&mut self, gl: &gl::Gl, image: TextureData) {
        let tex = TextureRender::new(gl, &image);
        self.texures.insert(image.id, tex);
    }

    pub fn unload(&mut self, id: u64) {
        self.texures.remove(&id);
    }
}
