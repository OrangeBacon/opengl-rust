use anyhow::Result;
use nalgebra_glm as glm;

use super::backend::{IndexBufferId, PipelineId, RendererBackend, TextureId, VertexBufferId};
use crate::texture::Texture;

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

    /// Load data as a vertex buffer
    #[inline(always)]
    pub fn load_vertex_buffer(&mut self, data: &[u8]) -> VertexBufferId {
        self.backend.load_vertex_buffer(data)
    }

    /// Unload a vertex buffer
    #[inline(always)]
    pub fn unload_vertex_buffer(&mut self, buffer: VertexBufferId) {
        self.backend.unload_vertex_buffer(buffer)
    }

    /// Load data as an index buffer
    #[inline(always)]
    pub fn load_index_buffer(&mut self, data: &[u8]) -> IndexBufferId {
        self.backend.load_index_buffer(data)
    }

    /// Unload an index buffer
    #[inline(always)]
    pub fn unload_index_buffer(&mut self, buffer: IndexBufferId) {
        self.backend.unload_index_buffer(buffer)
    }

    /// Load a new pipeline, including shader compilation
    #[inline(always)]
    pub fn load_pipeline(&mut self, pipeline: Pipeline) -> Result<PipelineId> {
        self.backend.load_pipeline(pipeline)
    }

    /// Unload a pipeline
    #[inline(always)]
    pub fn unload_pipeline(&mut self, pipeline: PipelineId) {
        self.backend.unload_pipeline(pipeline)
    }

    /// Bind a pipeline so it can be used for drawing
    #[inline(always)]
    pub fn bind_pipeline(&mut self, pipeline: PipelineId) -> BoundPipeline {
        BoundPipeline::new(self, pipeline)
    }
}

/// A rendering pipeline, in OpenGl would be one shader program
pub struct Pipeline {
    pub(crate) vertex_shader: Option<String>,
    pub(crate) fragment_shader: Option<String>,
    pub(crate) attributes: Vec<VertexAttribute>,
}

impl Pipeline {
    /// create a new pipeline
    pub fn new() -> Self {
        Pipeline {
            vertex_shader: None,
            fragment_shader: None,
            attributes: vec![],
        }
    }

    /// set the vertex shader source
    pub fn vertex_shader(&mut self, source: &str) -> &mut Self {
        self.from_vertex_shader(source.to_string())
    }

    /// set the vertex shader source
    pub fn from_vertex_shader(&mut self, source: String) -> &mut Self {
        self.vertex_shader = Some(source);
        self
    }

    /// set the fragment shader source
    pub fn frag_shader(&mut self, source: &str) -> &mut Self {
        self.from_frag_shader(source.to_string())
    }

    /// set the fragment shader source
    pub fn from_frag_shader(&mut self, source: String) -> &mut Self {
        self.fragment_shader = Some(source);
        self
    }

    /// add a vertex attribute, e.g. vertex position, uv coordinates, ...
    pub fn vertex_attribute(
        &mut self,
        location: u32,
        count: usize,
        item_type: AttributeType,
        normalised: bool,
    ) -> &mut Self {
        self.attributes
            .push(VertexAttribute::new(location, count, item_type, normalised));
        self
    }
}

pub(crate) struct VertexAttribute {
    /// Location specified in the shader by layout(location = N)
    pub(crate) location: u32,

    /// The number of items in this vertex attribute, e.g. vec3 => 3
    pub(crate) count: usize,

    /// The type of the items in this attribute
    pub(crate) item_type: AttributeType,

    /// Whether the values should be normalised
    pub(crate) normalised: bool,
}

impl VertexAttribute {
    pub fn new(location: u32, count: usize, item_type: AttributeType, normalised: bool) -> Self {
        Self {
            location,
            count,
            item_type,
            normalised,
        }
    }
}

/// The type of a vertex attribute, enum names correspond to the equivalent rust types
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum AttributeType {
    I8,
    I16,
    F32,
    F64,
    U8,
    U16,
    U32,
}

pub struct BoundPipeline<'a> {
    renderer: &'a mut Renderer,
    pipeline: PipelineId,
}

impl<'a> BoundPipeline<'a> {
    pub fn new(renderer: &'a mut Renderer, pipeline: PipelineId) -> Self {
        renderer.backend.bind_pipeline(pipeline);
        Self { renderer, pipeline }
    }

    pub fn bind_matrix(&mut self, name: &str, matrix: glm::Mat4) {
        self.renderer
            .backend
            .pipeline_bind_matrix(self.pipeline, name, matrix);
    }

    pub fn bind_texture(&mut self, name: &str, texture: TextureId) -> Result<()> {
        self.renderer
            .backend
            .pipeline_bind_texture(self.pipeline, name, texture)
    }

    pub fn bind_vertex_arrays(
        &mut self,
        buffers: &[VertexBufferId],
        offsets: &[usize],
        strides: &[usize],
    ) {
        self.renderer
            .backend
            .pipeline_bind_vertex_arrays(self.pipeline, buffers, offsets, strides);
    }

    pub fn draw(&mut self, mode: DrawingMode, start: u64, count: u64) {
        self.renderer
            .backend
            .draw(self.pipeline, mode, start, count);
    }

    /// draw indexed verticies using a pipeline
    /// draws count verticies
    pub fn draw_indicies(
        &mut self,
        mode: DrawingMode,
        indices: IndexBufferId,
        index_type: IndexType,
        index_offset: usize,
        count: usize,
    ) {
        self.renderer.backend.draw_indicies(
            self.pipeline,
            mode,
            indices,
            index_type,
            index_offset,
            count,
        );
    }
}

impl<'a> Drop for BoundPipeline<'a> {
    fn drop(&mut self) {
        self.renderer.backend.unbind_pipeline(self.pipeline);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawingMode {
    Points,
    Lines,
    LineLoop,
    LineStrip,
    Triangles,
    TriangleStrip,
    TriangleFan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndexType {
    U8,
    U16,
    U32,
}
