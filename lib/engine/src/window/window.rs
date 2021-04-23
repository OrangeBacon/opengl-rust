use anyhow::Result;

use crate::renderer::backend::RendererBackend;

use super::{event::Event, input::InputState};

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

    /// get a function that can be used as an opengl function loader
    fn gl_loader(&self, name: &'static str) -> *const ::std::os::raw::c_void;

    /// try to get a new event from the windows active, if there are no events
    /// that need to be processed, it returns None
    fn event(&mut self) -> Option<Event>;

    /// Allow the window to capture the mouse, e.g. for first person camera support
    fn set_mouse_mode(&mut self, mode: MouseGrabMode);

    /// Take the last frames mouse state and update it with the current frames
    /// mouse state
    fn update_mouse(&mut self, state: &mut InputState);

    /// Sets the current position of the mouse.  May not change the mouse
    /// position in other events read this frame.
    fn set_mouse_position(&mut self, x: u32, y: u32);

    /// Run the required functions at the end of a frame, for example swapping
    /// double buffers
    fn swap_window(&mut self);

    /// Get the current size (x, y) of the window being displayed
    fn size(&self) -> (u32, u32);

    /// Get the scale factor (x, y) to apply to the game, used for hdpi support
    fn scale(&self) -> (f32, f32);

    /// Get a setter and getter for the clipboard
    fn clipboard(&self) -> Box<dyn Clipboard>;

    /// Set the current cursor rendered by the windowing system
    fn set_cursor(&mut self, cursor: SystemCursors);

    /// Get a rendering context
    fn renderer(&mut self) -> Result<Box<dyn RendererBackend>>;
}

/// Wrapper around the system clipboard
pub trait Clipboard {
    /// try to get a string out of the clipboard
    fn get(&mut self) -> Option<String>;

    /// set the string stored by the clipboard
    fn set(&mut self, data: &str);
}

/// The default set of operating system cursors
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum SystemCursors {
    Arrow,
    TextInput,
    ResizeAll,
    ResizeNS,
    ResizeEW,
    ResizeNESW,
    ResizeNWSE,
    Hand,
    NotAllowed,
    NoCursor,
}

/// How the window controls the mouse position
pub enum MouseGrabMode {
    /// It doesn't, the default mode for the operating system
    Standard,

    /// The mouse is constrained to within the window
    Constrained,

    /// The mouse is hidden, mouse motion per frame is used, not mouse position
    Relative,

    /// The mouse is hidden, mouse motion per frame is used, not mouse position.
    /// Also, the hidden mouse is constrained to the current window, prevents
    /// it seeming like hovering over other windows while moving the mouse
    RelativeConstrained,
}
