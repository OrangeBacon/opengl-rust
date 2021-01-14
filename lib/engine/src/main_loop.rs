use anyhow::Result;
use gl::types::*;
use sdl2::{keyboard::Scancode, mouse::{MouseButton, MouseState, MouseWheelDirection}};
use std::{cell::RefCell, collections::HashMap, ptr, rc::Rc, time::Instant};
use thiserror::Error;

use crate::EventResult;

/// Error type used during initialisation of SDL2 - the default bindings only
/// output `String`, so this type annotates the string with the function that
/// generated the error string and is used to make the string a proper error
/// type, `anyhow::Error`
#[derive(Error, Debug)]
enum SdlError {
    #[error("Error while initialising SDL2: {reason}")]
    Init { reason: String },

    #[error("Error while initialising video subsystem: {reason}")]
    Video { reason: String },

    #[error("Error while initialising OpenGl Context: {reason}")]
    GlContext { reason: String },

    #[error("Error while initialising SLD2 event pump: {reason}")]
    Event { reason: String },
}

/// Current state of a key
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyState {

    /// The key is not currently pressed
    None,

    /// The key was pressed down on this frame
    Down,

    /// The key is being held down
    Hold,

    /// The key was released on this frame
    Up,
}

/// The state of the user input on the current frame
pub struct InputState {

    /// Current mouse horizontal position
    pub x: i32,

    /// Current mouse vertical position
    pub y: i32,

    /// How much the mouse moved horizontally this frame
    pub delta_x: i32,

    /// How much the mouse moved vertically this frame
    pub delta_y: i32,

    /// The current mouse wheel horizontal location
    pub wheel_x: i32,

    /// The current mouse wheel vertical location
    pub wheel_y: i32,

    /// How much the mouse wheel moved horizontally this frame
    pub wheel_delta_x: i32,

    /// How much the mouse wheel moved vertically this frame
    pub wheel_delta_y: i32,

    /// The state of the mouse buttons
    mouse_buttons: HashMap<MouseButton, bool>,

    /// The state of the keyboard
    keys: HashMap<Scancode, KeyState>,

    pub mouse_state: MouseState,
}

impl InputState {
    /// Get the current state of a key
    pub fn get_key_state(&self, key: Scancode) -> KeyState {
        *self.keys.get(&key).unwrap_or(&KeyState::None)
    }

    /// Is a key currently pressed down
    pub fn is_key_pressed(&self, key: Scancode) -> bool {
        let key = *self.keys.get(&key).unwrap_or(&KeyState::None);
        if key == KeyState::Down || key == KeyState::Hold {
            true
        } else {
            false
        }
    }

    /// Get the current state of a mouse button
    pub fn get_mouse_button(&self, button: MouseButton) -> bool {
        *self.mouse_buttons.get(&button).unwrap_or(&false)
    }

    /// updates the state for a new frame
    pub fn update(&mut self, mouse_state: MouseState) {
        let x = mouse_state.x();
        let y = mouse_state.y();

        self.delta_x = x - self.x;
        self.delta_y = y - self.y;
        self.x = x;
        self.y = y;

        self.wheel_delta_x = 0;
        self.wheel_delta_y = 0;

        self.mouse_buttons = mouse_state.mouse_buttons().collect();

        self.keys = self.keys.iter().map(|(scan, state)| {
            let new_state = match state {
                KeyState::Down => KeyState::Hold,
                KeyState::Up => KeyState::None,
                a => *a
            };
            (*scan, new_state)
        }).collect();

        self.mouse_state = mouse_state;
    }
}

/// Graphics api state that is available to render layers.
pub struct EngineState {
    /// Main window rendered to.
    pub window: sdl2::video::Window,

    /// Primary OpenGL context, used when rendering to the main window.
    pub gl: gl::Gl,

    /// The state of all keyboard and mouse inputs
    pub inputs: InputState,

    /// SDL2 video system, used for getting window properties,
    /// including the OpenGL context loader, clipboard and text input.
    pub video: sdl2::VideoSubsystem,

    /// The total time the program has been running in seconds
    pub run_time: f32,
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

    /// The event system for the main window stored in the engine state.
    events: sdl2::EventPump,

    /// The current OpenGL context. This struct will likely never be read,
    /// however if it is dropped, then the context will be deleted, causing all
    /// rendering operations to fail, therefore it is kept here to extend its
    /// lifetime to be the same as the rest of the graphics state
    _ctx: sdl2::video::GLContext,
}

impl MainLoop {
    /// Try to create a new game engine instance, using default settings
    /// Currently there is no way to change the settings used, this should
    /// probably be changed.  This method initialises the graphics and creates
    /// a window that will be shown to the user.
    pub fn new() -> Result<Self> {
        // initialise graphics library
        let sdl = sdl2::init().map_err(|e| SdlError::Init { reason: e })?;

        // enable graphics output
        let video = sdl.video().map_err(|e| SdlError::Video { reason: e })?;

        // set which OpenGL version is requested (OpenGL core 4.5)
        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(4, 5);

        // set the context to be in debug mode when the crate is compiled in
        // debug mode, enables the OpenGL debugging callback
        #[cfg(debug_assertions)]
        gl_attr.set_context_flags().debug().set();

        // Configure and create a new window
        // Todo: make these configuration options (or similar), not hardcoded
        let window = video
            .window("Game", 900, 700)
            .opengl()
            .resizable()
            .build()?;

        /*let mouse = sdl.mouse();
        mouse.capture(true);
        mouse.set_relative_mouse_mode(true);*/

        // Enable OpenGL for the main window
        let ctx = window
            .gl_create_context()
            .map_err(|e| SdlError::GlContext { reason: e })?;

        // Tell OpenGL where to find its functions
        let gl = gl::Gl::load_with(|s| video.gl_get_proc_address(s) as _);

        // connect debug hooks if in debug mode
        #[cfg(debug_assertions)]
        enable_gl_debugging(&gl);

        // Initialise the event pump here, not in the run function so the
        // mouse state can be returned
        let events = sdl
            .event_pump()
            .map_err(|e| SdlError::Event { reason: e })?;

        // A mouse state is required in initialisation so that the struct
        // field is initialised, it will likely be overwritten during the
        // first frame
        let mouse_state = events.mouse_state();

        let inputs = InputState {
            x: mouse_state.x(),
            y: mouse_state.y(),

            delta_x: 0,
            delta_y: 0,

            wheel_x: 0,
            wheel_y: 0,

            wheel_delta_x: 0,
            wheel_delta_y: 0,

            mouse_buttons: HashMap::new(),
            keys: HashMap::new(),
            mouse_state: mouse_state,
        };

        Ok(MainLoop {
            events,
            layers: vec![],
            _ctx: ctx,
            state: EngineState {
                gl,
                window,
                video,
                inputs,
                run_time: 0.0,
            },
        })
    }

    /// Adds a new layer to the renderer. Initialises the layer based upon
    /// the engine state.  Todo: allow layers to be configured based upon
    /// other settings, depending on the layer.
    pub fn add_layer<T: 'static + crate::Layer>(&mut self) -> Result<()> {
        let layer = T::new(&self.state)?;
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

            self.state.inputs.update(self.events.mouse_state());

            for event in self.events.poll_iter() {
                for layer in self.layers.iter_mut() {
                    let res = layer.borrow_mut().handle_event(&self.state, &event);
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
            self.state.window.gl_swap_window();
        }

        Ok(())
    }
}

fn default_event_handler(state: &mut EngineState, event: &sdl2::event::Event) -> EventResult {
    use sdl2::event::Event;
    match event {
        Event::Quit { .. } => return EventResult::Exit,
        Event::KeyDown { scancode: Some(scan), .. } => {
            state.inputs.keys.insert(*scan, KeyState::Down);
        }
        Event::KeyUp { scancode: Some(scan), .. } => {
            state.inputs.keys.insert(*scan, KeyState::Up);
        }
        Event::MouseWheel { x, y, direction, .. } => {
            let x = if *direction == MouseWheelDirection::Flipped {
                y
            } else {
                x
            };

            let y = if *direction == MouseWheelDirection::Flipped {
                x
            } else {
                y
            };

            state.inputs.wheel_delta_x = *x;
            state.inputs.wheel_delta_y = *y;
            state.inputs.wheel_x += *x;
            state.inputs.wheel_y += *y;
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
        gl.DebugMessageCallback(Some(gl_debug_log), ptr::null());

        // tell the driver that we want all possible debug messages
        gl.DebugMessageControl(
            gl::DONT_CARE,
            gl::DONT_CARE,
            gl::DONT_CARE,
            0,
            ptr::null(),
            gl::TRUE,
        );
    }
}

/// Debugging callback
#[cfg(debug_assertions)]
extern "system" fn gl_debug_log(
    source: GLenum,
    gltype: GLenum,
    id: GLuint,
    severity: GLenum,
    _length: GLsizei,
    message: *const GLchar,
    _user_param: *mut GLvoid,
) {
    // id of trivial, non error/warning information messages
    // not worth printing, would obscure actual errors
    if id == 0x20071 {
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
