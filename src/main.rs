use anyhow::Result;
use engine::{
    data, gl, glm,
    model::{GLModel, Model},
    resources::Resources,
    window::{event::Event, scancode::Scancode, sdl_window::SdlWindow},
    Camera, EngineState, EventResult, Layer, MainLoop,
};
use gl_derive::VertexAttribPointers;
use native_dialog::FileDialog;
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
    gl_data: GLModel,
}

impl Triangle {
    fn swap_model(&mut self, state: &mut EngineState) -> Option<()> {
        let result = FileDialog::new()
            .add_filter("glTF Model", &["gltf", "glb"])
            .show_open_single_file();

        let path = match result {
            Ok(Some(path)) => path,
            _ => return None,
        };

        let folder = path.parent()?;
        let file = path.file_name()?.to_str()?;

        let res = Resources::from_path(&folder);

        let model = match Model::from_res(&res, file) {
            Ok(model) => model,
            Err(e) => {
                println!("error: {}", e);
                return None;
            }
        };

        self.gl_data = match GLModel::new(&model, &state.gl) {
            Ok(g) => g,
            Err(e) => {
                println!("error {}", e);
                return None;
            }
        };

        self.model = model;
        self.set_cam_pos();

        Some(())
    }

    fn set_cam_pos(&mut self) {
        let bound = self.model.get_bounds();

        self.camera.set_pos(&glm::vec3(
            (bound.max_x + bound.min_x) / 2.0,
            (bound.max_y + bound.min_y) / 2.0,
            bound.max_z * 2.0,
        ));

        let x = bound.max_x - bound.min_x;
        let y = bound.max_y - bound.min_y;
        let z = bound.max_z - bound.min_z;

        // use max, not min so the camera can still move if one of the dimensions
        // of the mesh is equal to 0
        self.camera.set_speed(x.max(y).max(z) / 5.0)
    }
}

impl Layer for Triangle {
    fn new(state: &EngineState) -> Result<Self> {
        let res = Resources::from_exe_path(Path::new("assets"))?;

        let model = Model::from_res(&res, "sea_keep_lonely_watcher/scene.gltf")?;
        let gl_data = GLModel::new(&model, &state.gl)?;

        let (width, height) = state.window.size();

        unsafe {
            state.gl.Viewport(0, 0, width as i32, height as i32);
            state.gl.ClearColor(0.3, 0.3, 0.5, 1.0);
            state.gl.Enable(gl::DEPTH_TEST);
            //state.gl.PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
        }

        let mut this = Triangle {
            model,
            gl_data,
            camera: Camera::new(),
        };

        this.set_cam_pos();

        Ok(this)
    }

    fn handle_event(&mut self, state: &mut EngineState, event: &Event) -> EventResult {
        match event {
            Event::Resize { width, height, .. } => unsafe {
                state.gl.Viewport(0, 0, *width as _, *height as _);
                EventResult::Handled
            },

            // fix issue with mouse movement being limited if the window loses
            // and regains focus
            Event::FocusGained => {
                //state.window.set_mouse_capture(true);
                EventResult::Handled
            }
            Event::FocusLost => {
                //state.window.set_mouse_capture(false);
                EventResult::Handled
            }

            Event::KeyDown {
                key: Scancode::Escape,
                ..
            } => EventResult::Exit,

            Event::KeyDown {
                key: Scancode::O, ..
            } => {
                self.swap_model(state);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }

    fn update(&mut self, state: &EngineState, dt: f32) {
        self.camera.update(state, dt);
    }

    fn render(&mut self, state: &EngineState) {
        unsafe {
            state.gl.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }

        let (width, height) = state.window.size();

        let proj = glm::perspective(
            width as f32 / height as f32,
            self.camera.get_fov(),
            0.1,
            10000.0,
        );

        let view = self.camera.get_view();

        self.gl_data.render(&self.model, &state.gl, &proj, &view);
    }
}

fn main() {
    if let Err(e) = run() {
        println!("{}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut main_loop = MainLoop::new::<SdlWindow>()?;
    main_loop.add_layer::<Triangle>()?;
    main_loop.add_layer::<engine::imgui::ImguiLayer>()?;

    main_loop.run()?;

    Ok(())
}
