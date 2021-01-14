use anyhow::Result;
use gl::types::*;
use std::cell::RefCell;
use std::{ptr, rc::Rc};
use thiserror::Error;

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

/// Graphics api state that is available to render layers.
pub struct EngineState {
    /// Main window rendered to.
    pub window: sdl2::video::Window,

    /// Primary OpenGL context, used when rendering to the main window.
    pub gl: gl::Gl,

    /// The mouse state at the start of the current frame.
    pub mouse_state: sdl2::mouse::MouseState,

    /// SDL2 video system, used for getting window properties,
    /// including the OpenGL context loader, clipboard and text input.
    pub video: sdl2::VideoSubsystem,
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
    /// This should probably be stored inside the engine state, but due to it
    /// not being `Copy`, I kept getting double mutable borrow errors.
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

        Ok(MainLoop {
            events,
            layers: vec![],
            _ctx: ctx,
            state: EngineState {
                gl,
                window,
                video,
                mouse_state,
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
        'main: loop {
            for event in self.events.poll_iter() {
                for layer in self.layers.iter_mut() {
                    if layer.borrow_mut().handle_event(&event, &self.state) {
                        break 'main;
                    }
                }
            }

            self.state.mouse_state = self.events.mouse_state();

            for layer in self.layers.iter_mut() {
                layer.borrow_mut().render(&self.state);
            }
            self.state.window.gl_swap_window();
        }

        Ok(())
    }
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
