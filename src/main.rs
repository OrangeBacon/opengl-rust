use anyhow::Result;
use engine::{Camera, EngineState, EventResult, Layer, MainLoop, Model, Program, data, gl, glm, gltf, resources::Resources, sdl2::{self, keyboard::Scancode}};
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
    shader_program: Program,
    camera: Camera,
    model: Model,
}

impl Layer for Triangle {
    fn new(state: &EngineState) -> Result<Self> {
        let res = Resources::from_exe_path(Path::new("assets"))?;

        let model = gltf::Model::from_res(&state.gl, &res, "sea_keep_lonely_watcher/scene.gltf")?;
        let mut model = Model::new(model, &res, "sea_keep_lonely_watcher")?;
        model.load_vram(&state.gl)?;

        let shader_program = Program::from_res(&state.gl, &res, "shaders/triangle")?;

        let (width, height) = state.window.size();

        unsafe {
            state.gl.Viewport(0, 0, width as i32, height as i32);
            state.gl.ClearColor(0.3, 0.3, 0.5, 1.0);
            state.gl.Enable(gl::DEPTH_TEST);
            //state.gl.PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
        }

        Ok(Triangle {
            model,
            shader_program,
            camera: Camera::new(),
        })
    }

    fn handle_event(&mut self, state: &EngineState, event: &sdl2::event::Event) -> EventResult {
        use sdl2::event::{Event, WindowEvent};
        match event {
            Event::Window { win_event, .. } => {
                if let WindowEvent::Resized(w, h) = win_event {
                    unsafe {
                        state.gl.Viewport(0, 0, *w, *h);
                    }
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
            1000.0,
        );

        let view = self.camera.get_view();

        self.shader_program.set_used();
        self.shader_program.bind_matrix("view", view);
        self.shader_program.bind_matrix("projection", projection);
        self.model.render(&state.gl, &self.shader_program);

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
