use anyhow::Result;

use super::{event::Event, input::MouseState};

/// The startup settings to configure a new window with
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct WindowConfig<'a> {
    /// The initial width of the window.  Should not be relied upon to be
    /// correct, use `window.size()` to get the current width
    pub width: u32,

    /// The initial height of the window.  This has the same limitations as
    /// for the window width
    pub height: u32,

    /// The text to show in the window's title bar
    pub title: &'a str,

    /// Should the user be able to resize the window
    pub resizable: bool,

    /// Should the OpenGL contexts be created with debug mode?  This will try
    /// to enable debug mode, but should not be relied upon to have worked.
    /// Check the gl context flags before using debug mode.
    pub debug: bool,

    /// The requested version of OpenGL to use (major, minor)
    pub gl_version: (u8, u8),
}

/// The implementation of a windowing system, should probably also handle multi-
/// window support in the future, so is supposed to be a single global instance
/// rather than one per window.
pub trait Window {
    /// initialise the graphics library
    fn new(config: WindowConfig) -> Result<Self>
    where
        Self: Sized;

    /// try to create a new opengl context
    fn new_gl_context(&mut self) -> Result<gl::Gl>;

    /// try to get a new event from the windows active, if there are no events
    /// that need to be processed, it returns None
    fn event(&mut self) -> Option<Event>;

    /// Allow the window to capture the mouse, e.g. for first person camera support
    fn set_mouse_capture(&mut self, state: bool);

    /// Get the current state of the mouse
    fn update_mouse(&mut self) -> MouseState;

    /// Run the required functions at the end of a frame, for example swapping
    /// double buffers
    fn swap_window(&mut self);

    /// Get the current size of the window being displayed
    fn size(&self) -> (u32, u32);
}
