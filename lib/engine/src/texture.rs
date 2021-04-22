use crate::resources::{Error as ResourceError, Resources};
use anyhow::Result;
use image::GenericImageView;
use thiserror::Error;

/// Errors representing issues loading and decoding images
#[derive(Debug, Error)]
pub enum TextureError {
    /// wrapper error from engine's resource loader
    #[error("Error loading {name}: {source}")]
    ResourceLoad { name: String, source: ResourceError },

    /// image decoding errors
    #[error("Error decoding image: {source}")]
    Decode {
        #[from]
        source: image::ImageError,
    },

    #[error("Unable to convert encoded image into {ty:?}")]
    BadDecodeFormat { ty: TextureSourceType },
}

/// Filtering mode to use when increasing the size of a texture
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MagFilter {
    /// uses the weighted linear blend between nearest adjacent samples
    Linear,

    /// uses the texel closest to the texture coordinate provided
    Nearest,
}

impl Default for MagFilter {
    fn default() -> Self {
        Self::Nearest
    }
}

/// The filtering settings for decreasing the size of an image, e.g.
/// to use when sampling if a texel is larger than a single fragment
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MinFilter {
    /// Take the value of the nearest texel
    Nearest,

    /// linearly filter several adjacent texels
    Linear,

    /// Use the mipmap with the closest size, then use the nearest texel
    NearestMipmapNearest,

    /// Linearly filter the between the nearest mipmaps' nearest texels
    LinearMipmapNearest,

    /// Use the mipmap with the closest size, then linearly filter several adjacent texels
    NearestMipmapLinear,

    /// Linearly filter the between the nearest mipmaps, then linearly filter several adjacent texels
    LinearMipmapLinear,
}

impl Default for MinFilter {
    fn default() -> Self {
        Self::Nearest
    }
}

/// Description of what should be done if sampling outside of [0, 1]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WrappingMode {
    /// The texture coordinate wraps around the texture.
    /// e.g. a texture coordinate of -0.2 becomes the equivalent of 0.8.
    Repeat,

    /// The texture coordinate wraps around, but mirrored.
    /// e.g. a texture coordinate of -0.2 becomes the equivalent of 0.2.
    /// e.g. a texture coordinate of -1.2 becomes the equivalent of 0.8.
    MirroredRepeat,

    /// Clamps the provided texture coordinates to the range [0, 1]
    ClampToEdge,
}

impl Default for WrappingMode {
    fn default() -> Self {
        Self::Repeat
    }
}

/// Description of the pixel data provided
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextureSourceFormat {
    /// A single color component (red), converted to float in [0, 1]
    R,

    /// Two color components (red/green), converted to vec2 float in [0, 1]
    RG,

    /// Three color components (red/green/blue), converted to vec3 float in [0, 1]
    RGB,

    /// Three color components (blue/green/red), converted to vec3 float in [0, 1]
    BGR,

    /// Four color components (red/green/blue/alpha), converted to vec4 float in [0, 1]
    RGBA,

    /// Four color components (blue/green/red/alpha), converted to vec4 float in [0, 1]
    BGRA,
}

impl Default for TextureSourceFormat {
    fn default() -> Self {
        Self::RGBA
    }
}

/// Description of the type of each component in the provided pixel data.
/// Values represent the primitive type with the same name
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextureSourceType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
}

impl Default for TextureSourceType {
    fn default() -> Self {
        Self::U8
    }
}

/// The format of the GPU storage buffer requested
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextureStorageType {
    /// Single component red texture, float [0, 1]
    R,

    /// Two component red/green texture vec2 [0, 1]
    RG,

    /// Three component red/green/blue texture vec3 [0, 1]
    RGB,

    /// Three component red/green/blue texture vec3 [0, 1] where the components are
    /// treated as being in the sRGB color space, instead of the default linear
    /// color space
    SRGB,

    /// Four component red/green/blue/alpha texture vec4 [0, 1]
    RGBA,

    /// Four component red/green/blue/alpha texture vec4 [0, 1] where the components are
    /// treated as being in the sRGB color space, instead of the default linear
    /// color space
    SRGBA,
}

impl Default for TextureStorageType {
    fn default() -> Self {
        Self::RGBA
    }
}

/// The options related to texture loading
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextureOptions {
    /// How the 's' uv coordinate wraps
    pub wrap_s: WrappingMode,

    /// How the 't' uv coordinate wraps
    pub wrap_t: WrappingMode,

    /// Minification filtering setting
    pub min_filter: MinFilter,

    /// Magnification filtering setting
    pub mag_filter: MagFilter,

    /// The layout of the data provided
    pub source_format: TextureSourceFormat,

    /// The type of the data provided
    pub source_type: TextureSourceType,

    /// The pixel width of the image
    pub width: u32,

    /// The pixel height of the image
    pub height: u32,

    /// the format to store the texture as on the GPU
    pub storage: TextureStorageType,
}

/// An image and settings about how to interpret the data
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Texture {
    image: TextureData,
    config: TextureOptions,
}

/// Wrapper to simplify using both 16bit and 8bit textures
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum TextureData {
    U8(Vec<u8>),
    U16(Vec<u16>),
}

impl Texture {
    /// Load a named resource image file using the default settings
    pub fn from_res_encoding(res: &Resources, name: &str) -> Result<Self, TextureError> {
        Self::from_res_encoding_config(res, name, Default::default())
    }

    /// Loads a named resource file using the provided settings
    /// See [`Self::from_encoding_config`] for more information
    pub fn from_res_encoding_config(
        res: &Resources,
        name: &str,
        config: TextureOptions,
    ) -> Result<Self, TextureError> {
        let data = res
            .load_bytes(name)
            .map_err(|e| TextureError::ResourceLoad {
                name: name.to_string(),
                source: e,
            })?;

        Self::from_encoding_config(&data, config)
    }

    /// Loads an encoded image file, tries to detect what sort of file it is, for
    /// details of the detection see [`image`]'s file detection.  This will
    /// ignore the [`TextureOptions::width`], [`TextureOptions::height`] and
    /// [`TextureOptions::source_format`] options provided and instead derives
    /// them from the provided image data.
    pub fn from_encoding_config(
        data: &[u8],
        mut config: TextureOptions,
    ) -> Result<Self, TextureError> {
        let image = image::load_from_memory(data)?;

        config.width = image.width();
        config.height = image.height();
        config.source_format = TextureSourceFormat::RGBA;

        let image = match config.source_type {
            TextureSourceType::U8 => TextureData::U8(image.into_rgba8().into_raw()),
            TextureSourceType::U16 => TextureData::U16(image.into_rgba16().into_raw()),
            _ => {
                return Err(TextureError::BadDecodeFormat {
                    ty: config.source_type,
                })
            }
        };

        Ok(Texture { image, config })
    }

    /// Loads an image from a slice of raw 8 bit pixel data using the provided settings.
    /// Creates a copy of the pixel data, if possible prefer [`Self::from_raw_owned_config`]
    /// to prevent the copy.
    pub fn from_raw_config(data: &[u8], config: TextureOptions) -> Result<Self, TextureError> {
        Self::from_raw_owned_config(data.into(), config)
    }

    /// Loads an image from raw 8 bit pixel data using the provided settings
    pub fn from_raw_owned_config(
        data: Vec<u8>,
        config: TextureOptions,
    ) -> Result<Self, TextureError> {
        Ok(Self {
            image: TextureData::U8(data),
            config,
        })
    }

    /// Loads an image from a slice of raw 16 bit pixel data using the provided settings.
    /// Creates a copy of the pixel data, if possible prefer [`Self::from_raw_owned_config`]
    /// to prevent the copy.
    pub fn from_raw_config16(data: &[u16], config: TextureOptions) -> Result<Self, TextureError> {
        Self::from_raw_owned_config16(data.into(), config)
    }

    /// Loads an image from raw 16 bit pixel data using the provided settings
    pub fn from_raw_owned_config16(
        data: Vec<u16>,
        config: TextureOptions,
    ) -> Result<Self, TextureError> {
        Ok(Self {
            image: TextureData::U16(data),
            config,
        })
    }

    /// get the width of the image in pixels
    pub fn width(&self) -> u32 {
        self.config.width
    }

    /// get the height of the image in pixels
    pub fn height(&self) -> u32 {
        self.config.height
    }

    /// get a void pointer to the image data for loading into graphics apis
    pub fn img_ptr(&self) -> *const std::ffi::c_void {
        match &self.image {
            TextureData::U8(a) => a.as_ptr() as _,
            TextureData::U16(a) => a.as_ptr() as _,
        }
    }

    /// get the configuration settings used when creating the image
    pub fn config(&self) -> TextureOptions {
        self.config
    }
}
