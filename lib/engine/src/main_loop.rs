use anyhow::Result;
use mpsc::{Receiver, Sender};
use std::{marker::PhantomData, sync::mpsc, time::Instant};

use crate::{
    scene::triple_buffer::{TripleBuffer, TripleBufferReader, TripleBufferWriter},
    window::{
        event::Event,
        input::{InputState, KeyState},
        window::{Window, WindowConfig},
    },
    EventResult, Renderer, Updater,
};

/// Graphics api state that is available to render layers.
pub struct EngineState {
    /// Primary OpenGL context, used when rendering to the main window.
    pub gl: gl::Gl,

    pub window: Box<dyn Window>,
}

pub struct EngineUpdateState {
    /// The state of all keyboard and mouse inputs
    pub inputs: InputState,

    /// The total time the program has been running in seconds
    pub run_time: f32,
}

/// The main game storage, is used to call the main loop.
pub struct MainLoop<T, R, U>
where
    T: Send + Default,
    R: Renderer<T>,
    U: Updater<T>,
{
    renderer: R,
    updater: U,

    _state_type: PhantomData<T>,

    /// The current graphics state.
    state: EngineState,
}

impl<T, R, U> MainLoop<T, R, U>
where
    T: Send + Default + 'static,
    R: Renderer<T> + 'static,
    U: Updater<T> + Send + 'static,
{
    /// Try to create a new game engine instance, using default settings
    /// Currently there is no way to change the settings used, this should
    /// probably be changed.  This method initialises the graphics and creates
    /// a window that will be shown to the user.
    pub fn new<W: Window + 'static>() -> Result<Self> {
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

        enable_gl_debugging(&gl);

        let state = EngineState {
            gl,
            window: Box::new(window),
        };

        let renderer = R::new(&state)?;
        let updater = U::new(&state)?;

        Ok(MainLoop {
            renderer,
            updater,
            state,
            _state_type: PhantomData::default(),
        })
    }

    /// Start the game loop, this function will not return until the main window
    /// is closed, only saving, error reporting, etc should happen afterwards.
    pub fn run(self) -> Result<()> {
        let (read, write) = TripleBuffer::new();
        let (event_tx, event_rx) = mpsc::channel();

        let Self {
            renderer,
            updater,
            state,
            ..
        } = self;

        let update = std::thread::spawn(Self::update_loop(updater, write, event_rx));

        Self::render_loop(state, renderer, read, event_tx)?;

        update.join().unwrap()?;

        Ok(())
    }

    pub fn update_loop(
        mut updater: U,
        write: TripleBufferWriter<T>,
        event_rx: Receiver<Event>,
    ) -> impl FnMut() -> Result<()> {
        move || {
            let mut state = EngineUpdateState {
                inputs: Default::default(),
                run_time: 0.0,
            };

            const DT: f32 = 1.0 / 60.0;
            let mut current_time = Instant::now();
            let mut accumulator = 0.0;

            'main: loop {
                let new_time = Instant::now();
                let frame_time = (new_time - current_time).as_secs_f32();
                current_time = new_time;

                accumulator += frame_time;

                let mut write = write.get_write()?;
                let write = write.state_mut()?;

                while let Ok(event) = event_rx.try_recv() {
                    updater.handle_event(&mut state, &event);
                    if default_event_handler(&mut state, &event) == EventResult::Exit {
                        break 'main;
                    }
                }

                while accumulator >= DT {
                    updater.update(&mut state, write, DT);

                    accumulator -= DT;
                    state.run_time += DT;
                }

                std::thread::sleep(std::time::Duration::from_secs(0));
            }

            Ok(())
        }
    }

    pub fn render_loop(
        mut graphics: EngineState,
        mut renderer: R,
        read: TripleBufferReader<T>,
        event_tx: Sender<Event>,
    ) -> Result<()> {
        loop {
            let mouse_event = graphics.window.update_mouse();
            let mouse_event = Event::MouseMove { data: mouse_event };
            match renderer.handle_event(&graphics, &mouse_event) {
                EventResult::Handled => (),
                EventResult::Ignored => event_tx.send(mouse_event)?,
                EventResult::Exit => break,
            }

            while let Some(event) = graphics.window.event() {
                match renderer.handle_event(&graphics, &event) {
                    EventResult::Handled => (),
                    EventResult::Ignored => event_tx.send(event)?,
                    EventResult::Exit => break,
                }
            }

            let read = read.get_read()?;
            let read = read.state()?;

            renderer.render(&graphics, read);

            graphics.window.swap_window();
        }

        Ok(())
    }
}

fn default_event_handler(state: &mut EngineUpdateState, event: &Event) -> EventResult {
    match event {
        Event::Quit { .. } => return EventResult::Exit,
        Event::KeyDown { key, .. } => {
            state.inputs.set_key_state(*key, KeyState::Down);
        }
        Event::KeyUp { key, .. } => {
            state.inputs.set_key_state(*key, KeyState::Up);
        }
        Event::Scroll { x: dx, y: dy, .. } => {
            state.inputs.mouse().set_wheel_delta(*dx, *dy);

            let (x, y) = state.inputs.mouse().wheel_position();
            state.inputs.mouse().set_wheel_position(x + *dx, y + *dy);
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
