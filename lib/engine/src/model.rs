use std::path::Path;

use crate::{
    buffer, gltf,
    resources::{Error as ResourceError, Resources},
};
use anyhow::Result;
use gltf::BufferView;
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
}

#[derive(Debug)]
pub struct Model<'a> {
    scenes: Vec<Scene>,

    buffers: Vec<Buffer>,

    gl_buffers: Vec<buffer::Buffer>,

    model: &'a gltf::Model,
}

impl<'a> Model<'a> {
    pub fn new(gltf: &'a gltf::Model, res: &Resources, folder: &str) -> Result<Self, Error> {
        let res = res.extend(Path::new(folder));

        Ok(Model {
            buffers: gltf
                .buffers
                .iter()
                .map(|buffer| Buffer::new(buffer, &res))
                .collect::<Result<_, _>>()?,

            scenes: gltf
                .scenes
                .iter()
                .map(|scene| Scene::new(scene, gltf))
                .collect::<Result<_, _>>()?,
            model: gltf,
            gl_buffers: Vec::with_capacity(gltf.buffers.len()),
        })
    }

    pub fn load_vram(&mut self, gl: &gl::Gl) -> Result<(), Error> {
        for view in &self.model.buffer_views {
            self.gl_buffers.push(self.load_view(gl, view)?);
        }

        Ok(())
    }

    fn load_view(&self, gl: &gl::Gl, view: &BufferView) -> Result<buffer::Buffer, Error> {
        let target = if let Some(t) = view.target {
            t
        } else {
            return Err(Error::NoTarget);
        };

        let buffer = self
            .buffers
            .get(view.buffer)
            .ok_or_else(|| Error::BadIndex {
                array: "buffers",
                got: view.buffer,
                max: self.buffers.len(),
            })?;
        let data = buffer
            .data
            .get(view.byte_offset..(view.byte_offset + view.byte_length))
            .ok_or_else(|| Error::BadViewLen {
                get: view.byte_offset + view.byte_length,
                max: buffer.data.len(),
            })?;

        let buf = buffer::Buffer::new(gl, target as u32);
        buf.bind();
        buf.static_draw_data(data);
        buf.unbind();

        Ok(buf)
    }
}

#[derive(Debug)]
pub struct Buffer {
    data: Vec<u8>,
}

impl Buffer {
    fn new(buffer: &gltf::Buffer, res: &Resources) -> Result<Self, Error> {
        let mut bytes = res.load_bytes(&buffer.uri).map_err(|e| Error::BufferLoad {
            name: buffer.uri.clone(),
            inner: e,
        })?;

        if bytes.len() < buffer.byte_length {
            return Err(Error::BufferLength {
                len: bytes.len(),
                expected: buffer.byte_length,
            });
        }

        bytes.resize(buffer.byte_length, 0);

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
}

impl Node {
    fn new(
        node: &gltf::Node,
        parent: Option<DefaultKey>,
        gltf: &gltf::Model,
        nodes: &mut SlotMap<DefaultKey, Node>,
    ) -> Result<DefaultKey, Error> {
        let this_key = nodes.insert(Node::default());
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
}
