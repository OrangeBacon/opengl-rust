use anyhow::Result;
use engine::{
    buffer, data, gl, glm, resources::Resources, sdl2, EngineState, Layer, MainLoop, Program,
    Texture,
};
use gl_derive::VertexAttribPointers;
use std::{collections::HashSet, path::Path, time::Instant};

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
    last_frame: Instant,
    pos: glm::Vec3,
    front: glm::Vec3,
    up: glm::Vec3,
    key_state: HashSet<sdl2::keyboard::Keycode>,
    previous_mouse: sdl2::mouse::MouseState,
    yaw: f32,
    pitch: f32,
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

        let (width, height) = state.window.size();

        unsafe {
            state.gl.Viewport(0, 0, width as i32, height as i32);
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
            last_frame: Instant::now(),
            pos: glm::vec3(0.0, 0.0, 3.0),
            front: glm::vec3(0.0, 0.0, -1.0),
            up: glm::vec3(0.0, 1.0, 0.0),
            key_state: HashSet::new(),
            previous_mouse: state.mouse_state,
            yaw: -90.0,
            pitch: 0.0,
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
            sdl2::event::Event::KeyDown {
                keycode: Some(keycode),
                ..
            } => {
                if *keycode == sdl2::keyboard::Keycode::Escape {
                    return true;
                }
                self.key_state.insert(*keycode);
            }
            sdl2::event::Event::KeyUp {
                keycode: Some(keycode),
                ..
            } => {
                self.key_state.remove(keycode);
            }
            _ => (),
        }

        false
    }

    fn render(&mut self, state: &EngineState) {
        unsafe {
            state.gl.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }

        let start_time = Instant::now();

        let total_time = (start_time - self.start_time).as_secs_f32();
        let (width, height) = state.window.size();

        let projection = glm::perspective(
            width as f32 / height as f32,
            45.0f32.to_radians(),
            0.1,
            100.0,
        );

        let camera_speed = 2.5 * (start_time - self.last_frame).as_secs_f32();
        if self.key_state.contains(&sdl2::keyboard::Keycode::W) {
            self.pos += camera_speed * self.front;
        }
        if self.key_state.contains(&sdl2::keyboard::Keycode::S) {
            self.pos -= camera_speed * self.front;
        }
        if self.key_state.contains(&sdl2::keyboard::Keycode::A) {
            self.pos -= glm::normalize(&glm::cross(&self.front, &self.up)) * camera_speed;
        }
        if self.key_state.contains(&sdl2::keyboard::Keycode::D) {
            self.pos += glm::normalize(&glm::cross(&self.front, &self.up)) * camera_speed;
        }

        let sensitivity = 0.1;
        let x_offset = state.mouse_state.x() - self.previous_mouse.x();
        let y_offset = state.mouse_state.y() - self.previous_mouse.y();
        let x_offset = x_offset as f32 * sensitivity;
        let y_offset = y_offset as f32 * sensitivity;

        self.yaw += x_offset;
        self.pitch += y_offset;

        if self.pitch > 89.0 {
            self.pitch = 89.0
        }
        if self.pitch < -89.0 {
            self.pitch = -89.0
        }

        let yaw = self.yaw.to_radians();
        let pitch = self.pitch.to_radians();

        let direction = glm::vec3(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        );
        self.front = glm::normalize(&direction);

        let view = glm::look_at(&self.pos, &(self.pos + self.front), &self.up);

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
            let angle = 20.0 * (i as f32 + total_time);

            let model = glm::Mat4::identity();
            let model = glm::translate(&model, pos);
            let model = glm::rotate(&model, angle.to_radians(), &glm::vec3(1.0, 0.3, 0.5));

            self.shader_program.bind_matrix("model", model);
            unsafe {
                state.gl.DrawArrays(gl::TRIANGLES, 0, 36);
            }
        }

        self.last_frame = Instant::now();
        self.previous_mouse = state.mouse_state;
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
