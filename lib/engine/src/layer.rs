use crate::main_loop::EngineState;
use anyhow::Result;

pub trait Layer {
    fn new(state: &EngineState) -> Result<Self>
    where
        Self: std::marker::Sized;
    fn handle_event(&mut self, event: &sdl2::event::Event, state: &EngineState) -> bool;
    fn render(&mut self, state: &EngineState);
}
