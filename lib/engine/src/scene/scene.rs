use super::{
    polymap::PolyMap,
    triple_buffer::{TripleBuffer, TripleBufferReader, TripleBufferWriter},
};

#[derive(Default)]
pub struct RenderData {
    data: PolyMap<u64>,
}

impl RenderData {
    pub fn get_mut<T: Send + Default + 'static>(&mut self, id: u64) -> &mut T {
        if self.data.contains_key(&id) {
            self.data.get_mut(&id).unwrap()
        } else {
            self.data.insert(id, T::default());
            self.data.get_mut(&id).unwrap()
        }
    }
}

pub struct Scene {
    render_read: TripleBufferReader<RenderData>,
    render_write: TripleBufferWriter<RenderData>,
}

impl Scene {
    pub fn new() -> Self {
        let buffer = TripleBuffer::new();
        Self {
            render_read: buffer.0,
            render_write: buffer.1,
        }
    }
}
