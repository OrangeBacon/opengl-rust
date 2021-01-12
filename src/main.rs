use anyhow::Result;
use engine::{buffer, data, gl, resources::Resources, sdl2, Layer, MainLoop, Program};
use gl_derive::VertexAttribPointers;
use std::path::Path;

#[derive(VertexAttribPointers, Copy, Clone, Debug)]
#[repr(C, packed)]
struct Vertex {
    #[location = 0]
    pos: data::f32_f32_f32,

    #[location = 1]
    clr: data::f32_f32_f32,
}

struct Triangle {
    _vbo: buffer::ArrayBuffer,
    vao: buffer::VertexArray,
    shader_program: Program,
}

impl Layer for Triangle {
    fn new(gl: &gl::Gl) -> Result<Self> {
        let res = Resources::from_exe_path(Path::new("assets"))?;
        let shader_program = Program::from_res(&gl, &res, "shaders/triangle")?;

        #[rustfmt::skip]
        let verticies: Vec<Vertex> = vec![
            // positions                            // colors
            Vertex { pos: ( 0.5, -0.5, 0.0).into(), clr: (1.0, 0.0, 0.0).into() },   // bottom right
            Vertex { pos: (-0.5, -0.5, 0.0).into(), clr: (0.0, 1.0, 0.0).into() },   // bottom left
            Vertex { pos: ( 0.0,  0.5, 0.0).into(), clr: (0.0, 0.0, 1.0).into() },   // top
        ];

        let vbo = buffer::ArrayBuffer::new(&gl);
        vbo.bind();
        vbo.static_draw_data(&verticies);
        vbo.unbind();

        let vao = buffer::VertexArray::new(&gl);
        vao.bind();
        vbo.bind();
        Vertex::attrib_pointers(&gl);
        vbo.unbind();
        vao.unbind();

        unsafe {
            gl.Viewport(0, 0, 900, 700);
            gl.ClearColor(0.3, 0.3, 0.5, 1.0);
        }

        Ok(Triangle {
            vao,
            _vbo: vbo,
            shader_program,
        })
    }

    fn handle_event(&mut self, event: &sdl2::event::Event, gl: &gl::Gl) -> bool {
        match event {
            sdl2::event::Event::Quit { .. } => return true,
            sdl2::event::Event::Window { win_event, .. } => {
                if let sdl2::event::WindowEvent::Resized(w, h) = win_event {
                    unsafe {
                        gl.Viewport(0, 0, *w, *h);
                    }
                }
            }
            _ => (),
        }

        false
    }
    fn render(&mut self, gl: &gl::Gl) {
        unsafe {
            gl.Clear(gl::COLOR_BUFFER_BIT);
        }

        self.shader_program.set_used();
        self.vao.bind();
        unsafe {
            gl.DrawArrays(gl::TRIANGLES, 0, 3);
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

    main_loop.run()?;

    Ok(())
}
