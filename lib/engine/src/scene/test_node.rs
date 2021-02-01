use super::{node::WorldNode, scene::RenderData};

pub struct MyNode {
    id: u64,
    data: u32,
    children: Vec<Box<dyn WorldNode>>,
}

#[derive(Default)]
pub struct MyNodeRender {
    data: u32,
}

impl WorldNode for MyNode {
    fn new(id: u64) -> Self {
        Self {
            id,
            data: 0,
            children: vec![],
        }
    }

    fn update(&mut self, render_data: &mut RenderData) {
        let render_data: &mut MyNodeRender = render_data.get_mut(self.id);

        self.data += 1;
        render_data.data = self.data;
    }
}

impl MyNodeRender {
    fn render(&self) {
        print!("{}", self.data);
    }
}
