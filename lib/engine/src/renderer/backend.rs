use crate::texture::Texture;

/// type inside all *Id tuple structs
pub type IdType = usize;

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextureId(pub(crate) IdType);

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ShaderId(pub(crate) IdType);

/// The methods required for each renderer backend to implement
pub trait RendererBackend {
    /// Clear the screen to the specified color
    fn clear(&mut self, r: f32, g: f32, b: f32);

    /// Set the viewport size
    fn viewport(&mut self, width: u32, height: u32);

    /// Enable or disable backface culling
    fn backface_culling(&mut self, enable: bool);

    /// Load a new texture
    fn load_texture(&mut self, texture: Texture) -> TextureId;

    /// Unload a texture
    fn unload_texture(&mut self, texture: TextureId);
}
