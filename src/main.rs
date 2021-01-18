use anyhow::Result;
use engine::{
    data, gl, glm, gltf,
    resources::Resources,
    sdl2::{self, keyboard::Scancode},
    Camera, EngineState, EventResult, Layer, MainLoop, Model,
};
use gl_derive::VertexAttribPointers;
use std::path::Path;

#[derive(VertexAttribPointers, Copy, Clone, Debug)]
#[repr(C, packed)]
struct Vertex {
    #[location = 0]
    pos: data::f32_f32_f32,

    #[location = 1]
    uv: data::f32_f32,
}

struct Triangle {
    camera: Camera,
    model: Model,
}

impl Layer for Triangle {
    fn new(state: &EngineState) -> Result<Self> {
        let res = Resources::from_exe_path(Path::new("assets"))?;

        let model = gltf::Model::from_res(&res, "sea_keep_lonely_watcher/scene.gltf")?;
        let mut model = Model::new(model, &res, "sea_keep_lonely_watcher")?;
        model.load_vram(&state.gl)?;

        let (width, height) = state.window.size();

        unsafe {
            state.gl.Viewport(0, 0, width as i32, height as i32);
            state.gl.ClearColor(0.3, 0.3, 0.5, 1.0);
            state.gl.Enable(gl::DEPTH_TEST);
            //state.gl.PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
        }

        Ok(Triangle {
            model,
            camera: Camera::new(),
        })
    }

    fn handle_event(&mut self, state: &EngineState, event: &sdl2::event::Event) -> EventResult {
        use sdl2::event::{Event, WindowEvent};
        match event {
            Event::Window { win_event, .. } => {
                match win_event {
                    WindowEvent::Resized(w, h) => {
                        unsafe {
                            state.gl.Viewport(0, 0, *w, *h);
                        }
                    },

                    // fix issue with mouse movement being limited if the window loses
                    // and regains focus
                    WindowEvent::FocusGained => {
                        state.sdl.mouse().capture(true);
                    },
                    WindowEvent::FocusLost => {
                        state.sdl.mouse().capture(false);
                    },
                    _ => (),
                }
                EventResult::Handled
            }

            Event::KeyDown {
                scancode: Some(Scancode::Escape),
                ..
            } => EventResult::Exit,
            _ => EventResult::Ignored,
        }
    }

    fn update(&mut self, state: &EngineState, _time: f32, dt: f32) {
        self.camera.update(state, dt);
    }

    fn render(&mut self, state: &EngineState) {
        unsafe {
            state.gl.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }

        let (width, height) = state.window.size();

        let projection = glm::perspective(
            width as f32 / height as f32,
            self.camera.get_fov(),
            0.1,
            10000.0,
        );

        let view = self.camera.get_view();

        self.model.render(&state.gl, &projection, &view);
    }
}

fn main() {
    if let Err(e) = run() {
        println!("{}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut main_loop = MainLoop::new()?;
    main_loop.add_layer::<Triangle>()?;
    //main_loop.add_layer::<engine::imgui::ImguiLayer>()?;

    main_loop.run()?;

    Ok(())
}
