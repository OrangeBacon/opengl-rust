//! This file contains a parser for the json gLTF 3D object format
//! It defines a deserialiser into rust structures, however does not do any
//! processing on the file.
//!
//! For more infomation about the file format parsed in this file see
//! https://github.com/KhronosGroup/glTF/blob/master/specification/2.0/README.md

use std::{collections::HashMap, convert::TryInto, io::Read, path::Path, string::FromUtf8Error};

use anyhow::Result;
use gl::types::GLenum;
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;
use serde_repr::Deserialize_repr;
use thiserror::Error;

use crate::resources::{Error as ResourceError, Resources};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Resource loading error from model {name}:\n{inner}")]
    Resource {
        name: String,
        #[source]
        inner: ResourceError,
    },

    #[error("Unable to parse model file {name}:\n{inner}")]
    Parse {
        name: String,
        #[source]
        inner: serde_json::Error,
    },

    #[error("Invalid utf8:\n {inner}")]
    UTF8 {
        #[from]
        #[source]
        inner: FromUtf8Error,
    },

    #[error("GLB file too short")]
    BinaryTooShort,

    #[error("GLB version wrong: expecting v2")]
    BinaryVersion,

    #[error("GLB chunk type error")]
    ChunkError,

    #[error("IO Error: \n{inner}")]
    IoError {
        #[from]
        #[source]
        inner: std::io::Error,
    },
}

impl Model {
    pub fn from_path<T: AsRef<Path>>(path: T) -> Result<Self, Error> {
        let mut file = std::fs::File::open(path.as_ref())?;

        let mut buffer: Vec<u8> = Vec::with_capacity(file.metadata()?.len() as usize + 1);
        file.read_to_end(&mut buffer)?;

        Self::from_bytes(buffer, path)
    }

    /// load a gltf model from a file
    /// res: relative folder to load the model from
    /// name: name of the gltf scene file
    /// to load the referenced data call `crate::Model::new(...)`
    pub fn from_res(res: &Resources, path: &str) -> Result<Self, Error> {
        let file = res.load_bytes(&path).map_err(|e| Error::Resource {
            name: path.to_string(),
            inner: e,
        })?;

        Self::from_bytes(file, path)
    }

    fn from_bytes<T: AsRef<Path>>(data: Vec<u8>, name: T) -> Result<Self, Error> {
        // binary glb files always begin with "glTF", as this is not valid JSON
        // that makes it easy to distinguish between the plain text and binary
        // files, ignoring their file extension
        if data.starts_with(b"glTF") {
            return Model::from_binary(data);
        }

        let data = String::from_utf8(data)?;

        let model: Model = serde_json::from_str(&data).map_err(|e| Error::Parse {
            name: name.as_ref().to_string_lossy().to_string(),
            inner: e,
        })?;

        Ok(model)
    }

    /// convert a glb file into a model
    /// data: the binary data content of the file
    ///
    /// glb files:
    /// https://github.com/KhronosGroup/glTF/blob/master/specification/2.0/README.md#glb-file-format-specification
    /// Chunk based file format
    /// All values are little endian
    /// Header: uint32 magic, uint32 version, uint32 length
    /// magic == 0x46546C67 ("glTF"), version == 2,
    /// length == total length of the file, including the header
    ///
    /// Chunk headers and end must be 4 byte aligned
    /// Chunks: uint32 chunkLength, uint32 chunkType, ubyte[] chunkData
    /// chunkLength == length of the data in the chunk, ignoring the padding
    ///
    /// Chunk 0: type == 0x4E4F534A ("JSON"), JSON text, required
    ///   uses 0x20 (" ") for padding
    /// Chunk 1: type == 0x004E4942 ("\0BIN"), Binary data buffer, optional
    ///   uses 0x00 for padding
    fn from_binary(mut data: Vec<u8>) -> Result<Self, Error> {
        // file header + chunk 0 header length == 24
        if data.len() < 24 {
            return Err(Error::BinaryTooShort);
        }

        // version of the binary file container
        let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
        if version != 2 {
            return Err(Error::BinaryVersion);
        }

        // total data length
        let length = u32::from_le_bytes(data[8..12].try_into().unwrap());
        if length != data.len() as u32 {
            return Err(Error::BinaryTooShort);
        }

        // chunk 0 header
        let chunk_len = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
        let chunk_type = u32::from_le_bytes(data[16..20].try_into().unwrap());

        // chunk 0 must be JSON type
        if chunk_type != 0x4E4F534A {
            return Err(Error::ChunkError);
        }

        // get all data inside chunk 0
        let json = data
            .get(20..(20 + chunk_len))
            .ok_or(Error::BinaryTooShort)?;

        // interpret chunk 0 as a gltf model, same as in the non-binary file
        let json = String::from_utf8(json.to_vec())?;
        let mut model: Model = serde_json::from_str(&json).map_err(|e| Error::Parse {
            name: "GLB file".to_string(),
            inner: e,
        })?;

        // due to alignment, the next chunk header is the previous chunk
        // aligned upwards by four
        let idx = align_up(20 + chunk_len, 4);

        // try to get the length of the next chunk, if it cannot be got, then
        // assume that chunk 1 does not exist, so return the already existing
        // model from chunk 0
        let chunk_len = if let Some(chunk_len) = data.get(idx..(idx + 4)) {
            u32::from_le_bytes(chunk_len.try_into().unwrap()) as usize
        } else {
            return Ok(model);
        };

        // get chunk 1 type
        let chunk_type = if let Some(chunk_type) = data.get((idx + 4)..(idx + 8)) {
            u32::from_le_bytes(chunk_type.try_into().unwrap()) as usize
        } else {
            return Err(Error::BinaryTooShort);
        };

        // chunk 1 must be of type binary
        if chunk_type != 0x004E4942 {
            return Err(Error::ChunkError);
        }

        // try to get the chunk data
        data.drain(0..(idx + 8));
        if data.len() < chunk_len {
            return Err(Error::BinaryTooShort);
        }

        // store the data inside the model
        model.default_buffer = Some(data);

        Ok(model)
    }
}

/// increase a value up to the next multiple of the alignment
fn align_up(val: usize, align: usize) -> usize {
    let remainder = val % align;
    if remainder > 0 {
        val + (align - remainder)
    } else {
        val
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Accessor {
    #[serde(default)]
    pub buffer_view: usize,

    #[serde(default)]
    pub byte_offset: usize,

    pub component_type: ComponentType,

    #[serde(default)]
    pub normalised: bool,

    pub count: usize,

    pub r#type: Type,

    #[serde(default)]
    pub max: Vec<f64>,

    #[serde(default)]
    pub min: Vec<f64>,

    pub sparse: Option<BufferSparse>,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize_repr, Clone)]
#[repr(u32)]
pub enum ComponentType {
    Byte = 5120,
    UnsignedByte = 5121,
    Short = 5122,
    UnsignedShort = 5123,
    UnsignedInt = 5125,
    Float = 5126,
}

impl ComponentType {
    pub fn get_gl_type(&self) -> u32 {
        use ComponentType::*;

        match self {
            Byte => gl::BYTE,
            UnsignedByte => gl::UNSIGNED_BYTE,
            Short => gl::SHORT,
            UnsignedShort => gl::UNSIGNED_SHORT,
            UnsignedInt => gl::UNSIGNED_INT,
            Float => gl::FLOAT,
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Type {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
}

impl Type {
    pub fn component_count(&self) -> i32 {
        use Type::*;

        match self {
            Scalar => 1,
            Vec2 => 2,
            Vec3 => 3,
            Vec4 => 4,
            Mat2 => 4,
            Mat3 => 9,
            Mat4 => 16,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct BufferSparse {
    pub count: usize,
    pub indices: BufferIndices,
    pub values: BufferValues,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct BufferIndices {
    pub buffer_view: usize,

    #[serde(default)]
    pub byte_offset: usize,

    pub component_type: ComponentType,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct BufferValues {
    pub buffer_view: usize,

    #[serde(default)]
    pub byte_offset: usize,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    pub channels: Vec<AnimationChannel>,
    pub samplers: Vec<AnimationSampler>,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationSampler {
    pub input: usize,

    #[serde(default)]
    pub interpolation: AnimationInterpolation,

    pub output: usize,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "UPPERCASE")]
pub enum AnimationInterpolation {
    Linear,
    Step,
    CubicSpline,
}

impl Default for AnimationInterpolation {
    fn default() -> Self {
        AnimationInterpolation::Linear
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationChannel {
    pub sampler: usize,

    pub target: AnimationTarget,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationTarget {
    pub node: Option<usize>,

    pub path: AnimationTargetPath,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnimationTargetPath {
    Translation,
    Rotation,
    Scale,
    Weights,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    #[serde(default)]
    pub copyright: String,

    #[serde(default)]
    pub generator: String,

    pub version: String,

    #[serde(default)]
    pub min_version: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Buffer {
    #[serde(default)]
    pub uri: Option<String>,

    pub byte_length: usize,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct BufferView {
    pub buffer: usize,

    #[serde(default)]
    pub byte_offset: usize,

    pub byte_length: usize,

    pub byte_stride: Option<i32>,
    pub target: Option<BufferViewTarget>,

    pub name: Option<String>,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize_repr, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u32)]
pub enum BufferViewTarget {
    ArrayBuffer = 34962,
    ElementArrayBuffer = 34963,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub orthographic: Option<CameraOrtho>,

    pub perspective: Option<CameraPerspective>,

    pub r#type: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraOrtho {
    pub xmag: f64,
    pub ymag: f64,
    pub zfar: f64,
    pub znear: f64,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct CameraPerspective {
    pub aspect_ratio: Option<f64>,
    pub yfov: f64,
    pub zfar: Option<f64>,
    pub znear: f64,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Model {
    #[serde(default)]
    pub extensions_used: Vec<String>,

    #[serde(default)]
    pub extensions_required: Vec<String>,

    #[serde(default)]
    pub accessors: Vec<Accessor>,

    #[serde(default)]
    pub animations: Vec<Animation>,

    pub asset: Asset,

    #[serde(default)]
    pub buffers: Vec<Buffer>,

    #[serde(default)]
    pub buffer_views: Vec<BufferView>,

    #[serde(default)]
    pub cameras: Vec<Camera>,

    #[serde(default)]
    pub images: Vec<Image>,

    #[serde(default)]
    pub materials: Vec<Material>,

    #[serde(default)]
    pub meshes: Vec<Mesh>,

    #[serde(default)]
    pub nodes: Vec<Node>,

    #[serde(default)]
    pub samplers: Vec<Sampler>,

    pub scene: Option<usize>,

    #[serde(default)]
    pub scenes: Vec<Scene>,

    #[serde(default)]
    pub skins: Vec<Skin>,

    #[serde(default)]
    pub textures: Vec<Texture>,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,

    #[serde(skip)]
    pub default_buffer: Option<Vec<u8>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    #[serde(default)]
    pub uri: Option<String>,

    #[serde(default)]
    pub mime_type: String,

    #[serde(default)]
    pub buffer_view: Option<usize>,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    #[serde(default)]
    pub name: String,

    pub pbr_metallic_roughness: Option<MaterialRoughness>,

    pub normal_texture: Option<TextureNormal>,

    pub occulusion_texture: Option<TextureOcculusion>,

    pub emissive_texture: Option<TextureInfo>,

    #[serde(default)]
    pub emissive_factor: [f64; 3],

    #[serde(default)]
    pub alpha_mode: MaterialAlphaMode,

    #[serde(default = "default_alpha_cutoff")]
    pub alpha_cutoff: f64,

    #[serde(default)]
    pub double_sided: bool,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct MaterialRoughness {
    #[serde(default = "default_color_factor")]
    pub base_color_factor: [f64; 4],

    pub base_color_texture: Option<TextureInfo>,

    #[serde(default = "default_one")]
    pub metallic_factor: f64,

    #[serde(default = "default_one")]
    pub roughness_factor: f64,

    pub metallic_roughness_texture: Option<TextureInfo>,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

fn default_color_factor() -> [f64; 4] {
    [1.0; 4]
}
fn default_one() -> f64 {
    1.0
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MaterialAlphaMode {
    Opaque,
    Mask,
    Blend,
}

impl Default for MaterialAlphaMode {
    fn default() -> Self {
        MaterialAlphaMode::Opaque
    }
}

fn default_alpha_cutoff() -> f64 {
    0.5
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct TextureNormal {
    pub index: usize,

    #[serde(default)]
    pub tex_coord: usize,

    #[serde(default = "default_one")]
    pub scale: f64,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct TextureOcculusion {
    pub index: usize,

    #[serde(default)]
    pub tex_coord: usize,

    #[serde(default = "default_one")]
    pub strength: f64,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mesh {
    pub primitives: Vec<Primitive>,

    #[serde(default)]
    pub weights: Vec<f64>,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Primitive {
    pub attributes: IndexMap<String, i32>,

    pub indices: Option<usize>,

    pub material: Option<usize>,

    #[serde(default)]
    pub mode: PrimitiveMode,

    #[serde(default)]
    pub targets: Vec<PrimitiveTarget>,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize_repr, Clone)]
#[repr(u8)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrimitiveMode {
    Points,
    Lines,
    LineLoop,
    LineStrip,
    Triangles,
    TriangleStrip,
    TriangleFan,
}

impl Default for PrimitiveMode {
    fn default() -> Self {
        PrimitiveMode::Triangles
    }
}

impl PrimitiveMode {
    pub fn to_gl_enum(&self) -> GLenum {
        use PrimitiveMode::*;

        match self {
            Points => gl::POINTS,
            Lines => gl::LINES,
            LineLoop => gl::LINE_LOOP,
            LineStrip => gl::LINE_STRIP,
            Triangles => gl::TRIANGLES,
            TriangleStrip => gl::TRIANGLE_STRIP,
            TriangleFan => gl::TRIANGLE_FAN,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "UPPERCASE")]
pub struct PrimitiveTarget {
    pub position: Option<f64>,
    pub normal: Option<f64>,
    pub tangent: Option<f64>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub camera: Option<usize>,

    #[serde(default)]
    pub children: Vec<usize>,

    pub skin: Option<usize>,

    pub matrix: Option<[f32; 16]>,

    pub mesh: Option<usize>,

    pub rotation: Option<[f32; 4]>,

    pub scale: Option<[f32; 3]>,

    pub translation: Option<[f32; 3]>,

    #[serde(default)]
    pub weights: Vec<f64>,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Sampler {
    #[serde(default)]
    pub mag_filter: SamplerMagFilter,

    #[serde(default)]
    pub min_filter: SamplerMinFilter,

    #[serde(default)]
    pub wrap_s: SamplerWrap,

    #[serde(default)]
    pub wrap_t: SamplerWrap,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize_repr, Clone, Copy)]
#[repr(u32)]
#[serde(rename_all = "UPPERCASE")]
pub enum SamplerMagFilter {
    Nearest = gl::NEAREST,
    Linear = gl::LINEAR,
}

impl Default for SamplerMagFilter {
    fn default() -> Self {
        SamplerMagFilter::Nearest
    }
}

#[derive(Debug, Deserialize_repr, Clone, Copy)]
#[repr(u32)]
#[serde(rename_all = "UPPERCASE")]
pub enum SamplerMinFilter {
    Nearest = gl::NEAREST,
    Linear = gl::LINEAR,
    NearestMipmapNearest = gl::NEAREST_MIPMAP_NEAREST,
    LinearMipmapNearest = gl::LINEAR_MIPMAP_NEAREST,
    NearestMipmapLinear = gl::NEAREST_MIPMAP_LINEAR,
    LinearMipmapLinear = gl::LINEAR_MIPMAP_LINEAR,
}

impl Default for SamplerMinFilter {
    fn default() -> Self {
        SamplerMinFilter::Nearest
    }
}

#[derive(Debug, Deserialize_repr, Clone, Copy)]
#[repr(u32)]
#[serde(rename_all = "UPPERCASE")]
pub enum SamplerWrap {
    ClampToEdge = gl::CLAMP_TO_EDGE,
    MirroredRepeat = gl::MIRRORED_REPEAT,
    Repeat = gl::REPEAT,
}

impl Default for SamplerWrap {
    fn default() -> Self {
        SamplerWrap::Repeat
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    #[serde(default)]
    pub nodes: Vec<usize>,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    pub inverse_bind_matrices: Option<usize>,

    pub skeleton: Option<usize>,

    pub joints: Vec<usize>,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Texture {
    pub sampler: Option<usize>,

    pub source: Option<usize>,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct TextureInfo {
    pub index: usize,

    #[serde(default)]
    pub tex_coord: usize,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}
