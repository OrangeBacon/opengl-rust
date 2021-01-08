use sdl2;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdl = sdl2::init()?;

    let video = sdl.video()?;

    let gl_attr = video.gl_attr();
    gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
    gl_attr.set_context_version(4, 1);

    let window = video
        .window("Game", 900, 700)
        .opengl()
        .resizable()
        .build()?;

    let _ctx = window.gl_create_context()?;
    let _gl = gl::load_with(|s| video.gl_get_proc_address(s) as *const _);

    let mut events = sdl.event_pump()?;
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
            gl::ClearColor(0.2, 0.3, 0.3, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        window.gl_swap_window();
    }

    Ok(())
}
