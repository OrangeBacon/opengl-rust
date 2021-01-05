use gl::types::*;
use image::io::Reader as ImageReader;
use sdl2;
use std::ffi::CString;

mod gl_backend;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdl = sdl2::init().unwrap();

    let video = sdl.video().unwrap();

    let gl_attr = video.gl_attr();
    gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
    gl_attr.set_context_version(4, 1);

    let window = video
        .window("Game", 900, 700)
        .opengl()
        .resizable()
        .build()
        .unwrap();

    let _ctx = window.gl_create_context().unwrap();
    let _gl = gl::load_with(|s| video.gl_get_proc_address(s) as *const std::os::raw::c_void);

    let vert_shader = gl_backend::Shader::from_vert(
        &CString::new(include_str!("../asset/triangle.vert")).unwrap(),
    )
    .unwrap();

    let frag_shader = gl_backend::Shader::from_frag(
        &CString::new(include_str!("../asset/triangle.frag")).unwrap(),
    )
    .unwrap();

    let shader_program = gl_backend::Program::from_shaders(&[vert_shader, frag_shader]).unwrap();

    #[rustfmt::skip]
    let vertices: Vec<f32> = vec![
        // positions       colors           uv coords
         0.5,  0.5, 0.0,   1.0, 0.0, 0.0,   1.0, 1.0,
         0.5, -0.5, 0.0,   0.0, 1.0, 0.0,   1.0, 0.0,
        -0.5, -0.5, 0.0,   0.0, 0.0, 1.0,   0.0, 0.0,
        -0.5,  0.5, 0.0,   1.0, 1.0, 0.0,   0.0, 1.0,
    ];
    let indicies: Vec<GLuint> = vec![0, 1, 3, 1, 2, 3];

    let mut crate_tex = ImageReader::open("asset/container.jpg")?
        .decode()?
        .into_rgb8();
    image::imageops::flip_vertical_in_place(&mut crate_tex);

    let mut face_tex = ImageReader::open("asset/awesomeface.png")?
        .decode()?
        .into_rgb8();
    image::imageops::flip_vertical_in_place(&mut face_tex);

    let mut crate_idx: GLuint = 0;
    unsafe {
        gl::GenTextures(1, &mut crate_idx);
        gl::BindTexture(gl::TEXTURE_2D, crate_idx);

        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::REPEAT as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::REPEAT as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);

        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGB as GLint,
            crate_tex.width() as GLint,
            crate_tex.height() as GLint,
            0,
            gl::RGB,
            gl::UNSIGNED_BYTE,
            crate_tex.as_raw().as_ptr() as *const GLvoid,
        );
        gl::GenerateMipmap(gl::TEXTURE_2D);
    }

    let mut face_idx: GLuint = 0;
    unsafe {
        gl::GenTextures(1, &mut face_idx);
        gl::BindTexture(gl::TEXTURE_2D, face_idx);

        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::REPEAT as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::REPEAT as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);

        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGB as GLint,
            face_tex.width() as GLint,
            face_tex.height() as GLint,
            0,
            gl::RGB,
            gl::UNSIGNED_BYTE,
            face_tex.as_raw().as_ptr() as *const GLvoid,
        );
        gl::GenerateMipmap(gl::TEXTURE_2D);
    }

    let mut vbo: GLuint = 0;
    let mut vao: GLuint = 0;
    let mut ebo: GLuint = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);
        gl::GenBuffers(1, &mut ebo);

        gl::BindVertexArray(vao);

        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (vertices.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
            vertices.as_ptr() as *const GLvoid,
            gl::STATIC_DRAW,
        );

        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
        gl::BufferData(
            gl::ELEMENT_ARRAY_BUFFER,
            (indicies.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
            indicies.as_ptr() as *const GLvoid,
            gl::STATIC_DRAW,
        );

        gl::EnableVertexAttribArray(0);
        gl::VertexAttribPointer(
            0,
            3,
            gl::FLOAT,
            gl::FALSE,
            (8 * std::mem::size_of::<f32>()) as GLint,
            std::ptr::null(),
        );

        gl::EnableVertexAttribArray(1);
        gl::VertexAttribPointer(
            1,
            3,
            gl::FLOAT,
            gl::FALSE,
            (8 * std::mem::size_of::<f32>()) as GLint,
            (3 * std::mem::size_of::<f32>()) as *const GLvoid,
        );

        gl::EnableVertexAttribArray(2);
        gl::VertexAttribPointer(
            2,
            2,
            gl::FLOAT,
            gl::FALSE,
            (8 * std::mem::size_of::<f32>()) as GLint,
            (6 * std::mem::size_of::<f32>()) as *const GLvoid,
        );

        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindVertexArray(0);
    }

    unsafe {
        gl::Viewport(0, 0, 900, 700);
        gl::ClearColor(0.3, 0.3, 0.5, 1.0);
    }

    shader_program.set_used();

    unsafe {
        gl::Uniform1i(gl::GetUniformLocation(shader_program.id(), b"Crate".as_ptr() as *const i8), 0);
        gl::Uniform1i(gl::GetUniformLocation(shader_program.id(), b"Smiley".as_ptr() as *const i8), 1)
    }

    let mut events = sdl.event_pump().unwrap();
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

            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, crate_idx);
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, face_idx);
            gl::BindVertexArray(vao);
            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
        }

        window.gl_swap_window();
    }

    Ok(())
}
