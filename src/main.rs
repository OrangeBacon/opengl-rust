use anyhow::Result;
use engine::{
    buffer, data, gl, resources::Resources, sdl2, EngineState, Layer, MainLoop, Program, Texture, glm,
};
use gl_derive::VertexAttribPointers;
use std::{path::Path, ptr, time::Instant};

#[derive(VertexAttribPointers, Copy, Clone, Debug)]
#[repr(C, packed)]
struct Vertex {
    #[location = 0]
    pos: data::f32_f32_f32,

    #[location = 1]
    clr: data::f32_f32_f32,

    #[location = 2]
    uv: data::f32_f32,
}

struct Triangle {
    _vbo: buffer::ArrayBuffer,
    vao: buffer::VertexArray,
    ebo: buffer::ElementArrayBuffer,
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
        let vertices: Vec<Vertex> = vec![
            // positions                            // colours
            Vertex { pos: ( 0.5,  0.5, 0.0).into(), clr: (1.0, 0.0, 0.0).into(), uv: (1.0, 1.0).into() },
            Vertex { pos: ( 0.5, -0.5, 0.0).into(), clr: (0.0, 1.0, 0.0).into(), uv: (1.0, 0.0).into() },
            Vertex { pos: (-0.5, -0.5, 0.0).into(), clr: (0.0, 0.0, 1.0).into(), uv: (0.0, 0.0).into() },
            Vertex { pos: (-0.5,  0.5, 0.0).into(), clr: (1.0, 1.0, 0.0).into(), uv: (0.0, 1.0).into() },
        ];

        let indices = vec![0, 1, 3, 1, 2, 3];

        let ebo = buffer::ElementArrayBuffer::new(&state.gl);
        ebo.bind();
        ebo.static_draw_data(&indices);

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

        ebo.unbind();

        unsafe {
            state.gl.Viewport(0, 0, 900, 700);
            state.gl.ClearColor(0.3, 0.3, 0.5, 1.0);
        }

        Ok(Triangle {
            vao,
            ebo,
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
            state.gl.Clear(gl::COLOR_BUFFER_BIT);
        }

        let time = (Instant::now() - self.start_time).as_secs_f32();

        let trans = glm::Mat4::identity();
        let trans = glm::translate(&trans, &glm::vec3(0.5, -0.5, 0.0));
        let trans = glm::rotate(&trans, time, &glm::vec3(0.0, 0.0, 1.0));

        self.shader_program.set_used();
        self.shader_program.bind_texture("crate", &self.crate_tex);
        self.shader_program.bind_texture("face", &self.face_tex);
        self.shader_program.bind_matrix("transform", trans);
        self.vao.bind();
        self.ebo.bind();
        unsafe {
            state
                .gl
                .DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, ptr::null());
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
