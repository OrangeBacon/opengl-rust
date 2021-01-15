mod shader;
pub use shader::{Error, Program, Shader};

pub mod buffer;
pub mod data;
pub mod gltf;
pub mod imgui;
pub mod resources;

mod model;
pub use model::Model;

mod texture;
pub use texture::Texture;

mod layer;
pub use layer::{EventResult, Layer};

mod main_loop;
pub use main_loop::{EngineState, MainLoop};

mod camera;
pub use camera::Camera;

pub use gl;
pub use nalgebra_glm as glm;
pub use sdl2;
