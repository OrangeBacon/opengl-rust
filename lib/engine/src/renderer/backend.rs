use anyhow::Result;

use super::Pipeline;
use crate::texture::Texture;

/// type inside all *Id tuple structs
pub type IdType = u64;

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextureId(pub(crate) IdType);

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VertexBufferId(pub(crate) IdType);

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IndexBufferId(pub(crate) IdType);

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PipelineId(pub(crate) IdType);

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

    /// Load data as a vertex buffer
    fn load_vertex_buffer(&mut self, data: &[u8]) -> VertexBufferId;

    /// Unload a vertex buffer
    fn unload_vertex_buffer(&mut self, buffer: VertexBufferId);

    /// Load data as an index buffer
    fn load_index_buffer(&mut self, data: &[u8]) -> IndexBufferId;

    /// Unload an index buffer
    fn unload_index_buffer(&mut self, buffer: IndexBufferId);

    /// Load a new pipeline
    fn load_pipeline(&mut self, pipeline: Pipeline) -> Result<PipelineId>;

    /// Unloads a pipeline
    fn unload_pipeline(&mut self, pipeline: PipelineId);
}
