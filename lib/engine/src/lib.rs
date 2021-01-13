#![feature(min_const_generics)]

mod shader;
pub use shader::{Error, Program, Shader};

pub mod buffer;
pub mod data;
pub mod imgui;
pub mod resources;

mod texture;
pub use texture::Texture;

mod layer;
pub use layer::Layer;

mod main_loop;
pub use main_loop::{EngineState, MainLoop};

pub use gl;
pub use sdl2;
pub use nalgebra_glm as glm;
