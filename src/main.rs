use anyhow::Result;
use sdl2;
use gl;

fn main() -> Result<()> {
    let sdl = sdl2::init().unwrap();

    let video = sdl.video().unwrap();
    let window = video.window("Game", 900, 700).opengl().resizable().build().unwrap();

    let ctx = window.gl_create_context().unwrap();
    let gl = gl::load_with(|s| video.gl_get_proc_address(s) as *const std::os::raw::c_void);

    unsafe {
        gl::ClearColor(0.3, 0.3, 0.5, 1.0);
    }

    let mut events = sdl.event_pump().unwrap();
    'main: loop {
        for event in events.poll_iter() {
            match event {
                sdl2::event::Event::Quit {..} => break 'main,
                _ => {},
            }
        }

        unsafe {
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        window.gl_swap_window();
    }

    Ok(())
}
