//! This file contains a parser for the json gLTF 3D object format
//! It defines a deserialiser into rust structures, however does not do any
//! processing on the file.
//!
//! For more infomation about the file format parsed in this file see
//! https://github.com/KhronosGroup/glTF/blob/master/specification/2.0/README.md

use std::collections::HashMap;

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
    pub uri: String,

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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    #[serde(default)]
    pub uri: String,

    #[serde(default)]
    pub mime_type: String,

    #[serde(default)]
    pub buffer_view: f64,

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

impl Model {
    pub fn from_res(_gl: &gl::Gl, res: &Resources, name: &str) -> Result<Self, Error> {
        let file = res.load_string(name).map_err(|e| Error::Resource {
            name: name.to_string(),
            inner: e,
        })?;

        let model: Model = serde_json::from_str(&file).map_err(|e| Error::Parse {
            name: name.to_string(),
            inner: e,
        })?;

        Ok(model)
    }
}
