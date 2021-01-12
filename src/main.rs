use anyhow::Result;
use gl_derive::VertexAttribPointers;
use engine::{resources::Resources, buffer, Program, data};
use std::path::Path;
use thiserror::Error;

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

#[derive(VertexAttribPointers, Copy, Clone, Debug)]
#[repr(C, packed)]
struct Vertex {
    #[location = 0]
    pos: data::f32_f32_f32,

    #[location = 1]
    clr: data::f32_f32_f32,
}

fn main() {
    if let Err(e) = run() {
        println!("{}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let res = Resources::from_exe_path(Path::new("assets"))?;

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

    let _ctx = window
        .gl_create_context()
        .map_err(|e| SdlError::GlContext { reason: e })?;
    let _gl = gl::load_with(|s| video.gl_get_proc_address(s) as *const _);

    let shader_program = Program::from_res(&res, "shaders/triangle")?;

    #[rustfmt::skip]
    let verticies: Vec<Vertex> = vec![
        // positions                            // colors
        Vertex { pos: ( 0.5, -0.5, 0.0).into(), clr: (1.0, 0.0, 0.0).into() },   // bottom right
        Vertex { pos: (-0.5, -0.5, 0.0).into(), clr: (0.0, 1.0, 0.0).into() },   // bottom left
        Vertex { pos: ( 0.0,  0.5, 0.0).into(), clr: (0.0, 0.0, 1.0).into() },   // top
    ];

    let vbo = buffer::ArrayBuffer::new();
    vbo.bind();
    vbo.static_draw_data(&verticies);
    vbo.unbind();

    let vao = buffer::VertexArray::new();
    vao.bind();
    vbo.bind();
    Vertex::attrib_pointers();
    vbo.unbind();
    vao.unbind();

    unsafe {
        gl::Viewport(0, 0, 900, 700);
        gl::ClearColor(0.3, 0.3, 0.5, 1.0);
    }

    let mut events = sdl
        .event_pump()
        .map_err(|e| SdlError::Event { reason: e })?;
    'main: loop {
        for event in events.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. } => break 'main,
                sdl2::event::Event::Window { win_event, .. } => {
                    if let sdl2::event::WindowEvent::Resized(w, h) = win_event {
                        unsafe {
                            gl::Viewport(0, 0, w, h);
                        }
                    }
                }
                _ => {}
            }
        }

        unsafe {
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        shader_program.set_used();
        vao.bind();
        unsafe {
            gl::DrawArrays(gl::TRIANGLES, 0, 3);
        }

        window.gl_swap_window();
    }

    Ok(())
}
