use crate::main_loop::EngineState;
use anyhow::Result;

/// The result of a layer processing an event
#[derive(PartialEq, Eq)]
pub enum EventResult {
    /// The game loop should quit, e.g. if the close button is clicked
    Exit,

    /// This even has been handled and should no longer be processed further
    Handled,

    /// This event was ignored and should be passed to the next layer
    Ignored,
}

/// A single render layer
pub trait Layer {
    /// Create a new instance of the layer, depending on the current game engine
    /// state.
    fn new(state: &EngineState) -> Result<Self>
    where
        Self: Sized;

    /// Process a single input event.
    fn handle_event(&mut self, state: &EngineState, event: &sdl2::event::Event) -> EventResult;

    /// Physics update function, called with a fixed dt, shouldn't change between
    /// update calls.  Can be called multiple times per render.
    /// time: time in seconds at the start of the current update
    /// dt: delta time, the period of time for this update in seconds
    /// The update processes the time from `time` to `time + dt`
    fn update(&mut self, state: &EngineState, time: f64, dt: f64);

    /// Run the rendering for this layer
    fn render(&mut self, state: &EngineState);
}
