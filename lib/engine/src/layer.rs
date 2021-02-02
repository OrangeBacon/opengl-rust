use crate::{
    main_loop::{EngineState, EngineUpdateState},
    window::event::Event,
};
use anyhow::Result;

/// The result of a layer processing an event
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum EventResult {
    /// The game loop should quit, e.g. if the close button is clicked
    Exit,

    /// This even has been handled and should no longer be processed further
    Handled,

    /// This event was ignored and should be passed to the next layer
    Ignored,
}

pub trait Renderer<T: Send + Default> {
    /// Create a new instance of the layer, depending on the current game engine
    /// state.
    fn new(state: &EngineState) -> Result<Self>
    where
        Self: Sized;

    /// Run the rendering for this layer
    fn render(&mut self, graphics: &EngineState, state: &T);

    fn handle_event(&mut self, graphics: &EngineState, event: &Event) -> EventResult {
        let _ = graphics;
        let _ = event;
        EventResult::Ignored
    }
}

pub trait Updater<T: Send + Default> {
    /// Create a new instance of the layer, depending on the current game engine
    /// state.
    fn new(state: &EngineState) -> Result<Self>
    where
        Self: Sized;

    /// Process a single input event.
    fn handle_event(&mut self, state: &mut EngineUpdateState, event: &Event) -> EventResult {
        let _ = state;
        let _ = event;
        EventResult::Ignored
    }

    /// Physics update function, called with a fixed dt, shouldn't change between
    /// update calls.  Can be called multiple times per render.
    /// dt: delta time, the period of time for this update in seconds
    fn update(&mut self, state: &mut EngineUpdateState, data: &T, dt: f32);
}
