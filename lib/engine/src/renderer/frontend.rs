use crate::texture::Texture;

use super::backend::{IndexBufferId, PipelineId, RendererBackend, TextureId, VertexBufferId};

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
    pub fn load_pipeline(&mut self, pipeline: Pipeline) -> PipelineId {
        self.backend.load_pipeline(pipeline)
    }

    /// Unload a pipeline
    #[inline(always)]
    pub fn unload_pipeline(&mut self, pipeline: PipelineId) {
        self.backend.unload_pipeline(pipeline)
    }
}

/// A rendering pipeline, in OpenGl would be one shader program
pub struct Pipeline {
    vertex_shader: Option<String>,
    fragment_shader: Option<String>,
    attributes: Vec<VertexAttribute>,
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

struct VertexAttribute {
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
pub enum AttributeType {
    I8,
    I16,
    F32,
    F64,
    U8,
    U16,
    U32,
}
