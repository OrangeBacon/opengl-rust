pub mod buffer;
pub mod camera;
pub mod data;
pub mod gltf;
pub mod imgui;
pub mod render;
pub mod resources;
pub mod scene;
pub mod window;

mod shader;
pub use shader::{Error, Program, Shader};

mod create_shader;
pub use create_shader::DynamicShader;

mod model;
pub use model::{Model, ModelShaders};

mod layer;
pub use layer::{EventResult, Renderer, Updater};

mod main_loop;
pub use main_loop::{EngineState, EngineUpdateState, MainLoop};

pub use gl;
pub use nalgebra_glm as glm;
