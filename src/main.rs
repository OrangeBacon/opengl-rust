use anyhow::Result;
use engine::{
    camera::{CameraData, CameraRender},
    data, gl, glm, gltf,
    render::texture::TextureRenderCache,
    resources::Resources,
    window::{event::Event, scancode::Scancode, sdl_window::SdlWindow},
    EngineState, EngineUpdateState, EventResult, MainLoop, Model, Renderer, Updater,
};
use gl_derive::VertexAttribPointers;
use native_dialog::FileDialog;
use std::{ffi::OsStr, path::Path};

#[derive(VertexAttribPointers, Copy, Clone, Debug)]
#[repr(C, packed)]
struct Vertex {
    #[location = 0]
    pos: data::f32_f32_f32,

    #[location = 1]
    uv: data::f32_f32,
}

struct Triangle {
    camera: CameraData,
    model: Model,
}

#[derive(Default)]
struct TriangleRender {
    camera: CameraRender,
}

impl Triangle {
    fn swap_model(&mut self, gl: &gl::Gl) {
        let result = FileDialog::new()
            .add_filter("glTF Model", &["gltf", "glb"])
            .show_open_single_file();

        let path = match result {
            Ok(Some(path)) => path,
            _ => return,
        };

        let folder = match path.parent() {
            Some(p) => p,
            _ => return,
        };

        let file = match path.file_name() {
            Some(n) => n,
            _ => return,
        };

        let res = Resources::new(folder.to_path_buf());

        let model = match Triangle::load_model(gl, &res, file) {
            Ok(model) => model,
            Err(e) => {
                println!("{}", e);
                return;
            }
        };

        self.model = model;
    }

    fn load_model(gl: &gl::Gl, res: &Resources, path: &OsStr) -> Result<Model> {
        let model = gltf::Model::from_res(&res, path)?;

        let mut model = Model::new(model, &res, "./")?;
        model.load_vram(gl)?;

        println!("{:?}", model);

        Ok(model)
    }
}

impl Updater<TriangleRender> for Triangle {
    fn new(state: &EngineState) -> Result<Self> {
        let res = Resources::from_exe_path(Path::new("assets"))?;

        let model = gltf::Model::from_res(&res, "sea_keep_lonely_watcher/scene.gltf")?;
        let mut model = Model::new(model, &res, "sea_keep_lonely_watcher")?;
        model.load_vram(&state.gl)?;

        Ok(Triangle {
            model,
            camera: CameraData::new(),
        })
    }

    fn handle_event(&mut self, state: &mut EngineUpdateState, event: &Event) -> EventResult {
        match event {
            Event::KeyDown {
                key: Scancode::O, ..
            } => {
                self.swap_model(state.gl);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }

    fn update(&mut self, state: &mut EngineUpdateState, data: &TriangleRender, dt: f32) {
        self.camera.update(state, &mut data.camera, dt);
    }
}

struct OpenGlRenderer {
    textures: TextureRenderCache,
}

impl Renderer<TriangleRender> for OpenGlRenderer {
    fn new(state: &EngineState) -> Result<Self>
    where
        Self: Sized,
    {
        let (width, height) = state.window.size();

        unsafe {
            state.gl.Viewport(0, 0, width as i32, height as i32);
            state.gl.ClearColor(0.3, 0.3, 0.5, 1.0);
            state.gl.Enable(gl::DEPTH_TEST);
            //state.gl.PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
        }

        Ok(OpenGlRenderer {
            textures: TextureRenderCache::new(),
        })
    }

    fn render(&mut self, state: &EngineState, data: &TriangleRender) {
        unsafe {
            state.gl.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }

        let (width, height) = state.window.size();

        let projection = glm::perspective(
            width as f32 / height as f32,
            data.camera.fov(),
            0.1,
            10000.0,
        );

        let view = data.camera.view();

        self.model.render(&state.gl, &projection, &view);
    }

    fn handle_event(&mut self, graphics: &EngineState, event: &Event) -> EventResult {
        match event {
            Event::Resize { width, height, .. } => unsafe {
                graphics.gl.Viewport(0, 0, *width as _, *height as _);
                EventResult::Handled
            },

            // fix issue with mouse movement being limited if the window loses
            // and regains focus
            Event::FocusGained => {
                graphics.window.set_mouse_capture(true);
                EventResult::Handled
            }
            Event::FocusLost => {
                graphics.window.set_mouse_capture(false);
                EventResult::Handled
            }

            Event::KeyDown {
                key: Scancode::Escape,
                ..
            } => EventResult::Exit,

            _ => EventResult::Ignored,
        }
    }
}

fn main() {
    if let Err(e) = run() {
        println!("{}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut main_loop = MainLoop::<_, TriangleRender, Triangle>::new::<SdlWindow>()?;

    main_loop.run()?;

    Ok(())
}
