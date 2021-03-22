use anyhow::Result;
use nalgebra_glm as glm;

use super::{DrawingMode, IndexType, Pipeline};
use crate::texture::Texture;

/// type inside all *Id tuple structs
pub type IdType = u64;

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq)]
pub struct TextureId(pub(crate) IdType);

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq)]
pub struct VertexBufferId(pub(crate) IdType);

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq)]
pub struct IndexBufferId(pub(crate) IdType);

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq)]
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

    /// Bind a pipeline so that vertex buffers and uniforms can be bound to it
    fn bind_pipeline(&mut self, pipeline: PipelineId);

    /// Unbind a bound pipeline
    fn unbind_pipeline(&mut self, pipeline: PipelineId);

    /// Bind a 4x4 matrix to a pipeline
    fn pipeline_bind_matrix(&mut self, pipeline: PipelineId, name: &str, matrix: glm::Mat4);

    /// Bind a texture to a pipeline
    fn pipeline_bind_texture(
        &mut self,
        pipeline: PipelineId,
        name: &str,
        texture: TextureId,
    ) -> Result<()>;

    /// Bind vertex arrays with a given offset and stride to a bound pipeline
    /// offset and stride are both measured in bytes.
    fn pipeline_bind_vertex_arrays(
        &mut self,
        pipeline: PipelineId,
        buffers: &[VertexBufferId],
        offsets: &[usize],
        strides: &[usize],
    );

    /// draw verticies using a pipeline
    fn draw(&mut self, pipeline: PipelineId, mode: DrawingMode, start: u64, count: u64);

    /// draw indexed verticies
    fn draw_indicies(
        &mut self,
        pipeline: PipelineId,
        mode: DrawingMode,
        indices: IndexBufferId,
        index_type: IndexType,
        index_offset: usize,
        count: usize,
    );
}
