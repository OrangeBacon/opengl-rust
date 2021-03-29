use anyhow::Result;
use gl::types::*;
use std::{
    collections::HashMap,
    ffi::{CStr, CString, NulError},
};
use thiserror::Error;

use crate::{
    buffer::Buffer,
    texture::{GlTexture, Texture},
};

use super::{
    backend::RendererBackend, AttributeType, DrawingMode, IdType, IndexBufferId, IndexType,
    Pipeline, PipelineId, TextureId, VertexBufferId,
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
}

pub struct GlRenderer {
    gl: gl::Gl,

    id: IdType,

    textures: HashMap<IdType, GlTexture>,
    buffers: HashMap<IdType, Buffer>,
    pipelines: HashMap<IdType, GlPipeline>,

    texture_units: Vec<bool>,
    active_textures: HashMap<PipelineId, Vec<usize>>,
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
        if enable {
            unsafe {
                self.gl.Enable(gl::CULL_FACE);
                self.gl.CullFace(gl::BACK);
            }
        } else {
            unsafe { self.gl.Disable(gl::CULL_FACE) }
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

    fn load_pipeline(&mut self, pipeline: Pipeline) -> Result<PipelineId> {
        let id = self.id;
        self.id += 1;

        self.pipelines
            .insert(id, GlPipeline::new(pipeline, &self.gl)?);

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

        unsafe {
            self.gl.VertexArrayVertexBuffers(
                self.pipelines[&pipeline.0].vao,
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
    program_id: GLuint,
    vao: GLuint,
}

impl GlPipeline {
    fn new(pipeline: Pipeline, gl: &gl::Gl) -> Result<Self, GlError> {
        let shaders = vec![
            (pipeline.vertex_shader, gl::VERTEX_SHADER),
            (pipeline.fragment_shader, gl::FRAGMENT_SHADER),
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
            .map(|(source, kind)| shader_from_source(gl, &source, kind))
            .collect::<Result<Vec<_>, _>>()?;

        let program_id = program_from_shaders(gl, &shaders)?;

        // Create a vao to store the vertex attribute types using OpenGL DSA functions
        // If not using DSA then this would depend on vertex buffers being bound,
        // so would need to be re-done every time the vertex buffers bound are
        // changed, or every draw call if not cached.  That cahching could be
        // implemented if a version of OpenGl older than 4.5 is needed to be supported
        let mut vao = 0;
        unsafe {
            gl.CreateVertexArrays(1, &mut vao);
        }

        for (i, attribute) in pipeline.attributes.iter().enumerate() {
            let attribute_type = match attribute.item_type {
                AttributeType::I8 => gl::BYTE,
                AttributeType::I16 => gl::SHORT,
                AttributeType::F32 => gl::FLOAT,
                AttributeType::F64 => gl::DOUBLE,
                AttributeType::U8 => gl::UNSIGNED_BYTE,
                AttributeType::U16 => gl::UNSIGNED_SHORT,
                AttributeType::U32 => gl::UNSIGNED_INT,
            };

            let normalised = match attribute.normalised {
                true => gl::TRUE,
                false => gl::FALSE,
            };

            unsafe {
                gl.EnableVertexArrayAttrib(vao, attribute.location);
                gl.VertexArrayAttribFormat(
                    vao,
                    attribute.location,
                    attribute.count as _,
                    attribute_type,
                    normalised,
                    0,
                );
                gl.VertexArrayAttribBinding(vao, attribute.location, i as _);
            }
        }

        Ok(GlPipeline { program_id, vao })
    }

    fn bind(&self, gl: &gl::Gl) {
        unsafe {
            gl.UseProgram(self.program_id);
            gl.BindVertexArray(self.vao);
        }
    }
}

/// attach console print debugging to the provided OpenGL Context
#[cfg(debug_assertions)]
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
#[cfg(debug_assertions)]
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
