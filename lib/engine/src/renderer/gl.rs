use anyhow::Result;
use gl::types::*;
use std::{
    collections::HashMap,
    ffi::{CStr, CString, NulError},
};
use thiserror::Error;

use crate::{
    buffer::Buffer,
    texture::{
        MagFilter, MinFilter, Texture, TextureSourceFormat, TextureSourceType, TextureStorageType,
        WrappingMode,
    },
};

use super::{
    backend::RendererBackend,
    shader::{Program, Type},
    DrawingMode, IdType, IndexBufferId, IndexType, PipelineId, TextureId, VertexBufferId,
};

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

pub struct GlRenderer {
    gl: gl::Gl,

    id: IdType,

    textures: HashMap<IdType, GlTexture>,
    buffers: HashMap<IdType, Buffer>,
    pipelines: HashMap<IdType, GlPipeline>,

    texture_units: Vec<bool>,
    active_textures: HashMap<PipelineId, Vec<usize>>,

    backface_culling_enabled: bool,
}

impl GlRenderer {
    pub fn new(gl: gl::Gl) -> Self {
        if cfg!(debug_assertions) {
            enable_gl_debugging(&gl);
        }

        unsafe { gl.Enable(gl::DEPTH_TEST) }

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
        unsafe {
            self.gl.ClearColor(r, g, b, 1.0);
            self.gl.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }
    }

    fn viewport(&mut self, width: u32, height: u32) {
        unsafe {
            self.gl.Viewport(0, 0, width as _, height as _);
        }
    }

    fn backface_culling(&mut self, enable: bool) {
        // cache whether culling is enabled or not to reduce draw calls
        if enable && !self.backface_culling_enabled {
            unsafe {
                self.gl.Enable(gl::CULL_FACE);
                self.gl.CullFace(gl::BACK);
            }
            self.backface_culling_enabled = true;
        } else if self.backface_culling_enabled {
            unsafe { self.gl.Disable(gl::CULL_FACE) }
            self.backface_culling_enabled = false;
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
        self.textures.remove(&texture.0);
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

    fn unload_vertex_buffer(&mut self, buffer: VertexBufferId) {
        self.buffers.remove(&buffer.0);
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

    fn unload_index_buffer(&mut self, buffer: IndexBufferId) {
        self.buffers.remove(&buffer.0);
    }

    fn load_pipeline(&mut self, pipeline: Program) -> Result<PipelineId> {
        let id = self.id;
        self.id += 1;

        self.pipelines
            .insert(id, GlPipeline::new(pipeline, self.gl.clone())?);

        Ok(PipelineId(id))
    }

    fn unload_pipeline(&mut self, pipeline: PipelineId) {
        self.pipelines.remove(&pipeline.0);
    }

    fn bind_pipeline(&mut self, pipeline: PipelineId) {
        self.active_textures.insert(pipeline, vec![]);
        self.pipelines[&pipeline.0].bind(&self.gl);
    }

    fn unbind_pipeline(&mut self, pipeline: PipelineId) {
        for &texture_unit in &self.active_textures[&pipeline] {
            self.texture_units[texture_unit] = false;
        }

        self.active_textures.remove(&pipeline);
    }

    fn pipeline_bind_matrix(
        &mut self,
        pipeline: PipelineId,
        name: &str,
        matrix: nalgebra_glm::Mat4,
    ) {
        if let Ok(name) = CString::new(name) {
            let pipeline = self.pipelines[&pipeline.0].program_id;
            unsafe {
                let loc = self.gl.GetUniformLocation(pipeline, name.as_ptr());
                self.gl
                    .UniformMatrix4fv(loc, 1, gl::FALSE, matrix.as_slice().as_ptr());
            }
        }
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
        let pipeline = self.pipelines[&pipeline.0].program_id;
        unsafe {
            let loc = self.gl.GetUniformLocation(pipeline, name.as_ptr());
            self.gl.Uniform1i(loc, texture_unit as _);
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

        if let Some(vert) = pipeline.pipeline.vertex_main() {
            if vert.inputs().len() != buffers.len() {
                println!("Error: Trying to setup incorrect vertex buffers");
            }
        }

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

    fn draw(&mut self, _pipeline: PipelineId, mode: DrawingMode, start: u64, count: u64) {
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
        _pipeline: PipelineId,
        mode: DrawingMode,
        indices: IndexBufferId,
        index_type: IndexType,
        index_offset: usize,
        count: usize,
    ) {
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
        })
    }

    fn bind(&self, gl: &gl::Gl) {
        unsafe {
            gl.UseProgram(self.program_id);
            gl.BindVertexArray(self.vao);
        }
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
