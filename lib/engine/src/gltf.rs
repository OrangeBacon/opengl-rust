use std::collections::HashMap;

use anyhow::Result;
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Accessor {
    #[serde(default)]
    pub buffer_view: usize,

    #[serde(default)]
    pub byte_offset: usize,

    component_type: ComponentType,

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

#[derive(Debug, Deserialize_repr)]
#[repr(u32)]
pub enum ComponentType {
    Byte = 5120,
    UnsignedByte = 5121,
    Short = 5122,
    UnsignedShort = 5123,
    UnsignedInt = 5125,
    Float = 5126,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

    pub byte_stride: Option<usize>,
    pub target: Option<usize>,

    pub name: Option<String>,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
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
pub struct GltfModel {
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
    pub attributes: PrimitiveAttr,

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

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
#[serde(deny_unknown_fields)]
pub struct PrimitiveAttr {
    pub position: Option<f32>,
    pub normal: Option<f32>,
    pub tangent: Option<f32>,
    pub texcoord_0: Option<f64>,
    pub texcoord_1: Option<f64>,
    pub texcoord_2: Option<f64>,
    pub color_0: Option<f64>,
    pub joints_0: Option<u32>,
    pub weights_0: Option<f64>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Deserialize_repr)]
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

#[derive(Debug, Deserialize)]
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

    #[serde(default = "default_matrix")]
    pub matrix: [f64; 16],

    pub mesh: Option<usize>,

    #[serde(default = "default_rotation")]
    pub rotation: [f64; 4],

    #[serde(default = "default_scale")]
    pub scale: [f64; 3],

    #[serde(default)]
    pub translation: [f64; 3],

    #[serde(default)]
    pub weights: Vec<f64>,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    #[serde(default)]
    pub extras: Value,
}

fn default_matrix() -> [f64; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn default_rotation() -> [f64; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_scale() -> [f64; 3] {
    [1.0, 1.0, 1.0]
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize_repr)]
#[repr(u32)]
#[serde(rename_all = "UPPERCASE")]
pub enum SamplerMagFilter {
    None = 0,
    Nearest = 9728,
    Linear = 9729,
}

impl Default for SamplerMagFilter {
    fn default() -> Self {
        SamplerMagFilter::None
    }
}

#[derive(Debug, Deserialize_repr)]
#[repr(u32)]
#[serde(rename_all = "UPPERCASE")]
pub enum SamplerMinFilter {
    None = 0,
    Nearest = 9728,
    Linear = 9729,
    NearestMipmapNearest = 9984,
    LinearMipmapNearest = 9985,
    NearestMipmapLinear = 9986,
    LinearMipmapLinear = 9987,
}

impl Default for SamplerMinFilter {
    fn default() -> Self {
        SamplerMinFilter::None
    }
}

#[derive(Debug, Deserialize_repr)]
#[repr(u32)]
#[serde(rename_all = "UPPERCASE")]
pub enum SamplerWrap {
    ClampToEdge = 33071,
    MirroredRepeat = 33648,
    Repeat = 10497,
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

impl GltfModel {
    pub fn from_res(_gl: &gl::Gl, res: &Resources, name: &str) -> Result<Self, Error> {
        let file = res.load_string(name).map_err(|e| Error::Resource {
            name: name.to_string(),
            inner: e,
        })?;

        let model: GltfModel = serde_json::from_str(&file).map_err(|e| Error::Parse {
            name: name.to_string(),
            inner: e,
        })?;

        Ok(model)
    }
}
