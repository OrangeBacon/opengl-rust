use super::scancode::Scancode;

pub enum Event {
    Resize { width: u32, height: u32 },
    FocusGained,
    FocusLost,
    KeyDown { key: Scancode },
    KeyUp { key: Scancode },
}
