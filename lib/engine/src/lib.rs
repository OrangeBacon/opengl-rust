#![feature(min_const_generics)]

mod shader;
pub use self::shader::{Error, Program, Shader};

pub mod buffer;
pub mod data;
pub mod resources;
