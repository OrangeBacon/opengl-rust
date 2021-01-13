use anyhow::Result;
use engine::{
    buffer, data, gl, glm, resources::Resources, sdl2, EngineState, Layer, MainLoop, Program,
    Texture,
};
use gl_derive::VertexAttribPointers;
use std::{path::Path, time::Instant};

#[derive(VertexAttribPointers, Copy, Clone, Debug)]
#[repr(C, packed)]
struct Vertex {
    #[location = 0]
    pos: data::f32_f32_f32,

    #[location = 1]
    uv: data::f32_f32,
}

struct Triangle {
    _vbo: buffer::ArrayBuffer,
    vao: buffer::VertexArray,
    shader_program: Program,
    crate_tex: Texture,
    face_tex: Texture,
    start_time: Instant,
}

impl Layer for Triangle {
    fn new(state: &EngineState) -> Result<Self> {
        let res = Resources::from_exe_path(Path::new("assets"))?;
        let shader_program = Program::from_res(&state.gl, &res, "shaders/triangle")?;

        let crate_tex = Texture::from_res(&state.gl, &res, "container.jpg", 0)?;
        let face_tex = Texture::from_res(&state.gl, &res, "awesomeface.png", 1)?;

        #[rustfmt::skip]
        let vertices = vec![
            Vertex { pos: (-0.5, -0.5, -0.5).into(), uv: (0.0, 0.0).into() },
            Vertex { pos: ( 0.5, -0.5, -0.5).into(), uv: (1.0, 0.0).into() },
            Vertex { pos: ( 0.5,  0.5, -0.5).into(), uv: (1.0, 1.0).into() },
            Vertex { pos: ( 0.5,  0.5, -0.5).into(), uv: (1.0, 1.0).into() },
            Vertex { pos: (-0.5,  0.5, -0.5).into(), uv: (0.0, 1.0).into() },
            Vertex { pos: (-0.5, -0.5, -0.5).into(), uv: (0.0, 0.0).into() },

            Vertex { pos: (-0.5, -0.5,  0.5).into(), uv: (0.0, 0.0).into() },
            Vertex { pos: ( 0.5, -0.5,  0.5).into(), uv: (1.0, 0.0).into() },
            Vertex { pos: ( 0.5,  0.5,  0.5).into(), uv: (1.0, 1.0).into() },
            Vertex { pos: ( 0.5,  0.5,  0.5).into(), uv: (1.0, 1.0).into() },
            Vertex { pos: (-0.5,  0.5,  0.5).into(), uv: (0.0, 1.0).into() },
            Vertex { pos: (-0.5, -0.5,  0.5).into(), uv: (0.0, 0.0).into() },

            Vertex { pos: (-0.5,  0.5,  0.5).into(), uv: (1.0, 0.0).into() },
            Vertex { pos: (-0.5,  0.5, -0.5).into(), uv: (1.0, 1.0).into() },
            Vertex { pos: (-0.5, -0.5, -0.5).into(), uv: (0.0, 1.0).into() },
            Vertex { pos: (-0.5, -0.5, -0.5).into(), uv: (0.0, 1.0).into() },
            Vertex { pos: (-0.5, -0.5,  0.5).into(), uv: (0.0, 0.0).into() },
            Vertex { pos: (-0.5,  0.5,  0.5).into(), uv: (1.0, 0.0).into() },

            Vertex { pos: ( 0.5,  0.5,  0.5).into(), uv: (1.0, 0.0).into() },
            Vertex { pos: ( 0.5,  0.5, -0.5).into(), uv: (1.0, 1.0).into() },
            Vertex { pos: ( 0.5, -0.5, -0.5).into(), uv: (0.0, 1.0).into() },
            Vertex { pos: ( 0.5, -0.5, -0.5).into(), uv: (0.0, 1.0).into() },
            Vertex { pos: ( 0.5, -0.5,  0.5).into(), uv: (0.0, 0.0).into() },
            Vertex { pos: ( 0.5,  0.5,  0.5).into(), uv: (1.0, 0.0).into() },

            Vertex { pos: (-0.5, -0.5, -0.5).into(), uv: (0.0, 1.0).into() },
            Vertex { pos: ( 0.5, -0.5, -0.5).into(), uv: (1.0, 1.0).into() },
            Vertex { pos: ( 0.5, -0.5,  0.5).into(), uv: (1.0, 0.0).into() },
            Vertex { pos: ( 0.5, -0.5,  0.5).into(), uv: (1.0, 0.0).into() },
            Vertex { pos: (-0.5, -0.5,  0.5).into(), uv: (0.0, 0.0).into() },
            Vertex { pos: (-0.5, -0.5, -0.5).into(), uv: (0.0, 1.0).into() },

            Vertex { pos: (-0.5,  0.5, -0.5).into(), uv: (0.0, 1.0).into() },
            Vertex { pos: ( 0.5,  0.5, -0.5).into(), uv: (1.0, 1.0).into() },
            Vertex { pos: ( 0.5,  0.5,  0.5).into(), uv: (1.0, 0.0).into() },
            Vertex { pos: ( 0.5,  0.5,  0.5).into(), uv: (1.0, 0.0).into() },
            Vertex { pos: (-0.5,  0.5,  0.5).into(), uv: (0.0, 0.0).into() },
            Vertex { pos: (-0.5,  0.5, -0.5).into(), uv: (0.0, 1.0).into() },
        ];

        let vbo = buffer::ArrayBuffer::new(&state.gl);
        vbo.bind();
        vbo.static_draw_data(&vertices);
        vbo.unbind();

        let vao = buffer::VertexArray::new(&state.gl);
        vao.bind();
        vbo.bind();
        Vertex::attrib_pointers(&state.gl);
        vbo.unbind();
        vao.unbind();

        unsafe {
            state.gl.Viewport(0, 0, 900, 700);
            state.gl.ClearColor(0.3, 0.3, 0.5, 1.0);
            state.gl.Enable(gl::DEPTH_TEST);
        }

        Ok(Triangle {
            vao,
            crate_tex,
            face_tex,
            _vbo: vbo,
            shader_program,
            start_time: Instant::now(),
        })
    }

    fn handle_event(&mut self, event: &sdl2::event::Event, state: &EngineState) -> bool {
        match event {
            sdl2::event::Event::Quit { .. } => return true,
            sdl2::event::Event::Window { win_event, .. } => {
                if let sdl2::event::WindowEvent::Resized(w, h) = win_event {
                    unsafe {
                        state.gl.Viewport(0, 0, *w, *h);
                    }
                }
            }
            _ => (),
        }

        false
    }

    fn render(&mut self, state: &EngineState) {
        unsafe {
            state.gl.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }

        let (width, height) = state.window.size();

        let time = (Instant::now() - self.start_time).as_secs_f32();

        let view = glm::Mat4::identity();
        let view = glm::translate(&view, &glm::vec3(0.0, 0.0, -3.0));

        let projection = glm::perspective(
            45.0f32.to_radians(),
            width as f32 / height as f32,
            0.1,
            100.0,
        );

        let positions = [
            glm::vec3(0.0, 0.0, 0.0),
            glm::vec3(2.0, 5.0, -15.0),
            glm::vec3(-1.5, -2.2, -2.5),
            glm::vec3(-3.8, -2.0, -12.3),
            glm::vec3(2.4, -0.4, -3.5),
            glm::vec3(-1.7, 3.0, -7.5),
            glm::vec3(1.3, -2.0, -2.5),
            glm::vec3(1.5, 2.0, -2.5),
            glm::vec3(1.5, 0.2, -1.5),
            glm::vec3(-1.3, 1.0, -1.5),
        ];

        self.shader_program.set_used();
        self.shader_program.bind_texture("crate", &self.crate_tex);
        self.shader_program.bind_texture("face", &self.face_tex);
        self.shader_program.bind_matrix("view", view);
        self.shader_program.bind_matrix("projection", projection);
        self.vao.bind();

        for (i, pos) in positions.iter().enumerate() {
            let angle = 20.0 * (i as f32 + time);

            let model = glm::Mat4::identity();
            let model = glm::translate(&model, pos);
            let model = glm::rotate(&model, angle.to_radians(), &glm::vec3(1.0, 0.3, 0.5));

            self.shader_program.bind_matrix("model", model);
            unsafe {
                state.gl.DrawArrays(gl::TRIANGLES, 0, 36);
            }
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
    let mut main_loop = MainLoop::new()?;
    main_loop.add_layer::<Triangle>()?;
    main_loop.add_layer::<engine::imgui::ImguiLayer>()?;

    main_loop.run()?;

    Ok(())
}
