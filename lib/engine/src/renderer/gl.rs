use anyhow::Result;
use gl::types::*;
use std::{
    collections::HashMap,
    ffi::{CStr, CString, NulError},
    mem::size_of,
};
use thiserror::Error;

use crate::texture::{
    MagFilter, MinFilter, Texture, TextureSourceFormat, TextureSourceType, TextureStorageType,
    WrappingMode,
};

use super::{
    backend::RendererBackend,
    shader::{Program, Type},
    CullingMode, DrawingMode, IdType, IndexBufferId, IndexType, PipelineId, TextureId,
    VertexBufferId,
};

/// Possible errors encounted in OpenGl
#[derive(Debug, Error)]
enum GlError {
    #[error("Error compiling shader:\n{message}")]
    ShaderCompilation { message: String },

    #[error("Error linking shaders:\n{message}")]
    ShaderLink { message: String },

    #[error("Shader code contained nul byte, unable to compile it:\n{message}")]
    ShaderNullByte { message: NulError },

    #[error("Error getting buffer for error message, unable to display error")]
    ErrorBuffer,

    #[error("Unable to find a free active texture unit")]
    TextureUnitsFull,

    #[error("Cannot bind texture to unbound pipeline")]
    PipelineNotBound,

    #[error("Texture is not currently loaded, cannot bind it to a pipeline")]
    TextureUnloaded,

    #[error("Unable to load opaque type as vertex array")]
    OpaqueVerticies,
}

/// OpenGl renderer implementation
pub struct GlRenderer {
    /// The current opengl context used for all rendering operations
    gl: gl::Gl,

    /// The current id counter, constantly increasing counter, allocating any
    /// resource adds one to this counter
    id: IdType,

    /// All the currently loaded textures
    textures: HashMap<IdType, GlTexture>,

    /// All the currently loaded buffers stored on the gpu
    buffers: HashMap<IdType, Buffer>,

    /// All the shader pipelines currently avaliable
    pipelines: HashMap<IdType, GlPipeline>,

    /// A vector of all the texture units, if true then in use, if false then
    /// not in use.  Unit 0 is always set as in use as it is used as the binding
    /// location while loading new textures
    texture_units: Vec<bool>,

    /// A map connecting the active pipeliness and the indicies into the texture_units
    /// vec that the pipeline is currently using
    active_textures: HashMap<PipelineId, Vec<usize>>,

    /// Whether backface culling is enabled for all future draw calls
    backface_culling_enabled: bool,

    backface_culling_mode: GLuint,
}

impl GlRenderer {
    /// Create a new OpenGl rendering backend
    pub fn new(gl: gl::Gl) -> Self {
        // only enable gl debug logging in debug mode, todo: propper logging that
        // isn't just to the terminal
        if cfg!(debug_assertions) {
            enable_gl_debugging(&gl);
        }

        // depth desting is enabled to begin with
        unsafe { gl.Enable(gl::DEPTH_TEST) }

        // initial culling mode is back faces culled
        unsafe { gl.CullFace(gl::BACK) }

        // get maximum number of active texture units
        let mut texture_units = 0;
        unsafe {
            gl.GetIntegerv(gl::MAX_COMBINED_TEXTURE_IMAGE_UNITS, &mut texture_units);
        }

        let mut texture_units = vec![false; texture_units as _];

        // texture unit 0 is used for image loading so don't allow pipelines to
        // use it ever
        texture_units[0] = true;

        GlRenderer {
            gl,
            id: 0,
            backface_culling_enabled: false,
            backface_culling_mode: gl::BACK,
            textures: HashMap::new(),
            buffers: HashMap::new(),
            pipelines: HashMap::new(),
            active_textures: HashMap::new(),
            texture_units,
        }
    }
}

impl RendererBackend for GlRenderer {
    fn clear(&mut self, r: f32, g: f32, b: f32) {
        // assumes that the framebuffer has no alpha and that depth should
        // also be cleared
        unsafe {
            self.gl.ClearColor(r, g, b, 1.0);
            self.gl.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }
    }

    fn viewport(&mut self, width: u32, height: u32) {
        // top left (0, 0) view port always
        unsafe {
            self.gl.Viewport(0, 0, width as _, height as _);
        }
    }

    fn backface_culling(&mut self, enable: CullingMode) {
        // cache whether culling is enabled or not to reduce draw calls

        if enable != CullingMode::None && !self.backface_culling_enabled {
            unsafe { self.gl.Enable(gl::CULL_FACE) }
            self.backface_culling_enabled = true;
        }

        match enable {
            CullingMode::None if self.backface_culling_enabled => {
                unsafe { self.gl.Disable(gl::CULL_FACE) }
                self.backface_culling_enabled = false;
            }
            CullingMode::Front if self.backface_culling_mode != gl::FRONT => {
                unsafe { self.gl.CullFace(gl::FRONT) }
                self.backface_culling_mode = gl::FRONT;
            }
            CullingMode::Back if self.backface_culling_mode != gl::BACK => {
                unsafe { self.gl.CullFace(gl::BACK) }
                self.backface_culling_mode = gl::BACK;
            }
            CullingMode::FrontBack if self.backface_culling_mode != gl::FRONT_AND_BACK => {
                unsafe { self.gl.CullFace(gl::FRONT_AND_BACK) }
                self.backface_culling_mode = gl::FRONT_AND_BACK;
            }
            _ => (),
        }
    }

    fn load_texture(&mut self, texture: Texture) -> TextureId {
        let id = self.id;
        self.id += 1;

        self.textures
            .insert(id, GlTexture::new(&self.gl, &texture, 0));

        TextureId(id)
    }

    fn unload_texture(&mut self, texture: TextureId) {
        let removed = self.textures.remove(&texture.0);

        // if unloading a texture, it must have existed already
        debug_assert!(!removed.is_none());
    }

    fn load_vertex_buffer(&mut self, data: &[u8]) -> VertexBufferId {
        let id = self.id;
        self.id += 1;

        let buf = Buffer::new(&self.gl, gl::ARRAY_BUFFER);
        buf.bind();
        buf.static_draw_data(data);
        buf.unbind();

        self.buffers.insert(id, buf);

        VertexBufferId(id)
    }

    fn load_vertex_buffer_stream(&mut self, data: &[u8]) -> VertexBufferId {
        let id = self.id;
        self.id += 1;

        let buf = Buffer::new(&self.gl, gl::ARRAY_BUFFER);
        buf.bind();
        buf.static_draw_data_stream(data);
        buf.unbind();

        self.buffers.insert(id, buf);

        VertexBufferId(id)
    }

    fn unload_vertex_buffer(&mut self, buffer: VertexBufferId) {
        let removed = self.buffers.remove(&buffer.0);

        // if removing a vertex buffer it must have already existed
        debug_assert!(!removed.is_none());
    }

    fn load_index_buffer(&mut self, data: &[u8]) -> IndexBufferId {
        let id = self.id;
        self.id += 1;

        let buf = Buffer::new(&self.gl, gl::ELEMENT_ARRAY_BUFFER);
        buf.bind();
        buf.static_draw_data(data);
        buf.unbind();

        self.buffers.insert(id, buf);

        IndexBufferId(id)
    }

    fn load_index_buffer_stream(&mut self, data: &[u8]) -> IndexBufferId {
        let id = self.id;
        self.id += 1;

        let buf = Buffer::new(&self.gl, gl::ELEMENT_ARRAY_BUFFER);
        buf.bind();
        buf.static_draw_data_stream(data);
        buf.unbind();

        self.buffers.insert(id, buf);

        IndexBufferId(id)
    }

    fn unload_index_buffer(&mut self, buffer: IndexBufferId) {
        let removed = self.buffers.remove(&buffer.0);

        // if removing an index buffer it must have already existed
        debug_assert!(!removed.is_none());
    }

    fn load_pipeline(&mut self, pipeline: Program) -> Result<PipelineId> {
        let id = self.id;
        self.id += 1;

        self.pipelines
            .insert(id, GlPipeline::new(pipeline, self.gl.clone())?);

        Ok(PipelineId(id))
    }

    fn unload_pipeline(&mut self, pipeline: PipelineId) {
        let removed = self.pipelines.remove(&pipeline.0);

        // if removing a pipeline buffer it must have already existed
        debug_assert!(!removed.is_none());
    }

    fn bind_pipeline(&mut self, pipeline: PipelineId) {
        self.active_textures.insert(pipeline, vec![]);

        if let Some(pipeline) = self.pipelines.get_mut(&pipeline.0) {
            debug_assert!(!pipeline.is_bound);

            pipeline.bind(&self.gl);
        } else {
            debug_assert!(false, "Cannot bind non-existant pipeline");
        }
    }

    fn unbind_pipeline(&mut self, pipeline: PipelineId) {
        for &texture_unit in &self.active_textures[&pipeline] {
            self.texture_units[texture_unit] = false;
        }

        // doesn't matter if this succeeds, failure just means no textures were used
        self.active_textures.remove(&pipeline);

        if let Some(pipeline) = self.pipelines.get_mut(&pipeline.0) {
            debug_assert!(pipeline.is_bound);

            pipeline.unbind();
        } else {
            debug_assert!(false, "Cannot unbind non-existant pipeline");
        }
    }

    fn pipeline_bind_matrix(
        &mut self,
        pipeline: PipelineId,
        name: &str,
        matrix: nalgebra_glm::Mat4,
    ) -> Result<()> {
        let name = CString::new(name)?;

        if let Some(pipeline) = self.pipelines.get(&pipeline.0) {
            debug_assert!(pipeline.is_bound);

            // todo: cache get uniform location?
            unsafe {
                let loc = self
                    .gl
                    .GetUniformLocation(pipeline.program_id, name.as_ptr());
                self.gl
                    .UniformMatrix4fv(loc, 1, gl::FALSE, matrix.as_slice().as_ptr());
            }
        } else {
            debug_assert!(false, "Cannot bind matrix to non-existant pipeline");
        }

        Ok(())
    }

    fn pipeline_bind_texture(
        &mut self,
        pipeline: PipelineId,
        name: &str,
        texture: TextureId,
    ) -> Result<()> {
        // get the uniform's name
        let name = CString::new(name)?;

        // find the first avaliable texture unit
        let (texture_unit, _) = self
            .texture_units
            .iter()
            .enumerate()
            .find(|&(_, &in_use)| !in_use)
            .ok_or(GlError::TextureUnitsFull)?;

        // tell the renderer that a texture unit is in use
        self.texture_units[texture_unit] = true;
        self.active_textures
            .get_mut(&pipeline)
            .ok_or(GlError::PipelineNotBound)?
            .push(texture_unit);

        // tell the renderer which pipeline owns a particular texture unit
        self.textures
            .get_mut(&texture.0)
            .ok_or(GlError::TextureUnloaded)?
            .set_bound(texture_unit as _);

        // tell the shader about the texture unit
        if let Some(pipeline) = self.pipelines.get(&pipeline.0) {
            debug_assert!(pipeline.is_bound);

            unsafe {
                let loc = self
                    .gl
                    .GetUniformLocation(pipeline.program_id, name.as_ptr());
                self.gl.Uniform1i(loc, texture_unit as _);
            }
        } else {
            debug_assert!(false, "Cannot bind texture to pipeline that dosen't exist");
        }

        Ok(())
    }

    fn pipeline_bind_vertex_arrays(
        &mut self,
        pipeline: PipelineId,
        buffers: &[VertexBufferId],
        offsets: &[usize],
        strides: &[i32],
    ) {
        let buffers: Vec<_> = buffers
            .iter()
            .map(|&buffer| self.buffers[&buffer.0].id())
            .collect();

        let pipeline = &self.pipelines[&pipeline.0];
        debug_assert!(pipeline.is_bound);

        if let Some(vert) = pipeline.pipeline.vertex_main() {
            debug_assert!(
                vert.inputs().len() == buffers.len(),
                "Trying to setup incorrect numbers vertex buffers"
            );
        } else {
            debug_assert!(
                false,
                "Trying to apply vertex buffers to pipeline without vertex shader"
            );
        }

        // all slices must be the same length
        debug_assert!(buffers.len() == offsets.len());
        debug_assert!(buffers.len() == strides.len());

        unsafe {
            self.gl.VertexArrayVertexBuffers(
                pipeline.vao,
                0,
                buffers.len() as _,
                buffers.as_ptr(),
                offsets.as_ptr() as _,
                strides.as_ptr() as _,
            );
        }
    }

    fn draw(&mut self, pipeline: PipelineId, mode: DrawingMode, start: u64, count: u64) {
        if let Some(pipeline) = self.pipelines.get(&pipeline.0) {
            debug_assert!(pipeline.is_bound);
        } else {
            debug_assert!(false, "Cannot draw using pipeline that does not exist");
        }

        let mode = match mode {
            DrawingMode::Points => gl::POINTS,
            DrawingMode::Lines => gl::LINES,
            DrawingMode::LineLoop => gl::LINE_LOOP,
            DrawingMode::LineStrip => gl::LINE_STRIP,
            DrawingMode::Triangles => gl::TRIANGLES,
            DrawingMode::TriangleStrip => gl::TRIANGLE_STRIP,
            DrawingMode::TriangleFan => gl::TRIANGLE_FAN,
        };

        unsafe {
            self.gl.DrawArrays(mode, start as _, count as _);
        }
    }

    fn draw_indicies(
        &mut self,
        pipeline: PipelineId,
        mode: DrawingMode,
        indices: IndexBufferId,
        index_type: IndexType,
        index_offset: usize,
        count: usize,
    ) {
        if let Some(pipeline) = self.pipelines.get(&pipeline.0) {
            debug_assert!(pipeline.is_bound);
        } else {
            debug_assert!(false, "Cannot draw using pipeline that does not exist");
        }

        let mode = match mode {
            DrawingMode::Points => gl::POINTS,
            DrawingMode::Lines => gl::LINES,
            DrawingMode::LineLoop => gl::LINE_LOOP,
            DrawingMode::LineStrip => gl::LINE_STRIP,
            DrawingMode::Triangles => gl::TRIANGLES,
            DrawingMode::TriangleStrip => gl::TRIANGLE_STRIP,
            DrawingMode::TriangleFan => gl::TRIANGLE_FAN,
        };

        let index_type = match index_type {
            IndexType::U8 => gl::UNSIGNED_BYTE,
            IndexType::U16 => gl::UNSIGNED_SHORT,
            IndexType::U32 => gl::UNSIGNED_INT,
        };

        self.buffers[&indices.0].bind();

        unsafe {
            self.gl
                .DrawElements(mode, count as _, index_type, index_offset as _);
        }
    }
}

struct GlPipeline {
    gl: gl::Gl,
    program_id: GLuint,
    vao: GLuint,
    pipeline: Program,

    is_bound: bool,
}

impl GlPipeline {
    fn new(mut pipeline: Program, gl: gl::Gl) -> Result<Self> {
        let shaders = pipeline.to_glsl()?;

        let shaders = vec![
            (shaders.vert, gl::VERTEX_SHADER),
            (shaders.frag, gl::FRAGMENT_SHADER),
        ];

        // convert shader source code into gl shader ids
        let shaders = shaders
            .into_iter()
            .filter_map(|(source, kind)| Some((source?, kind)))
            .map::<Result<_, GlError>, _>(|(source, kind)| {
                Ok((
                    CString::new(source).map_err(|e| GlError::ShaderNullByte { message: e })?,
                    kind,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(source, kind)| shader_from_source(&gl, &source, kind))
            .collect::<Result<Vec<_>, _>>()?;

        let program_id = program_from_shaders(&gl, &shaders)?;

        // Create a vao to store the vertex attribute types using OpenGL DSA functions
        // If not using DSA then this would depend on vertex buffers being bound,
        // so would need to be re-done every time the vertex buffers bound are
        // changed, or every draw call if not cached.  That cahching could be
        // implemented if a version of OpenGl older than 4.5 is needed to be supported
        let mut vao = 0;
        unsafe {
            gl.CreateVertexArrays(1, &mut vao);
        }

        if let Some(vert) = pipeline.vertex_main() {
            for (i, attribute) in vert.inputs().iter().enumerate() {
                let name = CString::new(&attribute.name[..])?;

                let count = match attribute.ty {
                    Type::Vector(n) => n,
                    Type::Matrix(n, m) => n * m,
                    Type::Floating => 1,
                    Type::Sampler2D | Type::Unknown => return Err(GlError::OpaqueVerticies.into()),
                };

                unsafe {
                    let location = gl.GetAttribLocation(program_id, name.as_ptr());

                    if location >= 0 {
                        gl.EnableVertexArrayAttrib(vao, location as _);
                        gl.VertexArrayAttribFormat(
                            vao,
                            location as _,
                            count as _,
                            gl::FLOAT,
                            false as _,
                            0,
                        );
                        gl.VertexArrayAttribBinding(vao, location as _, i as _);
                    }
                }
            }
        }

        Ok(GlPipeline {
            program_id,
            vao,
            gl,
            pipeline,
            is_bound: false,
        })
    }

    fn bind(&mut self, gl: &gl::Gl) {
        unsafe {
            gl.UseProgram(self.program_id);
            gl.BindVertexArray(self.vao);
        }
        self.is_bound = true;
    }

    fn unbind(&mut self) {
        self.is_bound = false;
    }
}

impl Drop for GlPipeline {
    fn drop(&mut self) {
        unsafe {
            self.gl.DeleteVertexArrays(1, &self.vao);
            self.gl.DeleteProgram(self.program_id);
        }
    }
}

#[derive(Debug)]
pub struct GlTexture {
    gl: gl::Gl,
    id: GLuint,
    active_index: GLuint,
}

impl GlTexture {
    pub fn new(gl: &gl::Gl, tex: &Texture, index: GLuint) -> Self {
        let config = tex.config();

        let mut texture = 0;
        unsafe {
            gl.ActiveTexture(gl::TEXTURE0 + index);
            gl.GenTextures(1, &mut texture);
            gl.BindTexture(gl::TEXTURE_2D, texture);

            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, wrap_gl(config.wrap_s));
            gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, wrap_gl(config.wrap_t));
            gl.TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_MIN_FILTER,
                min_filter_gl(config.min_filter),
            );
            gl.TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_MAG_FILTER,
                mag_filter_gl(config.mag_filter),
            );

            gl.TexImage2D(
                gl::TEXTURE_2D,
                0,
                internal_format_gl(config.storage),
                tex.width() as _,
                tex.height() as _,
                0,
                format_gl(config.source_format),
                texture_type_gl(config.source_type),
                tex.img_ptr() as _,
            );

            gl.GenerateMipmap(gl::TEXTURE_2D);
        }

        Self {
            gl: gl.clone(),
            id: texture,
            active_index: 0,
        }
    }

    /// Bind this texture to the current shader program.
    pub fn bind(&self, index: GLuint) -> BoundGlTexture {
        BoundGlTexture::new(&self, index)
    }

    pub fn set_bound(&mut self, index: GLuint) {
        self.active_index = index;
        unsafe {
            self.gl.ActiveTexture(gl::TEXTURE0 + index);
            self.gl.BindTexture(gl::TEXTURE_2D, self.id);
        }
    }

    pub fn set_unbound(&mut self) {
        unsafe {
            self.gl.ActiveTexture(gl::TEXTURE0 + self.active_index);
            self.gl.BindTexture(gl::TEXTURE_2D, 0);
        }
        self.active_index = 0;
    }
}

impl Drop for GlTexture {
    /// deletes the texture from vram
    fn drop(&mut self) {
        unsafe {
            self.gl.DeleteTextures(1, &self.id);
        }
    }
}

fn wrap_gl(wrap: WrappingMode) -> GLint {
    match wrap {
        WrappingMode::Repeat => gl::REPEAT as _,
        WrappingMode::MirroredRepeat => gl::MIRRORED_REPEAT as _,
        WrappingMode::ClampToEdge => gl::CLAMP_TO_EDGE as _,
    }
}

fn min_filter_gl(min: MinFilter) -> GLint {
    match min {
        MinFilter::Nearest => gl::NEAREST as _,
        MinFilter::Linear => gl::LINEAR as _,
        MinFilter::NearestMipmapNearest => gl::NEAREST_MIPMAP_NEAREST as _,
        MinFilter::LinearMipmapNearest => gl::LINEAR_MIPMAP_NEAREST as _,
        MinFilter::NearestMipmapLinear => gl::NEAREST_MIPMAP_LINEAR as _,
        MinFilter::LinearMipmapLinear => gl::LINEAR_MIPMAP_LINEAR as _,
    }
}

fn mag_filter_gl(mag: MagFilter) -> GLint {
    match mag {
        MagFilter::Linear => gl::LINEAR as _,
        MagFilter::Nearest => gl::NEAREST as _,
    }
}

fn internal_format_gl(source: TextureStorageType) -> GLint {
    match source {
        TextureStorageType::R => gl::RED as _,
        TextureStorageType::RG => gl::RG as _,
        TextureStorageType::RGB => gl::RGB as _,
        TextureStorageType::SRGB => gl::SRGB as _,
        TextureStorageType::RGBA => gl::RGBA as _,
        TextureStorageType::SRGBA => gl::SRGB_ALPHA as _,
    }
}

fn format_gl(format: TextureSourceFormat) -> GLenum {
    match format {
        TextureSourceFormat::R => gl::RED,
        TextureSourceFormat::RG => gl::RG,
        TextureSourceFormat::RGB => gl::RGB,
        TextureSourceFormat::BGR => gl::BGR,
        TextureSourceFormat::RGBA => gl::RGBA,
        TextureSourceFormat::BGRA => gl::BGRA,
    }
}

fn texture_type_gl(ty: TextureSourceType) -> GLenum {
    match ty {
        TextureSourceType::U8 => gl::UNSIGNED_BYTE,
        TextureSourceType::I8 => gl::BYTE,
        TextureSourceType::U16 => gl::UNSIGNED_SHORT,
        TextureSourceType::I16 => gl::SHORT,
        TextureSourceType::U32 => gl::UNSIGNED_INT,
        TextureSourceType::I32 => gl::INT,
        TextureSourceType::F32 => gl::FLOAT,
    }
}

pub struct BoundGlTexture<'a> {
    tex: &'a GlTexture,
    index: GLuint,
}

impl<'a> BoundGlTexture<'a> {
    fn new(tex: &'a GlTexture, index: GLuint) -> Self {
        unsafe {
            tex.gl.ActiveTexture(gl::TEXTURE0 + index);
            tex.gl.BindTexture(gl::TEXTURE_2D, tex.id);
        }

        Self { tex, index }
    }

    /// Get the index of the texture's active texture unit.
    pub fn index(&self) -> GLuint {
        self.index
    }
}

impl<'a> Drop for BoundGlTexture<'a> {
    fn drop(&mut self) {
        unsafe {
            self.tex.gl.ActiveTexture(gl::TEXTURE0 + self.index);
            self.tex.gl.BindTexture(gl::TEXTURE_2D, 0);
        }
    }
}

#[derive(Debug)]
struct Buffer {
    gl: gl::Gl,
    vbo: GLuint,
    pub buffer_type: GLenum,
}

impl Buffer {
    fn new(gl: &gl::Gl, buffer_type: GLenum) -> Buffer {
        let mut vbo = 0;
        unsafe {
            gl.GenBuffers(1, &mut vbo);
        }

        Buffer {
            gl: gl.clone(),
            vbo,
            buffer_type,
        }
    }

    fn bind(&self) {
        unsafe {
            self.gl.BindBuffer(self.buffer_type, self.vbo);
        }
    }

    fn unbind(&self) {
        unsafe {
            self.gl.BindBuffer(self.buffer_type, 0);
        }
    }

    fn static_draw_data<T>(&self, data: &[T]) {
        unsafe {
            self.gl.BufferData(
                self.buffer_type,
                (data.len() * size_of::<T>()) as GLsizeiptr,
                data.as_ptr() as *const GLvoid,
                gl::STATIC_DRAW,
            );
        }
    }

    fn static_draw_data_stream<T>(&self, data: &[T]) {
        unsafe {
            self.gl.BufferData(
                self.buffer_type,
                (data.len() * size_of::<T>()) as GLsizeiptr,
                data.as_ptr() as *const GLvoid,
                gl::STREAM_DRAW,
            );
        }
    }

    fn id(&self) -> GLuint {
        self.vbo
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            self.gl.DeleteBuffers(1, &self.vbo);
        }
    }
}

/// attach console print debugging to the provided OpenGL Context
fn enable_gl_debugging(gl: &gl::Gl) {
    let mut flags = 0;
    unsafe {
        gl.GetIntegerv(gl::CONTEXT_FLAGS, &mut flags);
    }

    // Only set the debugging options if debugging enabled on the context
    if flags as u32 & gl::CONTEXT_FLAG_DEBUG_BIT == 0 {
        return;
    }

    unsafe {
        // enables debug output
        gl.Enable(gl::DEBUG_OUTPUT);

        // ensure that debugging messages are only output on the main thread
        // ensures that the log function is called in the same order that the
        // messages are generated
        gl.Enable(gl::DEBUG_OUTPUT_SYNCHRONOUS);

        // set the debug call back, with no context pointer
        gl.DebugMessageCallback(Some(gl_debug_log), std::ptr::null());

        // tell the driver that we want all possible debug messages
        gl.DebugMessageControl(
            gl::DONT_CARE,
            gl::DONT_CARE,
            gl::DONT_CARE,
            0,
            std::ptr::null(),
            gl::TRUE,
        );
    }
}

/// Debugging callback
extern "system" fn gl_debug_log(
    source: gl::types::GLenum,
    gltype: gl::types::GLenum,
    id: gl::types::GLuint,
    severity: gl::types::GLenum,
    _length: gl::types::GLsizei,
    message: *const gl::types::GLchar,
    _user_param: *mut gl::types::GLvoid,
) {
    // id of trivial, non error/warning information messages
    // not worth printing, would obscure actual errors
    if id == 0x20071 || id == 0x20084 {
        return;
    }

    println!("----------------");
    println!(
        "OpenGL {1} - {0:#x}:",
        id,
        match gltype {
            gl::DEBUG_TYPE_ERROR => "Error",
            gl::DEBUG_TYPE_DEPRECATED_BEHAVIOR => "Deprecated Behaviour",
            gl::DEBUG_TYPE_UNDEFINED_BEHAVIOR => "Undefined Behaviour",
            gl::DEBUG_TYPE_PORTABILITY => "Portability",
            gl::DEBUG_TYPE_PERFORMANCE => "Performance",
            gl::DEBUG_TYPE_MARKER => "Marker",
            gl::DEBUG_TYPE_PUSH_GROUP => "Push Group",
            gl::DEBUG_TYPE_POP_GROUP => "Pop Group",
            _ => "Other",
        }
    );

    // cast message from null terminated string, to rust types, is
    // guaranteed to be correctly null terminated by the standard,
    // assume that holds
    let message = unsafe { std::ffi::CStr::from_ptr(message) };

    println!("Message: {}", message.to_string_lossy());

    println!(
        "Severity: {}",
        match severity {
            gl::DEBUG_SEVERITY_HIGH => "high",
            gl::DEBUG_SEVERITY_MEDIUM => "medium",
            gl::DEBUG_SEVERITY_LOW => "low",
            gl::DEBUG_SEVERITY_NOTIFICATION => "notification",
            _ => "other",
        }
    );

    println!(
        "Source: {}",
        match source {
            gl::DEBUG_SOURCE_API => "API",
            gl::DEBUG_SOURCE_WINDOW_SYSTEM => "Window System",
            gl::DEBUG_SOURCE_SHADER_COMPILER => "Shader Compiler",
            gl::DEBUG_SOURCE_THIRD_PARTY => "Third Party",
            gl::DEBUG_SOURCE_APPLICATION => "Application",
            _ => "Other",
        }
    );
}

/// Create a new OpenGL shader from glsl source code
fn shader_from_source(gl: &gl::Gl, source: &CStr, kind: GLuint) -> Result<GLuint, GlError> {
    let id = unsafe { gl.CreateShader(kind) };

    unsafe {
        gl.ShaderSource(id, 1, &source.as_ptr(), std::ptr::null());
        gl.CompileShader(id);
    }

    let mut success: GLint = 1;
    unsafe {
        gl.GetShaderiv(id, gl::COMPILE_STATUS, &mut success);
    }

    if success == 0 {
        let mut len = 0;
        unsafe {
            gl.GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut len);
        }

        let error = create_whitespace_cstring(len as usize)?;

        unsafe {
            gl.GetShaderInfoLog(id, len, std::ptr::null_mut(), error.as_ptr() as *mut GLchar);
        }

        return Err(GlError::ShaderCompilation {
            message: error.to_string_lossy().to_string(),
        });
    }

    Ok(id)
}

/// Create a space filled CString of given length
fn create_whitespace_cstring(len: usize) -> Result<CString, GlError> {
    let mut buffer: Vec<u8> = Vec::with_capacity(len + 1);
    buffer.extend([b' '].iter().cycle().take(len));
    CString::new(buffer).map_err(|_| GlError::ErrorBuffer)
}

/// Creates an OpenGl shader program from shader IDs
fn program_from_shaders(gl: &gl::Gl, shaders: &[GLuint]) -> Result<GLuint, GlError> {
    let program_id = unsafe { gl.CreateProgram() };

    for &shader in shaders {
        unsafe { gl.AttachShader(program_id, shader) }
    }

    unsafe { gl.LinkProgram(program_id) };

    let mut success = 1;
    unsafe {
        gl.GetProgramiv(program_id, gl::LINK_STATUS, &mut success);
    }

    if success == 0 {
        let mut len = 0;
        unsafe {
            gl.GetProgramiv(program_id, gl::INFO_LOG_LENGTH, &mut len);
        }

        let error = create_whitespace_cstring(len as usize)?;

        unsafe {
            gl.GetProgramInfoLog(
                program_id,
                len,
                std::ptr::null_mut(),
                error.as_ptr() as *mut GLchar,
            )
        }

        return Err(GlError::ShaderLink {
            message: error.to_string_lossy().to_string(),
        });
    }

    for &shader in shaders {
        unsafe { gl.DetachShader(program_id, shader) }
    }

    Ok(program_id)
}
