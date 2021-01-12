#![feature(min_const_generics)]

mod shader;
pub use shader::{Error, Program, Shader};

pub mod buffer;
pub mod data;
pub mod resources;

mod layer;
pub use layer::Layer;

mod main_loop;
pub use main_loop::MainLoop;

pub use sdl2;
pub use gl;