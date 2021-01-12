use thiserror::Error;
use anyhow::Result;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Error, Debug)]
enum SdlError {
    #[error("Error while initialising SDL2: {reason}")]
    Init { reason: String },

    #[error("Error while initialising video subsystem: {reason}")]
    Video { reason: String },

    #[error("Error while initialising OpenGl Context: {reason}")]
    GlContext { reason: String },

    #[error("Error while intialising SLD2 event pump: {reason}")]
    Event { reason: String },
}

pub struct EngineState {
    pub window: sdl2::video::Window,
    pub gl: gl::Gl,
    pub mouse_state: sdl2::mouse::MouseState,
    pub video: sdl2::VideoSubsystem,
}

pub struct MainLoop {
    layers: Vec<Rc<RefCell<dyn crate::Layer>>>,
    state: EngineState,
    events: sdl2::EventPump,
    _ctx: sdl2::video::GLContext,
}

impl MainLoop {
    pub fn new() -> Result<Self> {
        let sdl = sdl2::init().map_err(|e| SdlError::Init { reason: e })?;

        let video = sdl.video().map_err(|e| SdlError::Video { reason: e })?;

        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(4, 5);

        let window = video
            .window("Game", 900, 700)
            .opengl()
            .resizable()
            .build()?;

        let ctx = window
            .gl_create_context()
            .map_err(|e| SdlError::GlContext { reason: e })?;
        let gl = gl::Gl::load_with(|s| video.gl_get_proc_address(s) as _);

        let events = sdl
            .event_pump()
            .map_err(|e| SdlError::Event { reason: e })?;

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
            }
        })
    }

    pub fn add_layer<T: 'static + crate::Layer>(&mut self) -> Result<()> {
        let layer = T::new(&self.state)?;
        self.layers.push(Rc::new(RefCell::new(layer)));

        Ok(())
    }

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
