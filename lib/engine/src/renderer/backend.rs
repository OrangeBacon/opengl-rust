use anyhow::Result;
use nalgebra_glm as glm;

use super::{
    shader::Program, CullingMode, DepthTesting, DrawingMode, IndexBufferId, IndexType, PipelineId,
    TextureId, VertexBufferId,
};
use crate::texture::Texture;

/// The methods required for each renderer backend to implement
pub trait RendererBackend {
    /// Clear the screen to the specified color
    fn clear(&mut self, r: f32, g: f32, b: f32);

    /// Set the viewport size
    fn viewport(&mut self, width: u32, height: u32);

    /// Enable or disable backface culling
    fn backface_culling(&mut self, enable: CullingMode);

    fn depth_testing(&mut self, mode: DepthTesting);

    /// Load a new texture
    fn load_texture(&mut self, texture: Texture) -> TextureId;

    /// Unload a texture
    fn unload_texture(&mut self, texture: TextureId);

    /// Load data as a vertex buffer
    fn load_vertex_buffer(&mut self, data: &[u8]) -> VertexBufferId;

    /// Load data as a vertex buffer for streaming upload
    fn load_vertex_buffer_stream(&mut self, data: &[u8]) -> VertexBufferId;

    /// Unload a vertex buffer
    fn unload_vertex_buffer(&mut self, buffer: VertexBufferId);

    /// Load data as an index buffer
    fn load_index_buffer(&mut self, data: &[u8]) -> IndexBufferId;

    /// Load data as an index buffer for streaming upload
    fn load_index_buffer_stream(&mut self, data: &[u8]) -> IndexBufferId;

    /// Unload an index buffer
    fn unload_index_buffer(&mut self, buffer: IndexBufferId);

    /// Load a new pipeline
    fn load_pipeline(&mut self, pipeline: Program) -> Result<PipelineId>;

    /// Unloads a pipeline
    fn unload_pipeline(&mut self, pipeline: PipelineId);

    /// Bind a pipeline so that vertex buffers and uniforms can be bound to it
    fn bind_pipeline(&mut self, pipeline: PipelineId);

    /// Unbind a bound pipeline
    fn unbind_pipeline(&mut self, pipeline: PipelineId);

    /// Bind a 4x4 matrix to a pipeline
    fn pipeline_bind_matrix(
        &mut self,
        pipeline: PipelineId,
        name: &str,
        matrix: glm::Mat4,
    ) -> Result<()>;

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
        strides: &[i32],
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
