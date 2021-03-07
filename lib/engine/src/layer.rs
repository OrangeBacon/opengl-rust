use crate::{window::event::Event, EngineStateRef};
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

/// The ordering that a layer should be updated or rendered in
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum CallOrder {
    /// This render or update should be defered until after its child states
    Deferred,

    /// This render or update should be performed immediately
    Standard,
}

/// A single render layer
pub trait Layer {
    /// Create a new instance of the layer, depending on the current game engine
    /// state.
    fn new(state: &mut EngineStateRef) -> Result<Self>
    where
        Self: Sized;

    /// Process a single input event.
    fn handle_event(&mut self, state: &mut EngineStateRef, event: &Event) -> EventResult {
        let _ = state;
        let _ = event;
        EventResult::Ignored
    }

    /// Physics update function, called with a fixed dt, shouldn't change between
    /// update calls.  Can be called multiple times per render.
    /// dt: delta time, the period of time for this update in seconds
    fn update(&mut self, state: &mut EngineStateRef, dt: f32);

    /// Run the rendering for this layer
    fn render(&mut self, state: &mut EngineStateRef);

    /// The order that the layer should be updated it, it is assumed that this
    /// is a const fn, but that cannot be expressed in the trait.  Default is
    /// standard update order.
    fn update_order(&self) -> CallOrder {
        CallOrder::Standard
    }

    /// The order that the layer should be rendered it, it is assumed that this
    /// is a const fn, but that cannot be expressed in the trait. Default is
    /// standard render order.
    fn render_order(&self) -> CallOrder {
        CallOrder::Standard
    }
}
