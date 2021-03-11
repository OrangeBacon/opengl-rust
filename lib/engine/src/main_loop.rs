use anyhow::Result;
use std::{
    ops::{Deref, DerefMut},
    time::Instant,
};

use crate::{
    renderer::Renderer,
    window::{
        event::Event,
        input::{InputState, KeyState},
        window::{Window, WindowConfig},
    },
    CallOrder, EventResult, Layer,
};

/// Graphics api state that is available to render layers.
pub struct EngineState {
    /// Primary OpenGL context, used when rendering to the main window.
    pub gl: gl::Gl,

    /// The state of all keyboard and mouse inputs
    pub inputs: InputState,

    /// The total time the program has been running in seconds
    pub run_time: f32,

    /// The operating system window being used
    pub window: Box<dyn Window>,

    /// The current renderer
    renderer: Box<dyn Renderer>,
}

// For now i'm allowing direct access to the renderer.  When all commands use
// the renderer, then it can be moved to using command lists and mpsc channels,
// allowing multi-threading.  That cannot be done now due to raw gl calls being
// mixed with renderer calls.
impl Deref for EngineState {
    type Target = dyn Renderer;

    fn deref(&self) -> &Self::Target {
        self.renderer.as_ref()
    }
}

impl DerefMut for EngineState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.renderer.as_mut()
    }
}

/// A reference to the current engine state, allowing updating the layer stack
/// from within update/render methods
pub struct EngineStateRef<'a> {
    /// The engine state being referenced
    state: &'a mut EngineState,

    /// The id of the layer that is being passed this reference
    layer_id: usize,

    /// Any states added to the engine during the call
    layer_push: &'a mut Vec<Box<dyn Layer>>,

    /// Indicis of any stats that are removed during the call
    layer_pop: &'a mut Vec<usize>,
}

impl<'a> EngineStateRef<'a> {
    /// Push a new layer to the engine at the end of this frame
    pub fn push_state<T: Layer + 'static>(&mut self, layer: T) {
        self.layer_push.push(Box::new(layer));
    }

    /// Remove this layer from the engine at the end of this frame
    pub fn pop_state(&mut self) {
        self.layer_pop.push(self.layer_id);
    }
}

impl<'a> Deref for EngineStateRef<'a> {
    type Target = EngineState;

    /// treat the EngineStateRef as an engine state, avoids api changes
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<'a> DerefMut for EngineStateRef<'a> {
    /// treat the EngineStateRef as an engine state, avoids api changes
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

/// The main game storage, is used to call the main loop.
pub struct MainLoop {
    /// Vector of everything in the render state of the game.
    layers: Vec<Box<dyn crate::Layer>>,

    /// The current graphics state.
    state: EngineState,

    /// The order that updates should be processed in
    update_order: Vec<usize>,

    /// The order that renders should be processed in
    render_order: Vec<usize>,
}

impl MainLoop {
    /// Try to create a new game engine instance, using default settings
    /// Currently there is no way to change the settings used, this should
    /// probably be changed.  This method initialises the graphics and creates
    /// a window that will be shown to the user.
    pub fn new<W: Window + 'static, L: Layer + 'static>() -> Result<Self> {
        let config = WindowConfig {
            debug: true,
            gl_version: (4, 5),
            width: 900,
            height: 700,
            resizable: true,
            title: "Game",
        };

        let mut window = W::new(config)?;

        let gl = window.new_gl_context()?;

        #[cfg(debug_assertions)]
        enable_gl_debugging(&gl);

        let state = EngineState {
            gl,
            renderer: window.renderer()?,
            window: Box::new(window),
            inputs: Default::default(),
            run_time: 0.0,
        };

        // vec capacity 4 is completely arbitary, could increase/decrease later
        // depending on layer stack size
        let mut this = MainLoop {
            layers: Vec::with_capacity(4),
            state,
            update_order: Vec::with_capacity(4),
            render_order: Vec::with_capacity(4),
        };

        // For the first layer, push goes directly onto the layer stack as there
        // isn't anything else to intefere, so no point allocating a new vec.
        // Ignores poping any states in this method as there should be no use
        // so don't bother trying to deal with it.
        let mut state_ref = EngineStateRef {
            layer_push: &mut this.layers,
            state: &mut this.state,
            layer_pop: &mut Vec::with_capacity(0),
            layer_id: 0,
        };

        let first_layer = Box::new(L::new(&mut state_ref)?);

        // Insert the first layer in index 0, as it should be first, regardless
        // of what else was pushed during its initialisation.
        this.layers.insert(0, first_layer);

        // this needs to be called any time self.layers is modified
        this.set_call_order();

        Ok(this)
    }

    /// Start the game loop, this function will not return until the main window
    /// is closed, only saving, error reporting, etc should happen afterwards.
    pub fn run(mut self) -> Result<()> {
        // timing infomation
        const DT: f32 = 1.0 / 60.0;
        let mut current_time = Instant::now();
        let mut accumulator = 0.0;

        // The layers pushed and poped during a single loop, allocated outside
        // the loop to reduce reallocation of these vectors, they are cleared
        // at the end of a loop if required.
        let mut layer_push = vec![];
        let mut layer_pop = vec![];

        'main: loop {
            let new_time = Instant::now();
            let frame_time = (new_time - current_time).as_secs_f32();
            current_time = new_time;

            accumulator += frame_time;

            self.state.window.update_mouse(&mut self.state.inputs);

            // poll for events
            'event: while let Some(event) = self.state.window.event() {
                for &layer in &self.update_order {
                    let mut state = EngineStateRef {
                        state: &mut self.state,
                        layer_push: &mut layer_push,
                        layer_pop: &mut layer_pop,
                        layer_id: layer,
                    };

                    let res = self.layers[layer].handle_event(&mut state, &event);
                    match res {
                        EventResult::Handled => continue 'event,
                        EventResult::Exit => break 'main,
                        EventResult::Ignored => (),
                    }
                }

                if default_event_handler(&mut self.state, &event) == EventResult::Exit {
                    break 'main;
                }
            }

            // run updates
            while accumulator >= DT {
                for &layer in &self.update_order {
                    let mut state = EngineStateRef {
                        state: &mut self.state,
                        layer_push: &mut layer_push,
                        layer_pop: &mut layer_pop,
                        layer_id: layer,
                    };
                    self.layers[layer].update(&mut state, DT);
                }

                accumulator -= DT;
                self.state.run_time += DT;
            }

            // render a scene
            for &layer in &self.render_order {
                let mut state = EngineStateRef {
                    state: &mut self.state,
                    layer_push: &mut layer_push,
                    layer_pop: &mut layer_pop,
                    layer_id: layer,
                };
                self.layers[layer].render(&mut state);
            }

            // update layers
            if !layer_pop.is_empty() || !layer_push.is_empty() {
                layer_pop.sort_unstable();

                for &layer in layer_pop.iter().rev() {
                    self.layers.remove(layer);
                }

                for layer in layer_push.drain(..) {
                    self.layers.push(layer);
                }

                self.set_call_order();

                layer_pop.clear();
            }

            self.state.window.swap_window();
        }

        Ok(())
    }

    /// Re-calculate the update and render orders for the main loop - takes into
    /// account whether layers updates and renders are deferred or not.
    fn set_call_order(&mut self) {
        self.update_order.clear();
        self.render_order.clear();

        let mut deferred_update = vec![];
        let mut deferred_render = vec![];

        for (i, layer) in self.layers.iter().enumerate() {
            if layer.update_order() == CallOrder::Deferred {
                deferred_update.push(i);
            } else {
                self.update_order.push(i);
            }

            if layer.render_order() == CallOrder::Deferred {
                deferred_render.push(i);
            } else {
                self.render_order.push(i);
            }
        }

        for i in deferred_update.into_iter().rev() {
            self.update_order.push(i);
        }

        for i in deferred_render.into_iter().rev() {
            self.render_order.push(i);
        }
    }
}

fn default_event_handler(state: &mut EngineState, event: &Event) -> EventResult {
    match event {
        Event::Quit { .. } => return EventResult::Exit,
        Event::KeyDown { key, .. } => {
            state.inputs.set_key_state(*key, KeyState::Down);
        }
        Event::KeyUp { key, .. } => {
            state.inputs.set_key_state(*key, KeyState::Up);
        }
        Event::Scroll { x: dx, y: dy, .. } => {
            state.inputs.set_wheel_delta(*dx, *dy);

            let (x, y) = state.inputs.wheel_position();
            state.inputs.set_wheel_position(x + *dx, y + *dy);
        }
        _ => (),
    }

    EventResult::Ignored
}

/// attach console print debugging to the provided OpenGL Context
#[cfg(debug_assertions)]
fn enable_gl_debugging(gl: &gl::Gl) {
    let mut flags = 0;
    unsafe {
        gl.GetIntegerv(gl::CONTEXT_FLAGS, &mut flags);
    }

    // Only set the debugging options if debugging enabled on the context
    if flags as u32 & gl::CONTEXT_FLAG_DEBUG_BIT == 0 {
        return;
    }

    unsafe {
        // enables debug output
        gl.Enable(gl::DEBUG_OUTPUT);

        // ensure that debugging messages are only output on the main thread
        // ensures that the log function is called in the same order that the
        // messages are generated
        gl.Enable(gl::DEBUG_OUTPUT_SYNCHRONOUS);

        // set the debug call back, with no context pointer
        gl.DebugMessageCallback(Some(gl_debug_log), std::ptr::null());

        // tell the driver that we want all possible debug messages
        gl.DebugMessageControl(
            gl::DONT_CARE,
            gl::DONT_CARE,
            gl::DONT_CARE,
            0,
            std::ptr::null(),
            gl::TRUE,
        );
    }
}

/// Debugging callback
#[cfg(debug_assertions)]
extern "system" fn gl_debug_log(
    source: gl::types::GLenum,
    gltype: gl::types::GLenum,
    id: gl::types::GLuint,
    severity: gl::types::GLenum,
    _length: gl::types::GLsizei,
    message: *const gl::types::GLchar,
    _user_param: *mut gl::types::GLvoid,
) {
    // id of trivial, non error/warning information messages
    // not worth printing, would obscure actual errors
    if id == 0x20071 || id == 0x20084 {
        return;
    }

    println!("----------------");
    println!(
        "OpenGL {1} - {0:#x}:",
        id,
        match gltype {
            gl::DEBUG_TYPE_ERROR => "Error",
            gl::DEBUG_TYPE_DEPRECATED_BEHAVIOR => "Deprecated Behaviour",
            gl::DEBUG_TYPE_UNDEFINED_BEHAVIOR => "Undefined Behaviour",
            gl::DEBUG_TYPE_PORTABILITY => "Portability",
            gl::DEBUG_TYPE_PERFORMANCE => "Performance",
            gl::DEBUG_TYPE_MARKER => "Marker",
            gl::DEBUG_TYPE_PUSH_GROUP => "Push Group",
            gl::DEBUG_TYPE_POP_GROUP => "Pop Group",
            _ => "Other",
        }
    );

    // cast message from null terminated string, to rust types, is
    // guaranteed to be correctly null terminated by the standard,
    // assume that holds
    let message = unsafe { std::ffi::CStr::from_ptr(message) };

    println!("Message: {}", message.to_string_lossy());

    println!(
        "Severity: {}",
        match severity {
            gl::DEBUG_SEVERITY_HIGH => "high",
            gl::DEBUG_SEVERITY_MEDIUM => "medium",
            gl::DEBUG_SEVERITY_LOW => "low",
            gl::DEBUG_SEVERITY_NOTIFICATION => "notification",
            _ => "other",
        }
    );

    println!(
        "Source: {}",
        match source {
            gl::DEBUG_SOURCE_API => "API",
            gl::DEBUG_SOURCE_WINDOW_SYSTEM => "Window System",
            gl::DEBUG_SOURCE_SHADER_COMPILER => "Shader Compiler",
            gl::DEBUG_SOURCE_THIRD_PARTY => "Third Party",
            gl::DEBUG_SOURCE_APPLICATION => "Application",
            _ => "Other",
        }
    );
}
