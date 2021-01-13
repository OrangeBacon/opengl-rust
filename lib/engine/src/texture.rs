use crate::resources::{Error as ResourceError, Resources};
use anyhow::Result;
use gl::types::*;
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

/// A loaded texture, stored on the GPU.  When dropped, the vram is released.
pub struct Texture {
    gl: gl::Gl,
    id: GLuint,
    texture_index: GLuint,
}

impl Texture {
    /// Readan image from a file and load it into the GPU
    /// gl: current OpenGL context to load using
    /// res: current resource loader
    /// name: path to texture relative to the current resource loader
    /// index: active texture unit number that gets used during texture loading
    ///    and when binding the texture
    pub fn from_res(
        gl: &gl::Gl,
        res: &Resources,
        name: &str,
        index: GLuint,
    ) -> Result<Self, Error> {
        let data = res.load_bytes(name).map_err(|e| Error::ResourceLoad {
            name: name.to_string(),
            inner: e,
        })?;

        let image = image::load_from_memory(&data);

        let mut image = image
            .map_err(|e| Error::Decode {
                name: name.to_string(),
                inner: e,
            })?
            .into_rgb8();

        // OpenGL's coordinate system is inverted on the y-axis, so flip all
        // images when loading, so they render in the correct orientation
        image::imageops::flip_vertical_in_place(&mut image);

        let mut texture = 0;
        unsafe {
            gl.ActiveTexture(gl::TEXTURE0 + index);
            gl.GenTextures(1, &mut texture);
            gl.BindTexture(gl::TEXTURE_2D, texture);

            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::REPEAT as _);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::REPEAT as _);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);

            gl.TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGB as _,
                image.width() as _,
                image.height() as _,
                0,
                gl::RGB,
                gl::UNSIGNED_BYTE,
                image.as_ptr() as _,
            );

            gl.GenerateMipmap(gl::TEXTURE_2D);
        }

        Ok(Texture {
            texture_index: index,
            gl: gl.clone(),
            id: texture,
        })
    }

    /// Get the index of the texture's active texture unit.
    pub fn index(&self) -> GLuint {
        self.texture_index
    }

    /// Bind this texture to the current shader program.
    pub fn bind(&self) {
        unsafe {
            self.gl.ActiveTexture(gl::TEXTURE0 + self.texture_index);
            self.gl.BindTexture(gl::TEXTURE_2D, self.id);
        }
    }

    /// Unbind this texture, is normally not needed as binding a different
    /// texture will override the previously bound one.
    pub fn unbind(&self) {
        unsafe {
            self.gl.ActiveTexture(gl::TEXTURE0 + self.texture_index);
            self.gl.BindTexture(gl::TEXTURE_2D, 0);
        }
    }
}

impl Drop for Texture {
    /// deletes the texture from vram
    fn drop(&mut self) {
        unsafe {
            self.unbind();
            self.gl.DeleteTextures(1, &self.id);
        }
    }
}
