use anyhow::Result;

pub trait Layer {
    fn new(gl: &gl::Gl) -> Result<Self>
        where Self: std::marker::Sized;
    fn handle_event(&mut self, event: &sdl2::event::Event, gl: &gl::Gl) -> bool;
    fn render(&mut self, gl: &gl::Gl);
}
