use sdl2;
pub mod render_gl;
pub mod resources;
use anyhow::Result;
use gl::types::*;
use resources::Resources;
use std::{mem::size_of, path::Path, ptr};
use thiserror::Error;

#[derive(Error, Debug)]
enum SdlError {
    #[error("Error while initialising SDL2: {reason}")]
    InitError { reason: String },

    #[error("Error while initialising video subsystem: {reason}")]
    VideoError { reason: String },

    #[error("Error while initialising OpenGl Context: {reason}")]
    GlContextError { reason: String },

    #[error("Error while intialising SLD2 event pump: {reason}")]
    EventError { reason: String },
}

fn main() {
    if let Err(e) = run() {
        println!("{}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let res = Resources::from_exe_path(Path::new("assets"))?;

    let sdl = sdl2::init().map_err(|e| SdlError::InitError { reason: e })?;

    let video = sdl
        .video()
        .map_err(|e| SdlError::VideoError { reason: e })?;

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
        .map_err(|e| SdlError::GlContextError { reason: e })?;
    let gl = gl::Gl::load_with(|s| video.gl_get_proc_address(s) as *const _);

    let shader_program = render_gl::Program::from_res(&gl, &res, "shaders/triangle")?;

    #[rustfmt::skip]
    let verticies: Vec<f32> = vec![
        // positions      // colors
        0.5, -0.5, 0.0,   1.0, 0.0, 0.0,   // bottom right
        -0.5, -0.5, 0.0,  0.0, 1.0, 0.0,   // bottom left
        0.0,  0.5, 0.0,   0.0, 0.0, 1.0    // top
    ];

    let mut vbo = 0;
    unsafe {
        gl.GenBuffers(1, &mut vbo);
        gl.BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl.BufferData(
            gl::ARRAY_BUFFER,
            (verticies.len() * size_of::<f32>()) as GLsizeiptr,
            verticies.as_ptr() as *const GLvoid,
            gl::STATIC_DRAW,
        );
        gl.BindBuffer(gl::ARRAY_BUFFER, 0);
    }

    let mut vao = 0;
    unsafe {
        gl.GenVertexArrays(1, &mut vao);
        gl.BindVertexArray(vao);
        gl.BindBuffer(gl::ARRAY_BUFFER, vbo);

        gl.EnableVertexAttribArray(0);
        gl.VertexAttribPointer(
            0,
            3,
            gl::FLOAT,
            gl::FALSE,
            (6 * size_of::<f32>()) as GLint,
            ptr::null(),
        );

        gl.EnableVertexAttribArray(1);
        gl.VertexAttribPointer(
            1,
            3,
            gl::FLOAT,
            gl::FALSE,
            (6 * size_of::<f32>()) as GLint,
            (3 * size_of::<f32>()) as *const GLvoid,
        );

        gl.BindBuffer(gl::ARRAY_BUFFER, 0);
        gl.BindVertexArray(0);
    }

    unsafe {
        gl.Viewport(0, 0, 900, 700);
        gl.ClearColor(0.3, 0.3, 0.5, 1.0);
    }

    let mut events = sdl
        .event_pump()
        .map_err(|e| SdlError::EventError { reason: e })?;
    'main: loop {
        for event in events.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. } => break 'main,
                sdl2::event::Event::Window { win_event, .. } => {
                    if let sdl2::event::WindowEvent::Resized(w, h) = win_event {
                        unsafe {
                            gl.Viewport(0, 0, w, h);
                        }
                    }
                }
                _ => {}
            }
        }

        unsafe {
            gl.Clear(gl::COLOR_BUFFER_BIT);
        }

        shader_program.set_used();
        unsafe {
            gl.BindVertexArray(vao);
            gl.DrawArrays(gl::TRIANGLES, 0, 3);
        }

        window.gl_swap_window();
    }

    Ok(())
}
