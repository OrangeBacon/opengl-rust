use std::{collections::HashMap, convert::TryInto};

use crate::{
    bound::Bounds,
    gltf,
    renderer::{
        self,
        shader::{
            BuiltinVariable, Expression, FunctionContext, Program, ShaderCreationError, Type,
        },
        DrawingMode, IndexBufferId, IndexType, PipelineId, Renderer, TextureId, VertexBufferId,
    },
    resources::{Error as ResourceError, Resources},
    texture,
    texture::Texture,
    EngineStateRef,
};
use anyhow::Result;
use nalgebra_glm as glm;
use renderer::CullingMode;
use slotmap::{DefaultKey, SlotMap};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
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

    #[error("Mesh does not contain position data")]
    NoPositions,

    #[error("Bad vertex attribute lengths")]
    AttribLen,

    #[error("Error loading image {name}: {inner}")]
    ImageLoad {
        name: String,
        #[source]
        inner: ResourceError,
    },

    #[error("Error loading texture into vram: {inner}")]
    Texture {
        #[source]
        inner: crate::texture::TextureError,
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

    #[error("Graphics error in model:\n {inner}")]
    Graphics {
        #[source]
        inner: anyhow::Error,
    },

    #[error("Trying to bind index buffer as vertex array")]
    IndexAsVertex,

    #[error("Trying to bind to an accessor without a defined buffer view")]
    AccessorWithoutBuffer,

    #[error("Shader compilation error while loading model:\n{source}")]
    ShaderCompilation { source: ShaderCreationError },

    #[error("Unable to convert shader to native type:\n{source}")]
    NativeShader { source: anyhow::Error },
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

    pub(crate) gpu_buffers: Vec<GPUBuffer>,
    gpu_textures: Vec<renderer::TextureId>,
    gpu_pipelines: Vec<Vec<GPUPrimitive>>,
}

impl Model {
    pub fn from_res(
        res: &Resources,
        path: &str,
        renderer: &mut Renderer,
    ) -> Result<Self, ModelError> {
        let parent = res.extend_file_root(path).ok_or(ModelError::RootPath {
            inner: path.to_string(),
        })?;

        let gltf = gltf::Model::from_res(res, path).map_err(|e| ModelError::Gltf { inner: e })?;

        let model = Model::from_gltf(gltf, &parent, renderer)?;

        Ok(model)
    }

    pub fn from_gltf(
        mut gltf: gltf::Model,
        res: &Resources,
        renderer: &mut Renderer,
    ) -> Result<Self, ModelError> {
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
    ) -> Result<Vec<u8>, ModelError> {
        let data = match img.uri {
            Some(ref uri) => res.load_bytes(uri).map_err(|e| ModelError::ImageLoad {
                name: uri.to_string(),
                inner: e,
            })?,
            None => {
                if let Some(buffer_view) = img.buffer_view {
                    let view = gltf
                        .buffer_views
                        .get(buffer_view)
                        .ok_or(ModelError::BadIndex {
                            array: "buffer views",
                            got: buffer_view,
                            max: gltf.buffer_views.len(),
                        })?;

                    let buffer = buffers.get(view.buffer).ok_or(ModelError::BadIndex {
                        array: "buffers",
                        got: view.buffer,
                        max: gltf.buffers.len(),
                    })?;

                    let data = buffer
                        .data
                        .get(view.byte_offset..(view.byte_offset + view.byte_length))
                        .ok_or(ModelError::BadIndex {
                            array: "buffers",
                            got: view.buffer,
                            max: gltf.buffers.len(),
                        })?;

                    data.to_vec()
                } else {
                    return Err(ModelError::NoSource);
                }
            }
        };

        Ok(data)
    }

    fn load_texture(
        tex: &gltf::Texture,
        gltf: &gltf::Model,
        images: &[Vec<u8>],
    ) -> Result<Texture, ModelError> {
        let default = gltf::Sampler::default();
        let sampler = if let Some(idx) = tex.sampler {
            &gltf.samplers[idx]
        } else {
            &default
        };

        let sampler = texture::TextureOptions {
            wrap_s: sampler.wrap_s.into(),
            wrap_t: sampler.wrap_t.into(),
            min_filter: sampler.min_filter.into(),
            mag_filter: sampler.mag_filter.into(),
            ..Default::default()
        };

        let source = tex.source.ok_or(ModelError::NoImage)?;
        let data = &images[source];

        let tex = Texture::from_encoding_config(data, sampler)
            .map_err(|e| ModelError::Texture { inner: e })?;

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
    fn check_load_accessors(&mut self) -> Result<(), ModelError> {
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
                .ok_or(ModelError::BadIndex {
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
                .ok_or(ModelError::BadIndex {
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
                .ok_or(ModelError::BadIndex {
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
            }

            // dispatch the correct accessor process
            match sparse.indices.component_type {
                gltf::ComponentType::Byte => process!(i8),
                gltf::ComponentType::UnsignedByte => process!(u8),
                gltf::ComponentType::Short => process!(i16),
                gltf::ComponentType::UnsignedShort => process!(u16),
                gltf::ComponentType::UnsignedInt => process!(u32),
                gltf::ComponentType::Float => return Err(ModelError::SparseFloat),
            }
        }

        Ok(())
    }

    // try to get a reference to the data from a buffer view
    fn buffer_data(&self, buffer_view: usize) -> Result<&[u8], ModelError> {
        let view = self
            .gltf
            .buffer_views
            .get(buffer_view)
            .ok_or_else(|| ModelError::BadIndex {
                array: "Buffer views",
                got: buffer_view,
                max: self.gltf.buffer_views.len(),
            })?;

        let buffer = self
            .buffers
            .get(view.buffer)
            .ok_or_else(|| ModelError::BadIndex {
                array: "Buffers",
                got: view.buffer,
                max: self.buffers.len(),
            })?;

        Ok(buffer
            .data
            .get(view.byte_offset..(view.byte_offset + view.byte_length))
            .ok_or_else(|| ModelError::BadIndex {
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
    ) -> Result<&'a mut [u8], ModelError> {
        let view = buffer_views
            .get(buffer_view)
            .ok_or_else(|| ModelError::BadIndex {
                array: "Buffer views",
                got: buffer_view,
                max: buffer_views.len(),
            })?;

        let buffer_len = buffers.len();

        let buffer = buffers.get_mut(view.buffer).ok_or(ModelError::BadIndex {
            array: "Buffers",
            got: view.buffer,
            max: buffer_len,
        })?;

        let buffer_len = buffer.data.len();

        Ok(buffer
            .data
            .get_mut(view.byte_offset..(view.byte_offset + view.byte_length))
            .ok_or(ModelError::BadIndex {
                array: "Buffer get",
                got: view.byte_offset + view.byte_length,
                max: buffer_len,
            })?)
    }

    /// load a 3d model onto the GPU
    fn load_gpu(&mut self, renderer: &mut Renderer, res: &Resources) -> Result<(), ModelError> {
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

        self.gpu_pipelines = Vec::with_capacity(self.gltf.meshes.len());

        for mesh in &self.gltf.meshes {
            let mut pipelines = Vec::with_capacity(mesh.primitives.len());

            for prim in &mesh.primitives {
                pipelines.push(GPUPrimitive::new(prim, self, renderer)?);
            }

            self.gpu_pipelines.push(pipelines);
        }

        Ok(())
    }

    pub fn render(
        &self,
        state: &mut EngineStateRef,
        proj: &glm::Mat4,
        view: &glm::Mat4,
    ) -> Result<()> {
        let scene_idx = self.gltf.scene.unwrap_or(0);
        let scene = &self.scenes[scene_idx];

        for node_idx in &scene.root_nodes {
            let node = &scene.nodes[*node_idx];
            self.render_node(node, scene, state, proj, view)?;
        }

        Ok(())
    }

    fn render_node(
        &self,
        node: &Node,
        scene: &Scene,
        state: &mut EngineStateRef,
        proj: &glm::Mat4,
        view: &glm::Mat4,
    ) -> Result<()> {
        if let Some(mesh_id) = node.mesh_id {
            self.render_mesh(mesh_id, state, &node.global_matrix, proj, view)?;
        }

        for child in &node.children {
            let node = &scene.nodes[*child];
            self.render_node(node, scene, state, proj, view)?;
        }

        Ok(())
    }

    fn render_mesh(
        &self,
        mesh_id: usize,
        state: &mut EngineStateRef,
        model_mat: &glm::Mat4,
        proj: &glm::Mat4,
        view: &glm::Mat4,
    ) -> Result<()> {
        for prim in &self.gpu_pipelines[mesh_id] {
            prim.render(state, view, proj, model_mat)?;
        }

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
) -> Result<(), ModelError>
where
    F: Fn(&[u8]) -> usize,
{
    if indicies.len() % idx_size != 0 {
        return Err(ModelError::SparseIndicies);
    }

    if indicies.len() / idx_size != values.len() / value_size {
        return Err(ModelError::SparseIndexValues);
    }

    let data_len = data.len();

    let iter = indicies.chunks(idx_size).zip(values.chunks(value_size));

    // loop through all possible sparse substitutions
    for (idx, value) in iter {
        let idx = to_usize(idx);
        let byte_offset = stride * idx;

        let data = data
            .get_mut(byte_offset..(byte_offset + value_size))
            .ok_or(ModelError::BadIndex {
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
    fn derive_types(model: &gltf::Model) -> Result<Vec<Self>, ModelError> {
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
                let view = types.get_mut(view).ok_or_else(|| ModelError::BadIndex {
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

                let view = types.get_mut(idx).ok_or_else(|| ModelError::BadIndex {
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
            return Err(ModelError::BufferViewNoInference);
        }

        Ok(types)
    }

    /// Derive the types of buffer for a mesh primative
    fn derive_prim(
        prim: &gltf::Primitive,
        model: &gltf::Model,
        types: &mut [Self],
    ) -> Result<(), ModelError> {
        // set the vertex attributes to be array buffers
        for (_, &attr_id) in &prim.attributes {
            let accessor =
                model
                    .accessors
                    .get(attr_id as usize)
                    .ok_or_else(|| ModelError::BadIndex {
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
                    .ok_or_else(|| ModelError::BadIndex {
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
    fn set_accessor(
        accessor: &gltf::Accessor,
        types: &mut [Self],
        set: Self,
    ) -> Result<(), ModelError> {
        // calculate length, required for errors due to get_mut + lifetimes
        let type_len = types.len();

        // Set the buffer view to the requested type, if the accessor has
        // a buffer view
        if let Some(view) = accessor.buffer_view {
            let view = types.get_mut(view).ok_or(ModelError::BadIndex {
                array: "buffer views",
                got: view,
                max: type_len,
            })?;

            Self::set_view(view, set)?;
        }

        // If there is a sparse accessor, set its views to be CPU buffers
        if let Some(sparse) = &accessor.sparse {
            let indicies = sparse.indices.buffer_view;
            let view = types.get_mut(indicies).ok_or(ModelError::BadIndex {
                array: "buffer views",
                got: indicies,
                max: type_len,
            })?;

            Self::set_view(view, Self::CPUBuffer)?;

            let values = sparse.values.buffer_view;
            let view = types.get_mut(values).ok_or(ModelError::BadIndex {
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
    fn set_view(view: &mut Self, set: Self) -> Result<(), ModelError> {
        match view {
            Self::None => *view = set,
            _ if *view == set => (),
            _ => return Err(ModelError::BufferViewType),
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
    ) -> Result<Self, ModelError> {
        let bytes = match buffer.uri {
            Some(ref uri) => res.load_bytes(&uri).map_err(|e| ModelError::BufferLoad {
                name: uri.clone(),
                inner: e,
            })?,
            None => match default.take() {
                Some(data) => data,
                None => return Err(ModelError::InternalBuffer),
            },
        };

        if bytes.len() < buffer.byte_length {
            return Err(ModelError::BufferLength {
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
    fn new(scene: &gltf::Scene, gltf: &gltf::Model) -> Result<Self, ModelError> {
        let mut nodes = SlotMap::new();

        let root_nodes = scene
            .nodes
            .iter()
            .map(|&node_id| {
                Ok(Node::new(
                    gltf.nodes
                        .get(node_id)
                        .ok_or_else(|| ModelError::BadIndex {
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
    ) -> Result<DefaultKey, ModelError> {
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
                    gltf.nodes
                        .get(node_id)
                        .ok_or_else(|| ModelError::BadIndex {
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

    fn get_matrix(node: &gltf::Node) -> Result<glm::Mat4, ModelError> {
        let mut matrix = glm::Mat4::identity();

        if let Some(m) = node.matrix {
            matrix.copy_from_slice(&m);

            if node.translation.is_some() || node.rotation.is_some() || node.scale.is_some() {
                return Err(ModelError::DuplicateTransform);
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
#[derive(Debug, Copy, Clone)]
pub(crate) enum GPUBuffer {
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

#[derive(Debug)]
struct GPUPrimitiveIndexInfo {
    buffer: IndexBufferId,
    item_type: IndexType,
    count: usize,
    offset: usize,
}

#[derive(Debug)]
struct GPUPrimitive {
    pipeline: PipelineId,
    vertex_count: usize,
    base_color_texidx: Option<TextureId>,
    indicies: Option<GPUPrimitiveIndexInfo>,
    draw_mode: DrawingMode,
    culling: bool,

    vertex_buffers: Vec<VertexBufferId>,
    vertex_strides: Vec<i32>,
    vertex_offsets: Vec<usize>,
}

impl GPUPrimitive {
    fn new(
        prim: &gltf::Primitive,
        model: &Model,
        renderer: &mut Renderer,
    ) -> Result<Self, ModelError> {
        let pipeline = Self::create_shader(prim, model)?;

        let mat = if let Some(mat) = prim.material {
            Some(&model.gltf.materials[mat])
        } else {
            None
        };

        let base_color = if let Some(mat) = mat {
            if let Some(pbr) = &mat.pbr_metallic_roughness {
                if let Some(color) = &pbr.base_color_texture {
                    Some(model.gpu_textures[color.index])
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let pipeline = renderer
            .load_pipeline(pipeline)
            .map_err(|e| ModelError::Graphics { inner: e.into() })?;

        let draw_mode = match prim.mode {
            gltf::PrimitiveMode::LineLoop => DrawingMode::LineLoop,
            gltf::PrimitiveMode::LineStrip => DrawingMode::LineStrip,
            gltf::PrimitiveMode::Lines => DrawingMode::Lines,
            gltf::PrimitiveMode::Points => DrawingMode::Points,
            gltf::PrimitiveMode::TriangleFan => DrawingMode::TriangleFan,
            gltf::PrimitiveMode::TriangleStrip => DrawingMode::TriangleStrip,
            gltf::PrimitiveMode::Triangles => DrawingMode::Triangles,
        };

        let indicies = prim
            .indices
            .and_then(|idx| {
                let accessor = &model.gltf.accessors[idx];
                if let Some(view) = accessor.buffer_view {
                    Some((accessor, view))
                } else {
                    None
                }
            })
            .and_then(|(accessor, view)| {
                if let GPUBuffer::Index(buf) = model.gpu_buffers[view] {
                    Some((accessor, buf))
                } else {
                    None
                }
            })
            .and_then(|(accessor, buf)| {
                if let Some(ty) = accessor.component_type.index_type() {
                    Some(GPUPrimitiveIndexInfo {
                        buffer: buf,
                        count: accessor.count,
                        item_type: ty,
                        offset: accessor.byte_offset,
                    })
                } else {
                    None
                }
            });

        let (vertex_buffers, vertex_offsets, vertex_strides, vertex_count) =
            Self::get_vertex_array_data(prim, model)?;

        Ok(Self {
            pipeline,
            vertex_count,
            draw_mode,
            indicies,
            vertex_buffers,
            vertex_offsets,
            vertex_strides,
            base_color_texidx: base_color,
            culling: !mat.map(|a| a.double_sided).unwrap_or(false),
        })
    }

    /// get the vertex buffers, strides and offsets (in that order) for a primitive
    fn get_vertex_array_data(
        prim: &gltf::Primitive,
        model: &Model,
    ) -> Result<(Vec<VertexBufferId>, Vec<usize>, Vec<i32>, usize), ModelError> {
        let capacity = prim.attributes.len();
        let mut buffers = Vec::with_capacity(capacity);
        let mut offsets = Vec::with_capacity(capacity);
        let mut strides = Vec::with_capacity(capacity);
        let mut vertex_count = 0;

        for (_, &attr) in &prim.attributes {
            let accessor = &model.gltf.accessors[attr as usize];
            if let Some(view_idx) = accessor.buffer_view {
                if let GPUBuffer::Vertex(buf) = model.gpu_buffers[view_idx] {
                    let view = &model.gltf.buffer_views[view_idx];

                    if vertex_count != 0 && vertex_count != accessor.count {
                        return Err(ModelError::AttribLen);
                    }
                    vertex_count = accessor.count;

                    buffers.push(buf);
                    offsets.push(accessor.byte_offset);
                    strides.push(view.byte_stride.unwrap_or(0) as _);
                } else {
                    return Err(ModelError::IndexAsVertex);
                }
            } else {
                return Err(ModelError::AccessorWithoutBuffer);
            }
        }

        Ok((buffers, offsets, strides, vertex_count))
    }

    fn render(
        &self,
        renderer: &mut Renderer,
        view: &glm::Mat4,
        proj: &glm::Mat4,
        model: &glm::Mat4,
    ) -> Result<()> {
        if self.culling {
            renderer.backface_culling(CullingMode::Back);
        } else {
            renderer.backface_culling(CullingMode::None)
        }
        renderer.depth_testing(renderer::DepthTesting::Default);

        let mut pipeline = renderer.bind_pipeline(self.pipeline);
        pipeline.bind_matrix("view", *view)?;
        pipeline.bind_matrix("projection", *proj)?;
        pipeline.bind_matrix("model", *model)?;

        if let Some(tex) = self.base_color_texidx {
            pipeline.bind_texture("base_color", tex)?;
        }

        pipeline.bind_vertex_arrays(
            &self.vertex_buffers,
            &self.vertex_offsets,
            &self.vertex_strides,
        );

        if let Some(indices) = &self.indicies {
            pipeline.draw_indicies(
                self.draw_mode,
                indices.buffer,
                indices.item_type,
                indices.offset,
                indices.count,
            );
        } else {
            pipeline.draw(self.draw_mode, 0, self.vertex_count as _);
        }

        Ok(())
    }

    fn create_shader(prim: &gltf::Primitive, model: &Model) -> Result<Program, ModelError> {
        let mut components: Vec<Attribute> = prim
            .attributes
            .iter()
            .map(|(name, &accessor)| Some(Attribute::from(&name, accessor as _)?))
            .flatten()
            .collect();

        let material_components = prim
            .material
            .map(|mat| Attribute::material(&model.gltf.materials[mat]));
        if let Some(material_components) = material_components {
            components.extend(material_components);
        }

        let mut shader = Program::new(|ctx| {
            ctx.vertex(|ctx| {
                for comp in &components {
                    comp.vertex(ctx, model);
                }
            });

            ctx.frag(|ctx| {
                let colors = components
                    .iter()
                    .map(|comp| comp.frag(ctx, prim, model))
                    .flatten();
                let output = colors.reduce(std::ops::Mul::mul);

                let output_global = ctx.output("frag_color", Type::Vec4);

                if let Some(output) = output {
                    ctx.set_output(output_global, output);
                } else {
                    ctx.set_output(
                        output_global,
                        Expression::vec(&[0.5.into(), 0.5.into(), 0.5.into(), 1.0.into()]),
                    )
                }
            })
        });

        shader
            .ok()
            .map_err(|e| ModelError::ShaderCompilation { source: e })?;

        Ok(shader)
    }
}

enum Attribute {
    Position,
    Normal { accessor: usize },
    Tangent { accessor: usize },
    TexCoord { idx: usize },
    VertexColor { accessor: usize, idx: usize },
    Joints { accessor: usize, idx: usize },
    Weights { accessor: usize, idx: usize },
    BaseColor { color: [f32; 4] },
}

impl Attribute {
    fn from(value: &str, accessor: usize) -> Option<Self> {
        let comps: Vec<_> = value.split('_').collect();

        let ty = match comps.as_slice() {
            ["POSITION"] => Attribute::Position,
            ["NORMAL"] => Attribute::Normal { accessor },
            ["TANGENT"] => Attribute::Tangent { accessor },
            ["TEXCOORD", a] => Attribute::TexCoord {
                idx: a.parse().ok()?,
            },
            ["COLOR", a] => Attribute::VertexColor {
                accessor,
                idx: a.parse().ok()?,
            },
            ["JOINTS", a] => Attribute::Joints {
                accessor,
                idx: a.parse().ok()?,
            },
            ["WEIGHTS", a] => Attribute::Weights {
                accessor,
                idx: a.parse().ok()?,
            },
            _ => return None,
        };

        Some(ty)
    }

    fn material(mat: &gltf::Material) -> Vec<Self> {
        let mut ret = vec![];

        if let Some(pbr) = &mat.pbr_metallic_roughness {
            if pbr.base_color_factor != [1.0; 4] {
                ret.push(Attribute::BaseColor {
                    color: pbr.base_color_factor,
                })
            }
        }

        ret
    }
}

impl Attribute {
    fn vertex(&self, ctx: &mut FunctionContext, model: &Model) {
        match self {
            Attribute::Position => {
                let view = ctx.uniform("view", Type::Mat4);
                let model = ctx.uniform("model", Type::Mat4);
                let projection = ctx.uniform("projection", Type::Mat4);

                let position = ctx.input("Position_in", Type::Vec3);
                let value = projection * view * model * Expression::vec(&[position, 1.0.into()]);

                ctx.set_builtin(BuiltinVariable::VertexPosition, value)
            }
            Attribute::VertexColor { accessor, idx } => {
                let ty = model.gltf.accessors[*accessor].r#type.to_shader_type();

                let color = ctx.input(&format!("Color{}_in", idx), ty.clone());
                let output = ctx.output(&format!("Color{}", idx), ty);
                ctx.set_output(output, color);
            }
            Attribute::TexCoord { idx, .. } => {
                let coord = ctx.input(&format!("TexCoord{}_in", idx), Type::Vec2);
                let output = ctx.output(&format!("TexCoord{}", idx), Type::Vec2);
                ctx.set_output(output, coord);
            }

            Attribute::Normal { accessor } => {
                let ty = model.gltf.accessors[*accessor].r#type.to_shader_type();
                ctx.input("Normal_in", ty);
            }
            Attribute::Tangent { accessor } => {
                let ty = model.gltf.accessors[*accessor].r#type.to_shader_type();
                ctx.input("Tangent_in", ty);
            }
            Attribute::Joints { accessor, idx } => {
                let ty = model.gltf.accessors[*accessor].r#type.to_shader_type();
                ctx.input(&format!("Joints{}_in", idx), ty);
            }
            Attribute::Weights { accessor, idx } => {
                let ty = model.gltf.accessors[*accessor].r#type.to_shader_type();
                ctx.input(&format!("Weights{}_in", idx), ty);
            }
            Attribute::BaseColor { .. } => {}
        }
    }

    fn frag(
        &self,
        ctx: &mut FunctionContext,
        prim: &gltf::Primitive,
        model: &Model,
    ) -> Option<Expression> {
        match self {
            Attribute::VertexColor { accessor, idx } => {
                let ty = model.gltf.accessors[*accessor].r#type.to_shader_type();

                let color = ctx.input(&format!("Color{}", idx), ty.clone());

                Some(color)
            }
            Attribute::TexCoord { idx, .. } if is_base_color(prim, model, *idx) => {
                let base_color = ctx.uniform("base_color", Type::Sampler2D);
                let uv = ctx.input(&format!("TexCoord{}", idx), Type::Vec2);

                Some(Expression::texture(base_color, uv))
            }
            Attribute::BaseColor { color } => {
                let color = [
                    color[0].into(),
                    color[1].into(),
                    color[2].into(),
                    color[3].into(),
                ];

                Some(Expression::vec(&color))
            }
            _ => None,
        }
    }
}

fn is_base_color(prim: &gltf::Primitive, model: &Model, idx: usize) -> bool {
    prim.material
        .and_then(|mat| model.gltf.materials[mat].pbr_metallic_roughness.as_ref())
        .and_then(|pbr| pbr.base_color_texture.as_ref())
        .map(|color| color.tex_coord == idx)
        .unwrap_or(false)
}
