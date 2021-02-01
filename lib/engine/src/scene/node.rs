use super::scene::RenderData;

pub trait WorldNode {
    fn new(id: u64) -> Self
    where
        Self: Sized;

    fn update(&mut self, render_state: &mut RenderData);
}
