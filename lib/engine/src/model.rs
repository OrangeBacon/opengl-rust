use std::{collections::HashMap, convert::TryInto};

use crate::{
    bound::Bounds,
    buffer, gltf,
    renderer::{self, IndexBufferId, Renderer, VertexBufferId},
    resources::{Error as ResourceError, Resources},
    texture,
    texture::{GlTexture, Texture},
    DynamicShader, EngineStateRef, Program,
};
use anyhow::Result;
use gl::types::{GLenum, GLsizei};
use nalgebra_glm as glm;
use slotmap::{DefaultKey, SlotMap};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Both matrix and trs properties supplied on one node")]
    DuplicateTransform,

    #[error("Error loading model buffer {name}: \n{inner}")]
    BufferLoad {
        name: String,
        #[source]
        inner: ResourceError,
    },

    #[error("Buffer read is too short: got {len} expected {expected}")]
    BufferLength { len: usize, expected: usize },

    #[error("Unable to get item {got} from {array}, the maximum is {max}")]
    BadIndex {
        max: usize,
        got: usize,
        array: &'static str,
    },

    #[error("No target specified for buffer")]
    NoTarget,

    #[error("Unable to map buffer view to buffer tried to get {get}, max is {max}")]
    BadViewLen { get: usize, max: usize },

    #[error("Mesh does not contain position data")]
    NoPositions,

    #[error("Bad vertex attribute lengths")]
    AttribLen,

    #[error("Generated shader contains nul byte")]
    NullShader,

    #[error("Shader Compilation error: \n{error}")]
    ShaderCompile { error: String },

    #[error("Shader link error: \n{error}")]
    ShaderLink { error: String },

    #[error("Error loading image {name}: {inner}")]
    ImageLoad {
        name: String,
        #[source]
        inner: ResourceError,
    },

    #[error("Error loading texture into vram: {inner}")]
    Texture {
        #[source]
        inner: crate::texture::Error,
    },

    #[error("No image provided for texture")]
    NoImage,

    #[error("Could not get internal buffer")]
    InternalBuffer,

    #[error("No buffer view or uri defined on image")]
    NoSource,

    #[error("Error while loading model: {inner}")]
    Gltf {
        #[source]
        inner: gltf::Error,
    },

    #[error("Could not get root path of scene: path = \"{inner}\"")]
    RootPath { inner: String },

    #[error("Multiple types required for buffer view")]
    BufferViewType,

    #[error("Unable to infer bufferview type")]
    BufferViewNoInference,

    #[error("Unable to use floats as sparce indicies")]
    SparseFloat,

    #[error("Malformed sparse indicies")]
    SparseIndicies,

    #[error("Unmatched sparse index to value count")]
    SparseIndexValues,
}

/// A 3d gltf model, including all its data.  Not dependant upon any rendering
/// backend
#[derive(Debug)]
pub struct Model {
    scenes: Vec<Scene>,

    buffers: Vec<Buffer>,

    textures: Vec<Texture>,

    buffer_view_types: Vec<BufferViewType>,

    pub(crate) gltf: gltf::Model,

    gpu_buffers: Vec<GPUBuffer>,
    gpu_textures: Vec<renderer::TextureId>,
    gpu_pipelines: Vec<renderer::PipelineId>,
}

impl Model {
    pub fn from_res(res: &Resources, path: &str, renderer: &mut Renderer) -> Result<Self, Error> {
        let parent = res.extend_file_root(path).ok_or(Error::RootPath {
            inner: path.to_string(),
        })?;

        let gltf = gltf::Model::from_res(res, path).map_err(|e| Error::Gltf { inner: e })?;

        let model = Model::from_gltf(gltf, &parent, renderer)?;

        Ok(model)
    }

    pub fn from_gltf(
        mut gltf: gltf::Model,
        res: &Resources,
        renderer: &mut Renderer,
    ) -> Result<Self, Error> {
        let mut default_buffer = gltf.default_buffer.take();

        let buffers = gltf
            .buffers
            .iter()
            .map(|buffer| Buffer::new(buffer, &res, &mut default_buffer))
            .collect::<Result<Vec<_>, _>>()?;

        let images = gltf
            .images
            .iter()
            .map(|img| Model::load_image(img, &res, &gltf, &buffers))
            .collect::<Result<Vec<_>, _>>()?;

        let textures = gltf
            .textures
            .iter()
            .map(|tex| Model::load_texture(tex, &gltf, &images))
            .collect::<Result<_, _>>()?;

        let scenes = gltf
            .scenes
            .iter()
            .map(|scene| Scene::new(scene, &gltf))
            .collect::<Result<_, _>>()?;

        let mut model = Model {
            buffers,
            scenes,
            gltf,
            textures,
            buffer_view_types: Vec::with_capacity(0),
            gpu_buffers: Vec::with_capacity(0),
            gpu_textures: Vec::with_capacity(0),
            gpu_pipelines: Vec::with_capacity(0),
        };

        model.check_load_accessors()?;
        model.buffer_view_types = BufferViewType::derive_types(&model.gltf)?;
        model.load_gpu(renderer, res)?;

        Ok(model)
    }

    fn load_image(
        img: &gltf::Image,
        res: &Resources,
        gltf: &gltf::Model,
        buffers: &[Buffer],
    ) -> Result<Vec<u8>, Error> {
        let data = match img.uri {
            Some(ref uri) => res.load_bytes(uri).map_err(|e| Error::ImageLoad {
                name: uri.to_string(),
                inner: e,
            })?,
            None => {
                if let Some(buffer_view) = img.buffer_view {
                    let view = gltf.buffer_views.get(buffer_view).ok_or(Error::BadIndex {
                        array: "buffer views",
                        got: buffer_view,
                        max: gltf.buffer_views.len(),
                    })?;

                    let buffer = buffers.get(view.buffer).ok_or(Error::BadIndex {
                        array: "buffers",
                        got: view.buffer,
                        max: gltf.buffers.len(),
                    })?;

                    let data = buffer
                        .data
                        .get(view.byte_offset..(view.byte_offset + view.byte_length))
                        .ok_or(Error::BadIndex {
                            array: "buffers",
                            got: view.buffer,
                            max: gltf.buffers.len(),
                        })?;

                    data.to_vec()
                } else {
                    return Err(Error::NoSource);
                }
            }
        };

        Ok(data)
    }

    fn load_texture(
        tex: &gltf::Texture,
        gltf: &gltf::Model,
        images: &[Vec<u8>],
    ) -> Result<Texture, Error> {
        let default = gltf::Sampler::default();
        let sampler = if let Some(idx) = tex.sampler {
            &gltf.samplers[idx]
        } else {
            &default
        };

        let sampler = texture::Sampler {
            wrap_s: sampler.wrap_s as _,
            wrap_t: sampler.wrap_t as _,
            min_filter: sampler.min_filter as _,
            mag_filter: sampler.mag_filter as _,
        };

        let source = tex.source.ok_or(Error::NoImage)?;
        let data = &images[source];

        let tex =
            Texture::load_from_bytes(data, sampler).map_err(|e| Error::Texture { inner: e })?;

        Ok(tex)
    }

    pub fn get_bounds(&self) -> Bounds {
        let mut bound = Bounds::new_nan();
        let scene = &self.scenes[0];

        for node in &scene.root_nodes {
            bound.merge(&scene.nodes[*node].get_bounds(&scene.nodes, &self.gltf));
        }

        bound
    }

    /// Try to apply all the sparse accessors to the model data, so the graphics
    /// backend doesn't have to handle them
    fn check_load_accessors(&mut self) -> Result<(), Error> {
        // check each accessor to see if it is sparse
        for accessor in &self.gltf.accessors {
            let sparse = match &accessor.sparse {
                Some(s) => s,
                _ => continue,
            };

            // there isn't always a buffer view, if not present a buffer of
            // zeros should be created to apply the sparse accessor to, so create
            // a new buffer view and buffer to store this new data
            let buffer_view = match accessor.buffer_view {
                Some(idx) => idx,
                None => {
                    let size = accessor.component_type.size()
                        * accessor.r#type.component_count()
                        * accessor.count;
                    self.buffers.push(Buffer {
                        data: vec![0; size],
                    });

                    let view = gltf::BufferView {
                        buffer: self.buffers.len() - 1,
                        byte_length: size,
                        byte_stride: None,
                        byte_offset: 0,
                        extensions: HashMap::default(),
                        extras: serde_json::Value::Null,
                        name: None,
                        target: None,
                    };

                    self.gltf.buffer_views.push(view);

                    self.gltf.buffer_views.len() - 1
                }
            };

            let value_size = accessor.component_type.size() * accessor.r#type.component_count();

            // get the indicies buffer view, then offset it with the infomation
            // from the sparse accessor offsetting the data
            let indicies = self.buffer_data(sparse.indices.buffer_view)?;
            let indicies = indicies
                .get(
                    sparse.indices.byte_offset
                        ..(sparse.indices.byte_offset
                            + sparse.count * sparse.indices.component_type.size()),
                )
                .ok_or(Error::BadIndex {
                    array: "Sparse indicies",
                    got: sparse.indices.byte_offset + sparse.count,
                    max: indicies.len(),
                })?
                .to_owned();

            // get the valaues buffer view, then offset it with the infomation
            // from the sparse accessor offsetting the data
            let values = self.buffer_data(sparse.values.buffer_view)?.to_owned();
            let values = values
                .get(
                    sparse.values.byte_offset
                        ..(sparse.values.byte_offset + sparse.count * value_size),
                )
                .ok_or(Error::BadIndex {
                    array: "Sparse values",
                    got: sparse.values.byte_offset + sparse.count,
                    max: indicies.len(),
                })?
                .to_owned();

            // get the mutable data to be edited from the model
            let data =
                Self::buffer_data_mut(&self.gltf.buffer_views, &mut self.buffers, buffer_view)?;
            let data_len = data.len();

            // get the relevant portion of the data buffer
            let data = data
                .get_mut(accessor.byte_offset..(accessor.byte_offset + accessor.count * value_size))
                .ok_or(Error::BadIndex {
                    array: "Sparse accessor data",
                    got: accessor.byte_offset + accessor.count,
                    max: data_len,
                })?;

            // get the stride, if it is not present, assume the stride is the same
            // as the value size.  If the code has run to this point, it
            // is gauranteed that the relavant buffer view exists so don't
            // need to check the access.
            let stride = if let Some(v) = self.gltf.buffer_views[buffer_view].byte_stride {
                v as usize
            } else {
                value_size
            };

            // macro to simplify generating the acessor processor for each possible
            // index type
            macro_rules! process {
                ($convert:ty) => {
                    process_accessor(
                        // unwrap here is safe as process accessor will only call
                        // this function with a slice that is the correct length
                        |a| <$convert>::from_le_bytes(a.try_into().unwrap()) as usize,
                        std::mem::size_of::<$convert>(),
                        value_size,
                        data,
                        &indicies,
                        &values,
                        stride,
                    )?
                };
            };

            // dispatch the correct accessor process
            match sparse.indices.component_type {
                gltf::ComponentType::Byte => process!(i8),
                gltf::ComponentType::UnsignedByte => process!(u8),
                gltf::ComponentType::Short => process!(i16),
                gltf::ComponentType::UnsignedShort => process!(u16),
                gltf::ComponentType::UnsignedInt => process!(u32),
                gltf::ComponentType::Float => return Err(Error::SparseFloat),
            }
        }

        Ok(())
    }

    // try to get a reference to the data from a buffer view
    fn buffer_data(&self, buffer_view: usize) -> Result<&[u8], Error> {
        let view = self
            .gltf
            .buffer_views
            .get(buffer_view)
            .ok_or_else(|| Error::BadIndex {
                array: "Buffer views",
                got: buffer_view,
                max: self.gltf.buffer_views.len(),
            })?;

        let buffer = self
            .buffers
            .get(view.buffer)
            .ok_or_else(|| Error::BadIndex {
                array: "Buffers",
                got: view.buffer,
                max: self.buffers.len(),
            })?;

        Ok(buffer
            .data
            .get(view.byte_offset..(view.byte_offset + view.byte_length))
            .ok_or_else(|| Error::BadIndex {
                array: "Buffer get",
                got: view.byte_offset + view.byte_length,
                max: buffer.data.len(),
            })?)
    }

    // try to get the mutable data for a buffer view
    // doesn't take self, instead takes slices so the borrows on self can be
    // split to allow both mut and immutable references to parts of self
    fn buffer_data_mut<'a>(
        buffer_views: &[gltf::BufferView],
        buffers: &'a mut [Buffer],
        buffer_view: usize,
    ) -> Result<&'a mut [u8], Error> {
        let view = buffer_views
            .get(buffer_view)
            .ok_or_else(|| Error::BadIndex {
                array: "Buffer views",
                got: buffer_view,
                max: buffer_views.len(),
            })?;

        let buffer_len = buffers.len();

        let buffer = buffers.get_mut(view.buffer).ok_or(Error::BadIndex {
            array: "Buffers",
            got: view.buffer,
            max: buffer_len,
        })?;

        let buffer_len = buffer.data.len();

        Ok(buffer
            .data
            .get_mut(view.byte_offset..(view.byte_offset + view.byte_length))
            .ok_or(Error::BadIndex {
                array: "Buffer get",
                got: view.byte_offset + view.byte_length,
                max: buffer_len,
            })?)
    }

    /// load a 3d model onto the GPU
    fn load_gpu(&mut self, renderer: &mut Renderer, res: &Resources) -> Result<(), Error> {
        self.gpu_buffers = self
            .gltf
            .buffer_views
            .iter()
            .zip(&self.buffer_view_types)
            .map(|(view, &view_type)| GPUBuffer::new(renderer, self, view, view_type))
            .collect();

        let images = self
            .gltf
            .images
            .iter()
            .map(|img| Model::load_image(img, &res, &self.gltf, &self.buffers))
            .collect::<Result<Vec<_>, _>>()?;

        let textures = self
            .gltf
            .textures
            .iter()
            .map(|tex| Model::load_texture(tex, &self.gltf, &images))
            .collect::<Result<Vec<_>, _>>()?;

        self.gpu_textures = textures
            .into_iter()
            .map(|tex| renderer.load_texture(tex))
            .collect();

        Ok(())
    }
}

// process a single sparse accessor
fn process_accessor<F>(
    // a function that converts bytes to usize depending upon the type of
    // indicies specified
    to_usize: F,

    // the number of bytes to pass to the to_usize function
    idx_size: usize,

    // number of bytes in each substituted value
    value_size: usize,

    // the data to apply the accessor to
    data: &mut [u8],

    // the indicies into the data to apply values at
    indicies: &[u8],

    // the values to be applied
    values: &[u8],

    // the stride of the data
    stride: usize,
) -> Result<(), Error>
where
    F: Fn(&[u8]) -> usize,
{
    if indicies.len() % idx_size != 0 {
        return Err(Error::SparseIndicies);
    }

    if indicies.len() / idx_size != values.len() / value_size {
        return Err(Error::SparseIndexValues);
    }

    let data_len = data.len();

    let iter = indicies.chunks(idx_size).zip(values.chunks(value_size));

    // loop through all possible sparse substitutions
    for (idx, value) in iter {
        let idx = to_usize(idx);
        let byte_offset = stride * idx;

        let data = data
            .get_mut(byte_offset..(byte_offset + value_size))
            .ok_or(Error::BadIndex {
                array: "Sparse data",
                got: byte_offset + value_size,
                max: data_len,
            })?;

        // replace the data in the buffer
        for (data, new) in data.iter_mut().zip(value) {
            *data = *new;
        }
    }

    Ok(())
}

/// How a buffer view is going to be used in the model
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BufferViewType {
    /// No usage found for the buffer view
    None,

    /// The buffer view is needed on the CPU, possibly a texture to decode
    CPUBuffer,

    /// The buffer view is needed as an OpenGL ArrayBuffer or equivalent.
    /// E.g. vertex attributes
    ArrayBuffer,

    /// The buffer view is needed as an OpenGL ElementArrayBuffer or
    /// equivalent. E.g. vertex indicies for DrawElements
    ElementArrayBuffer,
}

/// As the methods in here are called on load, rather than on render, they
/// need to use checked array access so the program does not crash if
/// a bad model is loaded.
impl BufferViewType {
    /// Attempt to derive all the buffer view types for a gltf model.
    /// Errors if multiple types are derived for one buffer view or if no type
    /// is derived for the buffer view.  The latter usually indicates that that
    /// part of the specification hasn't been implemented yet.
    fn derive_types(model: &gltf::Model) -> Result<Vec<Self>, Error> {
        let mut types = vec![Self::None; model.buffer_views.len()];

        // derive mesh primative types
        for mesh in &model.meshes {
            for prim in &mesh.primitives {
                Self::derive_prim(prim, model, &mut types)?;
            }
        }

        // Set image buffers to CPU usage only
        for image in &model.images {
            if let Some(view) = image.buffer_view {
                let view = types.get_mut(view).ok_or_else(|| Error::BadIndex {
                    array: "buffer views",
                    got: view,
                    max: model.buffer_views.len(),
                })?;

                Self::set_view(view, Self::CPUBuffer)?;
            }
        }

        // Some buffer views have a buffer type specified in the model format,
        // verify that the provided type matches the infered type or if no
        // type infered, take the type provided.  The last point is why this is
        // required, not a verification step.
        for (idx, view) in model.buffer_views.iter().enumerate() {
            if let Some(target) = view.target {
                let view_type = match target {
                    gltf::BufferViewTarget::ElementArrayBuffer => Self::ElementArrayBuffer,
                    gltf::BufferViewTarget::ArrayBuffer => Self::ArrayBuffer,
                };

                let view = types.get_mut(idx).ok_or_else(|| Error::BadIndex {
                    array: "buffer views",
                    got: idx,
                    max: model.buffer_views.len(),
                })?;

                Self::set_view(view, view_type)?;
            }
        }

        // count the number of not-infered buffers
        let none_count = types.iter().filter(|&&x| x == Self::None).count();

        if none_count > 0 {
            return Err(Error::BufferViewNoInference);
        }

        Ok(types)
    }

    /// Derive the types of buffer for a mesh primative
    fn derive_prim(
        prim: &gltf::Primitive,
        model: &gltf::Model,
        types: &mut [Self],
    ) -> Result<(), Error> {
        // set the vertex attributes to be array buffers
        for (_, &attr_id) in &prim.attributes {
            let accessor =
                model
                    .accessors
                    .get(attr_id as usize)
                    .ok_or_else(|| Error::BadIndex {
                        array: "accessors",
                        got: attr_id as usize,
                        max: model.accessors.len(),
                    })?;
            Self::set_accessor(accessor, types, Self::ArrayBuffer)?;
        }

        // set the vertex indicies to be an element array buffer
        if let Some(indicies) = prim.indices {
            let accessor =
                model
                    .accessors
                    .get(indicies as usize)
                    .ok_or_else(|| Error::BadIndex {
                        array: "accessors",
                        got: indicies as usize,
                        max: model.accessors.len(),
                    })?;
            Self::set_accessor(accessor, types, Self::ElementArrayBuffer)?;
        }

        Ok(())
    }

    /// Set the buffer views referenced by an accessor to have the buffer view
    /// type passed in, or to be CPU buffers if referenced as part of a sparse
    /// accessor.
    fn set_accessor(accessor: &gltf::Accessor, types: &mut [Self], set: Self) -> Result<(), Error> {
        // calculate length, required for errors due to get_mut + lifetimes
        let type_len = types.len();

        // Set the buffer view to the requested type, if the accessor has
        // a buffer view
        if let Some(view) = accessor.buffer_view {
            let view = types.get_mut(view).ok_or(Error::BadIndex {
                array: "buffer views",
                got: view,
                max: type_len,
            })?;

            Self::set_view(view, set)?;
        }

        // If there is a sparse accessor, set its views to be CPU buffers
        if let Some(sparse) = &accessor.sparse {
            let indicies = sparse.indices.buffer_view;
            let view = types.get_mut(indicies).ok_or(Error::BadIndex {
                array: "buffer views",
                got: indicies,
                max: type_len,
            })?;

            Self::set_view(view, Self::CPUBuffer)?;

            let values = sparse.values.buffer_view;
            let view = types.get_mut(values).ok_or(Error::BadIndex {
                array: "buffer views",
                got: values,
                max: type_len,
            })?;

            Self::set_view(view, Self::CPUBuffer)?;
        }

        Ok(())
    }

    /// Try to set a view to have a provided type, if it already has a different
    /// type, it will error.
    fn set_view(view: &mut Self, set: Self) -> Result<(), Error> {
        match view {
            Self::None => *view = set,
            _ if *view == set => (),
            _ => return Err(Error::BufferViewType),
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Buffer {
    data: Vec<u8>,
}

impl Buffer {
    fn new(
        buffer: &gltf::Buffer,
        res: &Resources,
        default: &mut Option<Vec<u8>>,
    ) -> Result<Self, Error> {
        let bytes = match buffer.uri {
            Some(ref uri) => res.load_bytes(&uri).map_err(|e| Error::BufferLoad {
                name: uri.clone(),
                inner: e,
            })?,
            None => match default.take() {
                Some(data) => data,
                None => return Err(Error::InternalBuffer),
            },
        };

        if bytes.len() < buffer.byte_length {
            return Err(Error::BufferLength {
                len: bytes.len(),
                expected: buffer.byte_length,
            });
        }

        Ok(Buffer { data: bytes })
    }
}

#[derive(Debug)]
pub struct Scene {
    root_nodes: Vec<DefaultKey>,
    nodes: SlotMap<DefaultKey, Node>,
}

impl Scene {
    fn new(scene: &gltf::Scene, gltf: &gltf::Model) -> Result<Self, Error> {
        let mut nodes = SlotMap::new();

        let root_nodes = scene
            .nodes
            .iter()
            .map(|&node_id| {
                Ok(Node::new(
                    gltf.nodes.get(node_id).ok_or_else(|| Error::BadIndex {
                        array: "Nodes",
                        got: node_id,
                        max: gltf.nodes.len(),
                    })?,
                    None,
                    gltf,
                    &mut nodes,
                )?)
            })
            .collect::<Result<_, _>>()?;

        Ok(Scene { root_nodes, nodes })
    }
}

#[derive(Debug, Default)]
pub struct Node {
    children: Vec<DefaultKey>,
    parent: Option<DefaultKey>,
    local_matrix: glm::Mat4,
    global_matrix: glm::Mat4,
    mesh_id: Option<usize>,
}

impl Node {
    fn new(
        node: &gltf::Node,
        parent: Option<DefaultKey>,
        gltf: &gltf::Model,
        nodes: &mut SlotMap<DefaultKey, Node>,
    ) -> Result<DefaultKey, Error> {
        let this_key = nodes.insert(Node::default());
        nodes[this_key].mesh_id = node.mesh;
        nodes[this_key].parent = parent;
        // process this node

        nodes[this_key].local_matrix = Node::get_matrix(node)?;

        let parent_mat = if let Some(parent) = parent {
            nodes[parent].global_matrix
        } else {
            glm::Mat4::identity()
        };

        nodes[this_key].global_matrix = parent_mat * nodes[this_key].local_matrix;

        // recursively process all children

        let children = node
            .children
            .iter()
            .map(|&node_id| {
                Ok(Node::new(
                    gltf.nodes.get(node_id).ok_or_else(|| Error::BadIndex {
                        array: "Nodes",
                        got: node_id,
                        max: gltf.nodes.len(),
                    })?,
                    Some(this_key),
                    gltf,
                    nodes,
                )?)
            })
            .collect::<Result<Vec<_>, _>>()?;
        nodes[this_key].children = children;

        Ok(this_key)
    }

    fn get_matrix(node: &gltf::Node) -> Result<glm::Mat4, Error> {
        let mut matrix = glm::Mat4::identity();

        if let Some(m) = node.matrix {
            matrix.copy_from_slice(&m);

            if node.translation.is_some() || node.rotation.is_some() || node.scale.is_some() {
                return Err(Error::DuplicateTransform);
            }
            return Ok(matrix);
        }

        let translation = node.translation.unwrap_or_default();
        let rotation = node.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let scale = node.scale.unwrap_or([1.0, 1.0, 1.0]);

        let translation = glm::translate(&matrix, &glm::Vec3::from(translation));
        let rotation = glm::quat_to_mat4(&glm::Quat::from(rotation));
        let scale = glm::scale(&matrix, &glm::Vec3::from(scale));

        Ok(translation * rotation * scale)
    }

    fn get_bounds(&self, nodes: &SlotMap<DefaultKey, Node>, model: &gltf::Model) -> Bounds {
        let mut bound = Bounds::new_nan();

        if let Some(mesh) = self.mesh_id {
            let mesh = &model.meshes[mesh];

            for prim in &mesh.primitives {
                if let Some(&pos) = prim.attributes.get("POSITION") {
                    let pos = &model.accessors[pos as usize];
                    bound.merge(
                        &Bounds::from_slice(&pos.min, &pos.max).apply_mat(&self.local_matrix),
                    );
                }
            }
        }

        for node in &self.children {
            bound.merge(
                &nodes[*node]
                    .get_bounds(nodes, model)
                    .apply_mat(&self.local_matrix),
            );
        }

        bound
    }
}

/// A loaded buffer, either an index buffer, a vertex buffer or neither for specifying
/// a CPU only buffer
#[derive(Debug)]
enum GPUBuffer {
    Index(IndexBufferId),
    Vertex(VertexBufferId),
    None,
}

impl GPUBuffer {
    fn new(
        renderer: &mut Renderer,
        model: &Model,
        view: &gltf::BufferView,
        view_type: BufferViewType,
    ) -> Self {
        let buffer = &model.buffers[view.buffer];
        let data = &buffer.data[view.byte_offset..(view.byte_offset + view.byte_length)];

        match view_type {
            BufferViewType::ArrayBuffer => GPUBuffer::Vertex(renderer.load_vertex_buffer(data)),
            BufferViewType::ElementArrayBuffer => {
                GPUBuffer::Index(renderer.load_index_buffer(data))
            }
            _ => GPUBuffer::None,
        }
    }
}

/// The state required to store the opengl state created from a model
pub struct GLModel {
    views: Vec<GLBuffer>,
    meshes: Vec<GLMesh>,
    textures: Vec<GlTexture>,
}

impl GLModel {
    pub fn new(model: &Model, gl: &gl::Gl) -> Result<Self> {
        let views: Vec<_> = model
            .gltf
            .buffer_views
            .iter()
            .zip(model.buffer_view_types.iter())
            .map(|(view, &view_type)| GLBuffer::new(view, view_type, model, gl))
            .collect();

        let meshes = model
            .gltf
            .meshes
            .iter()
            .map(|mesh| GLMesh::new(gl, &views, mesh, model))
            .collect::<Result<_, _>>()?;

        let textures = model
            .textures
            .iter()
            .map(|tex| GlTexture::new(gl, tex, 0))
            .collect();

        Ok(Self {
            views,
            meshes,
            textures,
        })
    }

    pub fn render(
        &self,
        model: &Model,
        state: &mut EngineStateRef,
        proj: &glm::Mat4,
        view: &glm::Mat4,
    ) {
        let scene_idx = model.gltf.scene.unwrap_or(0);
        let scene = &model.scenes[scene_idx];

        for node_idx in &scene.root_nodes {
            let node = &scene.nodes[*node_idx];
            self.render_node(node, scene, model, state, proj, view);
        }
    }

    fn render_node(
        &self,
        node: &Node,
        scene: &Scene,
        model: &Model,
        state: &mut EngineStateRef,
        proj: &glm::Mat4,
        view: &glm::Mat4,
    ) {
        if let Some(id) = node.mesh_id {
            self.meshes[id].render(model, self, state, &node.global_matrix, proj, view);
        }

        for child in &node.children {
            let node = &scene.nodes[*child];
            self.render_node(node, scene, model, state, proj, view)
        }
    }

    pub(crate) fn load_accessor(
        gl: &gl::Gl,
        buf: &GLBuffer,
        accessor: &gltf::Accessor,
        index: u32,
    ) -> Result<(), Error> {
        if let Some(ref buf) = buf.buf {
            if buf.buffer_type != gltf::BufferViewTarget::ArrayBuffer as u32 {
                return Ok(());
            }
        }

        buf.buf().bind();

        unsafe {
            gl.VertexAttribPointer(
                index,
                accessor.r#type.component_count() as _,
                accessor.component_type.gl_type(),
                gl::FALSE,
                buf.stride,
                accessor.byte_offset as _,
            );
            gl.EnableVertexAttribArray(index);
        }

        buf.buf().unbind();

        Ok(())
    }
}

pub struct GLBuffer {
    buf: Option<buffer::Buffer>,
    stride: i32,
}

impl GLBuffer {
    fn new(view: &gltf::BufferView, view_type: BufferViewType, model: &Model, gl: &gl::Gl) -> Self {
        // if a GPU buffer is required, use the type of buffer infered for
        // the buffer view as that takes into account the view's target
        // if it exists
        let target = match view_type {
            BufferViewType::ArrayBuffer => gl::ARRAY_BUFFER,
            BufferViewType::ElementArrayBuffer => gl::ELEMENT_ARRAY_BUFFER,
            BufferViewType::CPUBuffer | BufferViewType::None => {
                return Self {
                    buf: None,
                    stride: view.byte_stride.unwrap_or_default(),
                };
            }
        };

        let buffer = &model.buffers[view.buffer];
        let data = &buffer.data[view.byte_offset..(view.byte_offset + view.byte_length)];

        let buf = buffer::Buffer::new(gl, target as _);
        buf.bind();
        buf.static_draw_data(data);
        buf.unbind();

        GLBuffer {
            buf: Some(buf),
            stride: view.byte_stride.unwrap_or_default(),
        }
    }

    fn buf(&self) -> &buffer::Buffer {
        self.buf.as_ref().unwrap()
    }
}

#[derive(Debug)]
pub struct GLMesh {
    prims: Vec<GlPrim>,
}

impl GLMesh {
    fn new(
        gl: &gl::Gl,
        buffers: &[GLBuffer],
        mesh: &gltf::Mesh,
        model: &Model,
    ) -> Result<Self, Error> {
        let prims = mesh
            .primitives
            .iter()
            .map(|prim| GlPrim::new(gl, prim, model, buffers))
            .collect::<Result<_, _>>()?;

        Ok(GLMesh { prims })
    }

    fn render(
        &self,
        model: &Model,
        gl_state: &GLModel,
        state: &mut EngineStateRef,
        model_mat: &glm::Mat4,
        proj: &glm::Mat4,
        view: &glm::Mat4,
    ) {
        for prim in &self.prims {
            prim.render(model, gl_state, state, model_mat, proj, view);
        }
    }
}

#[derive(Debug)]
pub struct GlPrim {
    vao: buffer::VertexArray,
    ebo: Option<usize>,
    mode: GLenum,
    count: usize,
    shader: Program,
    base_color: Option<usize>,
    culling: bool,
}

impl GlPrim {
    fn new(
        gl: &gl::Gl,
        prim: &gltf::Primitive,
        model: &Model,
        buffers: &[GLBuffer],
    ) -> Result<Self, Error> {
        let vao = buffer::VertexArray::new(gl);

        vao.bind();

        let count = DynamicShader::set_attribs(gl, buffers, prim, model)?;
        let shader = DynamicShader::new(gl, prim, model)?;

        vao.unbind();

        let mat = if let Some(mat) = prim.material {
            Some(&model.gltf.materials[mat])
        } else {
            None
        };

        let base_color = if let Some(mat) = mat {
            if let Some(pbr) = &mat.pbr_metallic_roughness {
                if let Some(color) = &pbr.base_color_texture {
                    Some(color.index)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok(GlPrim {
            vao,
            count,
            base_color,
            ebo: prim.indices,
            mode: prim.mode.to_gl_enum(),
            shader,
            culling: !mat.map(|a| a.double_sided).unwrap_or(false),
        })
    }

    fn render(
        &self,
        model: &Model,
        gl_state: &GLModel,
        state: &mut EngineStateRef,
        model_mat: &glm::Mat4,
        proj: &glm::Mat4,
        view: &glm::Mat4,
    ) {
        let shader = self.shader.set_used();
        shader.bind_matrix("view", *view);
        shader.bind_matrix("projection", *proj);
        shader.bind_matrix("model", *model_mat);

        let _tex = if let Some(idx) = self.base_color {
            let tex = gl_state.textures[idx].bind(idx as _);
            shader.bind_texture("baseColor", &tex);
            Some(tex)
        } else {
            None
        };

        if self.culling {
            state.backface_culling(true);
        }

        self.vao.bind();

        if let Some(ebo_idx) = self.ebo {
            let access = &model.gltf.accessors[ebo_idx];
            let view_idx = access.buffer_view.unwrap();
            let view = &model.gltf.buffer_views[view_idx];
            let buffer_idx = view.buffer;
            let buffer = &gl_state.views[buffer_idx].buf();
            buffer.bind();

            let r#type = access.component_type.gl_type();

            unsafe {
                state.gl.DrawElements(
                    self.mode,
                    access.count as GLsizei,
                    r#type,
                    access.byte_offset as _,
                );
            }

            buffer.unbind();
        } else {
            unsafe {
                state.gl.DrawArrays(self.mode, 0, self.count as i32);
            }
        }

        if self.culling {
            state.backface_culling(false);
        }

        self.vao.unbind();
    }
}
