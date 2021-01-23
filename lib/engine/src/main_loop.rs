use anyhow::Result;
use std::{cell::RefCell, rc::Rc, time::Instant};

use crate::{
    window::{
        event::Event,
        input::{InputState, KeyState},
        window::{Window, WindowConfig},
    },
    EventResult,
};

/// Graphics api state that is available to render layers.
pub struct EngineState {
    /// Primary OpenGL context, used when rendering to the main window.
    pub gl: gl::Gl,

    /// The state of all keyboard and mouse inputs
    pub inputs: InputState,

    /// The total time the program has been running in seconds
    pub run_time: f32,

    pub window: Box<dyn Window>,
}

/// The main game storage, is used to call the main loop.
pub struct MainLoop {
    /// Vector of everything in the render state of the game.
    /// I couldn't get rust to have a vec of mutable trait objects without
    /// using `Rc<RefCell<...>>`, in reality these objects should only ever
    /// have one owner, this vector
    layers: Vec<Rc<RefCell<dyn crate::Layer>>>,

    /// The current graphics state.
    state: EngineState,
}

impl MainLoop {
    /// Try to create a new game engine instance, using default settings
    /// Currently there is no way to change the settings used, this should
    /// probably be changed.  This method initialises the graphics and creates
    /// a window that will be shown to the user.
    pub fn new<T: Window + 'static>() -> Result<Self> {
        let config = WindowConfig {
            debug: true,
            gl_version: (4, 5),
            width: 900,
            height: 700,
            resizable: true,
            title: "Game",
        };

        let mut window = T::new(config)?;

        let gl = window.new_gl_context()?;

        enable_gl_debugging(&gl);

        Ok(MainLoop {
            layers: vec![],
            state: EngineState {
                gl,
                window: Box::new(window),
                inputs: Default::default(),
                run_time: 0.0,
            },
        })
    }

    /// Adds a new layer to the renderer. Initialises the layer based upon
    /// the engine state.  Todo: allow layers to be configured based upon
    /// other settings, depending on the layer.
    pub fn add_layer<L: 'static + crate::Layer>(&mut self) -> Result<()> {
        let layer = L::new(&self.state)?;
        self.layers.push(Rc::new(RefCell::new(layer)));

        Ok(())
    }

    /// Start the game loop, this function will not return until the main window
    /// is closed, only saving, error reporting, etc should happen afterwards.
    pub fn run(mut self) -> Result<()> {
        let mut t = 0.0;
        const DT: f32 = 1.0 / 60.0;
        let mut current_time = Instant::now();
        let mut accumulator = 0.0;

        'main: loop {
            let new_time = Instant::now();
            let frame_time = (new_time - current_time).as_secs_f32();
            current_time = new_time;

            accumulator += frame_time;
            self.state.run_time += frame_time;

            self.state.window.update_mouse(&mut self.state.inputs);

            while let Some(event) = self.state.window.event() {
                for layer in self.layers.iter_mut() {
                    let res = layer.borrow_mut().handle_event(&mut self.state, &event);
                    match res {
                        EventResult::Handled => break,
                        EventResult::Exit => break 'main,
                        EventResult::Ignored => (),
                    }
                }

                if default_event_handler(&mut self.state, &event) == EventResult::Exit {
                    break 'main;
                }
            }

            while accumulator >= DT {
                for layer in self.layers.iter_mut() {
                    layer.borrow_mut().update(&self.state, t, DT);
                }

                accumulator -= DT;
                t += DT;
            }

            for layer in self.layers.iter_mut() {
                layer.borrow_mut().render(&self.state);
            }
            self.state.window.swap_window();
        }

        Ok(())
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
