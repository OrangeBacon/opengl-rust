#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextureId(pub(crate) usize);

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ShaderId(pub(crate) usize);

/// The methods required for each renderer backend to implement
pub trait RendererBackend {
    /// Clear the screen to the specified color
    fn clear(&mut self, r: f32, g: f32, b: f32);

    /// Set the viewport size
    fn viewport(&mut self, width: u32, height: u32);

    /// Enable or disable backface culling
    fn backface_culling(&mut self, enable: bool);
}
