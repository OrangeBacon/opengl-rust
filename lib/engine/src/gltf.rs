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

/// Describes a strongly typed view into a buffer view's raw binary data.
/// In OpenGL is used to call vertexAttribPointer correctly
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Accessor {
    /// The buffer view containing the data described in the accessor.
    /// If none, the data should be all zeros.  This can be changed by
    /// extensions or the sparse property
    pub buffer_view: Option<usize>,

    /// The offset into the start of the buffer view that this accessor
    /// starts at, default 0
    #[serde(default)]
    pub byte_offset: usize,

    /// The datatype of the components described by this accessor.  The
    /// unsigned integer component type
    pub component_type: ComponentType,

    /// Should integer data be normalised when it is accessed?
    /// Should only be defined if this accessor is used for vertex attributes
    /// or animation data
    #[serde(default)]
    pub normalised: bool,

    /// The number of components referenced by this accessor
    /// e.g. used in glDrawElements count
    pub count: usize,

    /// Whether the data referenced is scalar, vector or matrix
    pub r#type: Type,

    /// The maximum value of each component.  The type of each item in the
    /// vector should be determined by component_type, the length of the
    /// vector is determined by r#type.  Is not affected by normalisation.
    /// If this is a sparse accessor, the values should be from after the
    /// sparse accessor is applied.
    #[serde(default)]
    pub max: Vec<f32>,

    /// The minimum value of each component.  See the maximim value for more
    /// information about its value.
    #[serde(default)]
    pub min: Vec<f32>,

    /// If the data in this accessor is sparse, then this is describing it
    pub sparse: Option<SparseAccessor>,

    /// The name of this object
    #[serde(default)]
    pub name: String,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// The type of data represented by an accessor.
/// The values are the same as the OpenGl enum values
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
    /// convert a type to the OpenGL enum value for the type
    pub fn gl_type(&self) -> u32 {
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

    pub fn size(&self) -> usize {
        use ComponentType::*;

        match self {
            Byte => 1,
            UnsignedByte => 1,
            Short => 2,
            UnsignedShort => 2,
            UnsignedInt => 4,
            Float => 4,
        }
    }
}

/// How the elements of an accessor are layed out
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
    /// Get the number of components, i.e individual numeric values in
    /// a value of this type.  E.g. Scalar => 1, Vec2 => 2, etc.
    pub fn component_count(&self) -> usize {
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

/// Specifies how a sparse buffer view is accessed, used in Accessors
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SparseAccessor {
    /// The number of entries stored in the array
    pub count: usize,

    /// The indicies modified by the sparse accessor
    pub indices: SparseIndicies,

    /// The values substituted by the sparse accessor
    pub values: SparseValues,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct SparseIndicies {
    /// The buffer view containing the sparce indicies
    pub buffer_view: usize,

    /// The offset into the buffer view in bytes.  Must be aligned.
    #[serde(default)]
    pub byte_offset: usize,

    /// The data stored in the indicies
    pub component_type: ComponentType,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}
/// The values in a sparse accessor for each index
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct SparseValues {
    /// The view containing the values.  Cannot be an array buffer or element
    /// array buffer buffer view
    pub buffer_view: usize,

    /// The offset into the buffer view to start at
    #[serde(default)]
    pub byte_offset: usize,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// A keyframe animation
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    /// The motion channels of the animation
    pub channels: Vec<AnimationChannel>,

    /// The motion start, end and interpolation algorithms
    pub samplers: Vec<AnimationSampler>,

    /// The name of the animation
    #[serde(default)]
    pub name: String,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// An animation input, output and interpolation
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationSampler {
    /// The accessor index for the keyframe's input (time).
    /// Must be a floating point accessor.
    /// The values are time in seconds, time[0] >= 0.0, all values are
    /// strictly increasing
    pub input: usize,

    /// How the keyframes should be interpolated between
    #[serde(default)]
    pub interpolation: AnimationInterpolation,

    /// The accessor index for the keyframe's output values.  If translation
    /// or scale animation, must be float, if rotation or morph animation
    /// must be float or normalised integer.  For weights, each output is scalar
    /// with a count equal to the number of morph targets
    pub output: usize,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// The interpolation mode for an animation
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "UPPERCASE")]
pub enum AnimationInterpolation {
    /// Linear interpolation between frames.  If rotation is animated, uses
    /// spherical linear interpolation.  Output count == input count.
    Linear,

    /// No interpolation, values are constant until the next keyframe.
    /// Output count == input count.
    Step,

    /// Cubic spline with specified tangents.  Output count = 3 * input count
    /// Each output element is [in-tangent, spline vertex, out-tangent].
    /// Minimum two keyframes.
    CubicSpline,
}

impl Default for AnimationInterpolation {
    /// If unspecified, animations are linearly interpolated.
    fn default() -> Self {
        AnimationInterpolation::Linear
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationChannel {
    pub sampler: usize,

    pub target: AnimationTarget,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// What an animation affects
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationTarget {
    /// The index of the node to animate
    pub node: Option<usize>,

    /// The property of the node to animate
    pub path: AnimationTargetPath,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// The target property of an animation
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnimationTargetPath {
    /// translation (x,y,z)
    Translation,

    /// rotation quaternion (x,y,z,w)
    Rotation,

    /// scale (x,y,z)
    Scale,

    /// weights of morph targets
    Weights,
}

/// Metadata about the 3d model
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    /// Copyright message
    #[serde(default)]
    pub copyright: String,

    /// Program that generated the model
    #[serde(default)]
    pub generator: String,

    /// The version of the gLTF standard used for this model
    pub version: String,

    /// The minimum gLTF version supported by the model
    #[serde(default)]
    pub min_version: String,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// A binary data file descriptor
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Buffer {
    /// Where to find the data, can be external file or data uri
    #[serde(default)]
    pub uri: Option<String>,

    /// The length of the buffer in bytes
    pub byte_length: usize,

    /// The name of the buffer
    #[serde(default)]
    pub name: String,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// A view representing a subset of a buffer
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct BufferView {
    /// The index of the buffer
    pub buffer: usize,

    /// The byte offset into the buffer the view starts at
    #[serde(default)]
    pub byte_offset: usize,

    /// The length of the view
    pub byte_length: usize,

    /// The stride of the data in this view in bytes
    pub byte_stride: Option<i32>,

    /// The type of buffer that this view should be bound to
    /// none means it should be infered, or not be a buffer
    pub target: Option<BufferViewTarget>,

    /// The name of this buffer view
    pub name: Option<String>,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// The possible targets of a buffer.  Uses the OpenGL enum values.
#[derive(Debug, Deserialize_repr, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u32)]
pub enum BufferViewTarget {
    ArrayBuffer = 34962,
    ElementArrayBuffer = 34963,
}

/// A camera projection
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    /// Properties for an orthographic camera
    pub orthographic: Option<CameraOrtho>,

    /// Properties for a perspective camera
    pub perspective: Option<CameraPerspective>,

    /// The type of camera represented
    pub r#type: CameraType,

    /// The name of the camera
    #[serde(default)]
    pub name: String,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// The possible camera types
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CameraType {
    Perspective,
    Orthographic,
}

/// An orthographic camera projection
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraOrtho {
    /// horizontal magnification, non zero
    pub xmag: f64,

    /// vertical magnification, non zero
    pub ymag: f64,

    /// distance to the far clipping plane
    pub zfar: f64,

    /// distance to the near clipping plane
    pub znear: f64,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// A perspective projection camera
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct CameraPerspective {
    /// The aspect ratio, if not provided, a default is chosen
    pub aspect_ratio: Option<f64>,

    /// The vertical field of view in radians
    pub yfov: f64,

    /// The distance to the far clipping plane.  If not defined, an infinite
    /// projection matrix must be used
    pub zfar: Option<f64>,

    ///The distance to the near clipping plane
    pub znear: f64,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// The root object containing all other objects in the model
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// Names of extensions used in the model
    #[serde(default)]
    pub extensions_used: Vec<String>,

    /// Names of extensions required to load the asset
    #[serde(default)]
    pub extensions_required: Vec<String>,

    /// Vector of all accessors
    #[serde(default)]
    pub accessors: Vec<Accessor>,

    /// Vector of all keyframe animations
    #[serde(default)]
    pub animations: Vec<Animation>,

    /// Model metadata
    pub asset: Asset,

    /// Vector of binary data buffers
    #[serde(default)]
    pub buffers: Vec<Buffer>,

    /// Vector of views into the buffers
    #[serde(default)]
    pub buffer_views: Vec<BufferView>,

    /// Vector of possible cameras
    #[serde(default)]
    pub cameras: Vec<Camera>,

    /// Vector of images
    #[serde(default)]
    pub images: Vec<Image>,

    /// Vector of materials
    #[serde(default)]
    pub materials: Vec<Material>,

    /// Vector of meshes
    #[serde(default)]
    pub meshes: Vec<Mesh>,

    /// Vector of nodes
    #[serde(default)]
    pub nodes: Vec<Node>,

    /// Vector of samplers
    #[serde(default)]
    pub samplers: Vec<Sampler>,

    /// The default scene index
    pub scene: Option<usize>,

    /// Vector of scenes
    #[serde(default)]
    pub scenes: Vec<Scene>,

    /// Vector of skins
    #[serde(default)]
    pub skins: Vec<Skin>,

    /// Vector of textures
    #[serde(default)]
    pub textures: Vec<Texture>,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,

    #[serde(skip)]
    pub default_buffer: Option<Vec<u8>>,
}

/// Data required to create a texture
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    /// The uri of the texture, could be an external file or data uri
    #[serde(default)]
    pub uri: Option<String>,

    /// The mime type of the image, can be "image/jpeg" or "image/png"
    #[serde(default)]
    pub mime_type: String,

    /// A buffer view that contains the image data, if specified use this
    /// instead of the uri
    #[serde(default)]
    pub buffer_view: Option<usize>,

    /// The name of the image
    #[serde(default)]
    pub name: String,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// The material appearance of a primitive
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    /// The name of the material
    #[serde(default)]
    pub name: String,

    /// The phisically based rendering parameters
    pub pbr_metallic_roughness: Option<MaterialRoughness>,

    /// The normal map texture
    pub normal_texture: Option<TextureNormal>,

    /// The occlusion map texture
    pub occulusion_texture: Option<TextureOcculusion>,

    /// The emissive map texture
    pub emissive_texture: Option<TextureInfo>,

    /// The linear rgb emissive color.  If emissive texture specified,
    /// this value is multiplied with the texel values
    #[serde(default)]
    pub emissive_factor: [f64; 3],

    /// The alpha blending mode to be used
    #[serde(default)]
    pub alpha_mode: MaterialAlphaMode,

    /// If using a mask alpha mode, this is the cutoff threshold, if alpha is
    /// greater or equal to this value it is opaque, otherwise transparant.
    #[serde(default = "default_alpha_cutoff")]
    pub alpha_cutoff: f64,

    /// Should both sides of the model be rendered.  If true, backface-culling
    /// is disabled and double sided lighting used.  The backface must have
    /// normals reversed before lighting.
    #[serde(default)]
    pub double_sided: bool,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// A material's metallic-roughness properties
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct MaterialRoughness {
    /// The base color factor, rgba color, if a base texture is specified, this
    /// is multiplied with the texel values
    #[serde(default = "default_color_factor")]
    pub base_color_factor: [f64; 4],

    /// The base color texture, the rgb components use the sRGB transfer function
    /// If alpha specified, is the linear alpha coverage of the material,
    /// otherwise assumed to be 1.0.  Must not hve premultiplied alpha.
    pub base_color_texture: Option<TextureInfo>,

    /// How metallic the material is, 1.0 == metal, 0.0 == dilectric, in between
    /// is for blending between metalic and dilectric, uses linear interpolation.
    /// If a metallic roughness texture specified, this is multiplied by the
    /// texel values.
    #[serde(default = "default_one")]
    pub metallic_factor: f64,

    /// How rough the material is, 1.0 == completely rough, 0.0 == completely
    /// smooth.  Linear interpolation between the values.  If a metallic roughness
    /// texture specified, this is multiplied by the texel values.
    #[serde(default = "default_one")]
    pub roughness_factor: f64,

    /// The metallic roughness texture, metalness values sampled from the blue
    /// channel, roughness from the green channel, other channels ignored
    pub metallic_roughness_texture: Option<TextureInfo>,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// The default color factor is [1.0, 1.0, 1.0, 1.0]
fn default_color_factor() -> [f64; 4] {
    [1.0; 4]
}

/// The value 1.0 for some defaults
fn default_one() -> f64 {
    1.0
}
/// The alpha blending mode of a material
#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MaterialAlphaMode {
    /// Alpha ignored, fully opaque
    Opaque,

    /// The value is either fully opaque or fully transparant
    Mask,

    /// The alpha is used to composite the source and destination areas
    Blend,
}

impl Default for MaterialAlphaMode {
    /// By default alpha is ignored
    fn default() -> Self {
        MaterialAlphaMode::Opaque
    }
}

/// By default the alpha cutoff is 0.5
fn default_alpha_cutoff() -> f64 {
    0.5
}

/// Infomation about a normal texture
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct TextureNormal {
    /// The index of the texture
    pub index: usize,

    /// The uv coordinates to use for the texture, taken from the mesh primitive
    /// attributes, so a value of 0 means the uv coordinates are from the
    /// TEXCOORD_0 attribute.
    #[serde(default)]
    pub tex_coord: usize,

    /// A scalar multiplier applied to each normal vector of the texture.
    /// Linearly interpolated.
    #[serde(default = "default_one")]
    pub scale: f64,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// Infomation about an occlusion texture
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct TextureOcculusion {
    /// The index of the texture
    pub index: usize,

    /// The uv coordinates to use for the texture, taken from the mesh primitive
    /// attributes, so a value of 0 means the uv coordinates are from the
    /// TEXCOORD_0 attribute.
    #[serde(default)]
    pub tex_coord: usize,

    /// A scalar multiplier controlling the occlusion applied. 0.0 == no
    /// occlusion, 1.0 == fully occluded.  Linearly interpolated.
    #[serde(default = "default_one")]
    pub strength: f64,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// A set of primitives to be rendered
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mesh {
    /// The array of primitives to be rendered
    pub primitives: Vec<Primitive>,

    /// The weights to be applied to morph targets
    #[serde(default)]
    pub weights: Vec<f64>,

    /// The name of the mesh
    #[serde(default)]
    pub name: String,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// A piece of geometry to be rendered with a material
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Primitive {
    /// The attributes of the primitive, maps names to accessor indicies
    pub attributes: IndexMap<String, i32>,

    /// The accessor of the indicies of the primitive
    pub indices: Option<usize>,

    /// The index of the material of the primitive
    pub material: Option<usize>,

    /// The type of primitive described
    #[serde(default)]
    pub mode: PrimitiveMode,

    /// The morph targets for the primitive
    #[serde(default)]
    pub targets: Vec<PrimitiveTarget>,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// How a primitive should be rendered.  See the OpenGL drawing modes for
/// each mode described here.
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
    /// By default triangle rendering is used
    fn default() -> Self {
        PrimitiveMode::Triangles
    }
}

impl PrimitiveMode {
    /// Convert a primitive mode to the relavant OpenGL enum value.
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

/// The morph targets for a primitive
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "UPPERCASE")]
pub struct PrimitiveTarget {
    /// Position morph target
    pub position: Option<f64>,

    /// Normal morph tarrget
    pub normal: Option<f64>,

    /// Tangent morph target
    pub tangent: Option<f64>,
}

/// A node in a scene
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// The index of a camera referenced by the node
    pub camera: Option<usize>,

    /// The indicies of the node's children
    #[serde(default)]
    pub children: Vec<usize>,

    /// The index of the skin referenced by the node.
    pub skin: Option<usize>,

    /// A transformation matrix in column major order
    pub matrix: Option<[f32; 16]>,

    /// The index of the mesh in the node
    pub mesh: Option<usize>,

    /// A rotation quaternion
    pub rotation: Option<[f32; 4]>,

    /// A scale factor
    pub scale: Option<[f32; 3]>,

    /// A translation location
    pub translation: Option<[f32; 3]>,

    /// The morph target weights.  count == count of morph targets used
    #[serde(default)]
    pub weights: Vec<f64>,

    /// The name of the node
    #[serde(default)]
    pub name: String,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// Texture sampler properties
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Sampler {
    /// Magnification filter
    #[serde(default)]
    pub mag_filter: SamplerMagFilter,

    /// Minification filter
    #[serde(default)]
    pub min_filter: SamplerMinFilter,

    /// S wrapping mode
    #[serde(default)]
    pub wrap_s: SamplerWrap,

    /// T wrapping mode
    #[serde(default)]
    pub wrap_t: SamplerWrap,

    /// The name of the sampler
    #[serde(default)]
    pub name: String,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// A sampler magnification filter, based on the OpenGL enum values
#[derive(Debug, Deserialize_repr, Clone, Copy)]
#[repr(u32)]
#[serde(rename_all = "UPPERCASE")]
pub enum SamplerMagFilter {
    Nearest = gl::NEAREST,
    Linear = gl::LINEAR,
}

impl Default for SamplerMagFilter {
    /// Default magnification filter is nearest
    fn default() -> Self {
        SamplerMagFilter::Nearest
    }
}

/// A sampler minification filter, based on the OpenGL enum values
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
    /// The default minification filter is nearest.
    fn default() -> Self {
        SamplerMinFilter::Nearest
    }
}

/// A sampler wrapping mode, based on the OpenGL wrapping mode enum
#[derive(Debug, Deserialize_repr, Clone, Copy)]
#[repr(u32)]
#[serde(rename_all = "UPPERCASE")]
pub enum SamplerWrap {
    ClampToEdge = gl::CLAMP_TO_EDGE,
    MirroredRepeat = gl::MIRRORED_REPEAT,
    Repeat = gl::REPEAT,
}

impl Default for SamplerWrap {
    /// The default wrapping mode is repeat
    fn default() -> Self {
        SamplerWrap::Repeat
    }
}

/// The root nodes of a scene
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    /// The indicies of each root node
    #[serde(default)]
    pub nodes: Vec<usize>,

    /// The name of a scene
    #[serde(default)]
    pub name: String,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// The joints and matricies defining a skin
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    /// Index of the accessor containing the floating point inverse bind
    /// matricies.  Uses an identity matrix by default.
    pub inverse_bind_matrices: Option<usize>,

    /// The index of the node used as a skeleton root.
    pub skeleton: Option<usize>,

    /// The indicies of the nodes in the skeleton, used as joints in the skin.
    /// count == inverse bind matricies count if defined
    pub joints: Vec<usize>,

    /// Name of the skin
    #[serde(default)]
    pub name: String,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// Matches an image to a sampler
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Texture {
    /// Index of a sampler
    pub sampler: Option<usize>,

    /// Index of an image
    pub source: Option<usize>,

    /// Name of the texture
    #[serde(default)]
    pub name: String,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}

/// Reference to a texture
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct TextureInfo {
    /// Texture index
    pub index: usize,

    /// The uv coordinates to use for the texture, taken from the mesh primitive
    /// attributes, so a value of 0 means the uv coordinates are from the
    /// TEXCOORD_0 attribute.
    #[serde(default)]
    pub tex_coord: usize,

    /// Extension specific data
    #[serde(default)]
    pub extensions: HashMap<String, Value>,

    /// Application specific data
    #[serde(default)]
    pub extras: Value,
}
