use crate::{
    buffer, gltf,
    resources::{Error as ResourceError, Resources},
    texture,
    texture::Texture,
    DynamicShader, Program,
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
}

pub struct ModelShaders {
    pub plain: Program,
    pub color: Program,
}

/// A 3d gltf model, including all its data.  Not dependant upon any rendering
/// backend
#[derive(Debug)]
pub struct Model {
    scenes: Vec<Scene>,

    buffers: Vec<Buffer>,

    textures: Vec<Texture>,

    pub(crate) model: gltf::Model,
}

impl Model {
    pub fn from_res(res: &Resources, path: &str) -> Result<Self, Error> {
        let parent = res.extend_file_root(path).ok_or(Error::RootPath {
            inner: path.to_string(),
        })?;

        let gltf = gltf::Model::from_res(res, path).map_err(|e| Error::Gltf { inner: e })?;

        let model = Model::from_gltf(gltf, &parent)?;

        Ok(model)
    }

    pub fn from_gltf(mut gltf: gltf::Model, res: &Resources) -> Result<Self, Error> {
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

        Ok(Model {
            buffers,
            scenes,
            model: gltf,
            textures,
        })
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
            gltf.samplers[idx]
        } else {
            default
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
    /*
    pub(crate) fn load_accessor(
        &self,
        gl: &gl::Gl,
        accessor: &gltf::Accessor,
        index: u32,
    ) -> Result<(), Error> {
        let max_len = self.model.buffer_views.len();

        let buf = self
            .model
            .buffer_views
            .get(accessor.buffer_view)
            .ok_or_else(|| Error::BadIndex {
                array: "buffer views",
                got: accessor.buffer_view,
                max: max_len,
            })?;

        if let Some(ref buf) = buf.buf {
            if buf.buffer_type != gltf::BufferViewTarget::ArrayBuffer as u32 {
                return Ok(());
            }
        }

        buf.bind();

        unsafe {
            gl.VertexAttribPointer(
                index,
                accessor.r#type.component_count(),
                accessor.component_type.get_gl_type(),
                gl::FALSE,
                buf.stride,
                accessor.byte_offset as _,
            );
            gl.EnableVertexAttribArray(index);
        }

        buf.unbind();

        Ok(())
    }

    pub fn render(&self, gl: &gl::Gl, proj: &glm::Mat4, view: &glm::Mat4) {
        self.scenes[0].render(self, gl, proj, view);
    }*/
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
/*
    fn render(&self, model: &Model, gl: &gl::Gl, proj: &glm::Mat4, view: &glm::Mat4) {
        for node_id in &self.root_nodes {
            self.nodes[*node_id].render(model, self, gl, proj, view);
        }
    }*/
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
    /*
    fn render(
        &self,
        model: &Model,
        scene: &Scene,
        gl: &gl::Gl,
        proj: &glm::Mat4,
        view: &glm::Mat4,
    ) {
        if let Some(id) = self.mesh_id {
            model.meshes[id].render(model, gl, &self.global_matrix, proj, view);
        }

        for child in &self.children {
            scene.nodes[*child].render(model, scene, gl, proj, view);
        }
    }*/
}

#[derive(Debug)]
pub struct GlMesh {
    prims: Vec<GlPrim>,
}

impl GlMesh {
    fn load(gl: &gl::Gl, mesh: &gltf::Mesh, model: &Model) -> Result<Self, Error> {
        let prims = mesh
            .primitives
            .iter()
            .map(|prim| GlPrim::load(gl, prim, model))
            .collect::<Result<_, _>>()?;

        Ok(GlMesh { prims })
    }

    fn render(
        &self,
        model: &Model,
        gl: &gl::Gl,
        model_mat: &glm::Mat4,
        proj: &glm::Mat4,
        view: &glm::Mat4,
    ) {
        for prim in &self.prims {
            prim.render(model, gl, model_mat, proj, view);
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
}

impl GlPrim {
    fn load(gl: &gl::Gl, prim: &gltf::Primitive, model: &Model) -> Result<Self, Error> {
        let vao = buffer::VertexArray::new(gl);

        vao.bind();

        let count = DynamicShader::set_attribs(gl, prim, model)?;
        let shader = DynamicShader::new(gl, prim, model)?;

        vao.unbind();

        let base_color = if let Some(mat) = prim.material {
            let mat = &model.model.materials[mat];
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
        })
    }

    fn render(
        &self,
        model: &Model,
        gl: &gl::Gl,
        model_mat: &glm::Mat4,
        proj: &glm::Mat4,
        view: &glm::Mat4,
    ) {
        let shader = self.shader.set_used();
        shader.bind_matrix("view", *view);
        shader.bind_matrix("projection", *proj);
        shader.bind_matrix("model", *model_mat);

        let _tex = if let Some(idx) = self.base_color {
            let tex = model.textures[idx].bind(idx as _);
            shader.bind_texture("baseColor", &tex);
            Some(tex)
        } else {
            None
        };

        self.vao.bind();

        if let Some(ebo_idx) = self.ebo {
            let access = &model.model.accessors[ebo_idx];
            let view_idx = access.buffer_view;
            let view = &model.model.buffer_views[view_idx];
            let buffer_idx = view.buffer;
            let buffer = &model.buffer_views[buffer_idx];
            buffer.bind();

            let r#type = access.component_type.get_gl_type();

            unsafe {
                gl.DrawElements(
                    self.mode,
                    access.count as GLsizei,
                    r#type,
                    access.byte_offset as _,
                );
            }

            buffer.unbind();
        } else {
            unsafe {
                gl.DrawArrays(self.mode, 0, self.count as i32);
            }
        }

        self.vao.unbind();
    }
}

/// The state required to store the opengl state created from a model
pub struct GLModel {}
