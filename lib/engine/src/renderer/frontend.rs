use crate::texture::Texture;

use super::backend::{RendererBackend, TextureId};

pub struct Renderer {
    backend: Box<dyn RendererBackend>,
}

impl Renderer {
    pub fn new(backend: Box<dyn RendererBackend>) -> Self {
        Self { backend }
    }

    /// Clear the screen to the specified color
    #[inline(always)]
    pub fn clear(&mut self, r: f32, g: f32, b: f32) {
        self.backend.clear(r, g, b)
    }

    /// Set the viewport size
    #[inline(always)]
    pub fn viewport(&mut self, width: u32, height: u32) {
        self.backend.viewport(width, height)
    }

    /// Enable or disable backface culling
    #[inline(always)]
    pub fn backface_culling(&mut self, enable: bool) {
        self.backend.backface_culling(enable)
    }

    /// Load a new texture
    #[inline(always)]
    pub fn load_texture(&mut self, texture: Texture) -> TextureId {
        self.backend.load_texture(texture)
    }

    /// Unload a texture
    #[inline(always)]
    pub fn unload_texture(&mut self, texture: TextureId) {
        self.backend.unload_texture(texture)
    }
}
