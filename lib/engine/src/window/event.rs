use super::scancode::Scancode;

/// An generic event, from any source
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Event {
    /// The window was resized to have new width and height
    Resize { width: u32, height: u32 },

    /// The window gained user focus
    FocusGained,

    /// The window lost user focus
    FocusLost,

    /// A key was pressed down
    KeyDown { key: Scancode },

    /// A key was released
    KeyUp { key: Scancode },

    /// The window's quit button was pressed
    Quit,

    /// The mouse scroll wheel was moved
    Scroll { x: i32, y: i32 },
}
