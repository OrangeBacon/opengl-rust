pub mod gl_render;

/// The methods required for each renderer backend to implement
pub trait Renderer {
    /// Clear the screen to the specified color
    fn clear(&mut self, r: f32, g: f32, b: f32);

    /// Set the viewport size
    fn viewport(&mut self, width: u32, height: u32);
}
