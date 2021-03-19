use anyhow::Result;
use thiserror::Error;

use crate::renderer::gl::GlRenderer;

use super::{
    event::{Event, MouseButton, MouseButtonState},
    input::InputState,
    scancode::Scancode,
    window::{Clipboard, SystemCursors, Window, WindowConfig},
};

/// Error type used during initialisation of SDL2 - the default bindings only
/// output `String`, so this type annotates the string with the function that
/// generated the error string and is used to make the string a proper error
/// type, `anyhow::Error`
#[derive(Error, Debug)]
enum SdlError {
    #[error("Error while initialising SDL2: {reason}")]
    Init { reason: String },

    #[error("Error while initialising video subsystem: {reason}")]
    Video { reason: String },

    #[error("Error while initialising OpenGl Context: {reason}")]
    GlContext { reason: String },

    #[error("Error while initialising SLD2 event pump: {reason}")]
    Event { reason: String },

    #[error("Error while creating a cursor: {reason}")]
    Cursor { reason: String },
}

/// Stores sdl state required
pub struct SdlWindow {
    /// The initialised sdl library
    sdl: sdl2::Sdl,

    /// The video subsystem of sdl, used for loading opengl
    video: sdl2::VideoSubsystem,

    /// The current window, todo: multiple window support?
    window: sdl2::video::Window,

    /// All the opengl contexts associated with the current window.  These need
    /// to be kept around for the application to not break when the context is
    /// dropped.  Can also be used for setting which context is current, which
    /// is why multiple contexts are possible.  No idea if it is useful though.
    gl_contexts: Vec<sdl2::video::GLContext>,

    /// The global sdl event pump, is valid for all windows
    event_pump: sdl2::EventPump,

    /// The currently loaded cursor, needed so it isn't dropped
    _cursor: sdl2::mouse::Cursor,
}

impl Window for SdlWindow {
    fn new(config: WindowConfig) -> Result<Self> {
        // initialise graphics library
        let sdl = sdl2::init().map_err(|e| SdlError::Init { reason: e })?;

        // enable graphics output
        let video = sdl.video().map_err(|e| SdlError::Video { reason: e })?;

        // set which OpenGL version is requested (OpenGL core 4.5)
        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(config.gl_version.0, config.gl_version.1);

        if config.debug {
            gl_attr.set_context_flags().debug().set();
        }

        // Configure and create a new window
        let mut window = video.window(config.title, config.width, config.height);

        window.opengl();
        if config.resizable {
            window.resizable();
        }

        let window = window.build()?;

        // Initialise the event pump here, not in the run function so the
        // mouse state can be returned
        let event_pump = sdl
            .event_pump()
            .map_err(|e| SdlError::Event { reason: e })?;

        let cursor = sdl2::mouse::Cursor::from_system(sdl2::mouse::SystemCursor::Arrow)
            .map_err(|e| SdlError::Cursor { reason: e })?;

        Ok(Self {
            sdl,
            video,
            window,
            event_pump,
            _cursor: cursor,
            gl_contexts: Vec::with_capacity(1),
        })
    }

    fn new_gl_context(&mut self) -> Result<gl::Gl> {
        // create a new opengl context
        // this makes the new context current
        let ctx = self
            .window
            .gl_create_context()
            .map_err(|e| SdlError::GlContext { reason: e })?;

        // store it so it isn't dropped
        self.gl_contexts.push(ctx);

        // Tell OpenGL where to find its functions
        Ok(gl::Gl::load_with(|s| self.gl_loader(s)))
    }

    fn gl_loader(&self, name: &'static str) -> *const std::ffi::c_void {
        self.video.gl_get_proc_address(name) as _
    }

    fn set_mouse_capture(&mut self, state: bool) {
        let mouse = self.sdl.mouse();

        // sets mouse capture mode, if not enabled sdl seems to act wierdly
        // when in relative mouse mode
        mouse.capture(state);

        // keeps the mouse in the middle of the window and hides it
        mouse.set_relative_mouse_mode(state);
    }

    fn event(&mut self) -> Option<Event> {
        // iterate through all avaliable events sdl will provide
        while let Some(event) = self.event_pump.poll_event() {
            // if the even can be converted into this engine's event type
            // return the event
            if let Some(event) = event_from_sdl_event(&event) {
                return Some(event);
            }
        }

        // no valid events found
        None
    }

    fn update_mouse(&mut self, state: &mut InputState) {
        // get the current mouse state
        let mouse = self.event_pump.mouse_state();
        let (old_x, old_y) = state.mouse_position();
        let (x, y) = (mouse.x(), mouse.y());

        // update the existing state with the new values
        state.set_mouse_position(x, y);
        state.set_mouse_delta(x - old_x, y - old_y);
        state.set_mouse_left(mouse.left());
        state.set_mouse_middle(mouse.middle());
        state.set_mouse_right(mouse.right());
        state.set_mouse_four(mouse.x1());
        state.set_mouse_five(mouse.x2());

        // The mouse wheel cannot be accessed here, instead wait for the
        // scroll event to update it, so assume zero delta incase the scroll
        // event doesn't come.
        state.set_wheel_delta(0, 0);
    }

    fn swap_window(&mut self) {
        // Required at the end of every frame to swap the double buffer.
        self.window.gl_swap_window();
    }

    fn size(&self) -> (u32, u32) {
        // Get the pixel size of the window. Todo: wtf is dpi
        self.window.size()
    }

    fn clipboard(&self) -> Box<dyn Clipboard> {
        let clip = self.window.subsystem().clipboard();

        Box::new(SdlClipboard(clip))
    }

    fn set_cursor(&mut self, cursor: SystemCursors) {
        use sdl2::mouse::SystemCursor as SdlCursor;

        let mouse = self.window.subsystem().sdl().mouse();

        let cursor = match cursor {
            SystemCursors::Arrow => SdlCursor::Arrow,
            SystemCursors::TextInput => SdlCursor::IBeam,
            SystemCursors::ResizeAll => SdlCursor::SizeAll,
            SystemCursors::ResizeNS => SdlCursor::SizeNS,
            SystemCursors::ResizeEW => SdlCursor::SizeWE,
            SystemCursors::ResizeNESW => SdlCursor::SizeNESW,
            SystemCursors::ResizeNWSE => SdlCursor::SizeNWSE,
            SystemCursors::Hand => SdlCursor::Hand,
            SystemCursors::NotAllowed => SdlCursor::No,
            SystemCursors::NoCursor => {
                mouse.show_cursor(false);
                return;
            }
        };

        if let Ok(sys) = sdl2::mouse::Cursor::from_system(cursor) {
            mouse.show_cursor(true);
            sys.set();
            self._cursor = sys;
        }
    }

    fn renderer(&mut self) -> Result<Box<dyn crate::renderer::backend::RendererBackend>> {
        // create a new opengl context
        // this makes the new context current
        let ctx = self
            .window
            .gl_create_context()
            .map_err(|e| SdlError::GlContext { reason: e })?;

        // store it so it isn't dropped
        self.gl_contexts.push(ctx);

        // Tell OpenGL where to find its functions
        let gl = gl::Gl::load_with(|s| self.video.gl_get_proc_address(s) as _);

        Ok(Box::new(GlRenderer::new(gl)))
    }
}

struct SdlClipboard(sdl2::clipboard::ClipboardUtil);

impl Clipboard for SdlClipboard {
    fn get(&mut self) -> Option<String> {
        if self.0.has_clipboard_text() {
            self.0.clipboard_text().ok()
        } else {
            None
        }
    }

    fn set(&mut self, data: &str) {
        // assume that the user doesn't care if setting the clipboard fails
        let _ = self.0.set_clipboard_text(data);
    }
}

/// Try to convert an sdl event into this engine's event type
fn event_from_sdl_event(event: &sdl2::event::Event) -> Option<Event> {
    use sdl2::event::{Event as SdlEvent, WindowEvent};

    match event {
        // window events are a seperate enum in sdl, but flattened in the engine
        // to make it easier to pattern match against
        SdlEvent::Window { win_event, .. } => match win_event {
            WindowEvent::FocusGained => Some(Event::FocusGained),
            WindowEvent::FocusLost => Some(Event::FocusLost),
            WindowEvent::Resized(width, height) => Some(Event::Resize {
                width: *width as u32,
                height: *height as u32,
            }),
            _ => None,
        },

        SdlEvent::KeyDown {
            scancode: Some(scancode),
            ..
        } => Some(Event::KeyDown {
            key: convert_scancode(*scancode)?,
        }),

        SdlEvent::KeyUp {
            scancode: Some(scancode),
            ..
        } => Some(Event::KeyUp {
            key: convert_scancode(*scancode)?,
        }),

        SdlEvent::MouseButtonDown { mouse_btn, .. } => Some(Event::MouseButton {
            state: MouseButtonState::Pressed,
            button: convert_mouse_button(*mouse_btn)?,
        }),

        SdlEvent::MouseButtonUp { mouse_btn, .. } => Some(Event::MouseButton {
            state: MouseButtonState::Released,
            button: convert_mouse_button(*mouse_btn)?,
        }),

        SdlEvent::TextInput { ref text, .. } => Some(Event::TextInput {
            text: text.to_owned(),
        }),

        SdlEvent::MouseWheel { x, y, .. } => Some(Event::Scroll { x: *x, y: *y }),

        SdlEvent::Quit { .. } => Some(Event::Quit),

        _ => None,
    }
}

fn convert_mouse_button(button: sdl2::mouse::MouseButton) -> Option<MouseButton> {
    use sdl2::mouse::MouseButton as SdlMouse;
    let val = match button {
        SdlMouse::Left => MouseButton::Left,
        SdlMouse::Right => MouseButton::Right,
        SdlMouse::Middle => MouseButton::Middle,
        SdlMouse::X1 => MouseButton::Four,
        SdlMouse::X2 => MouseButton::Five,
        SdlMouse::Unknown => return None,
    };

    Some(val)
}

/// convert between sdl scancodes and the engine's scancode
/// returns an option because the number of scancodes in sdl is a valid scancode
/// which should not be converted to a valid scancode.  Hopefully it wouldn't
/// be passed in either.
fn convert_scancode(scancode: sdl2::keyboard::Scancode) -> Option<Scancode> {
    use sdl2::keyboard::Scancode as SdlCode;
    let code = match scancode {
        SdlCode::A => Scancode::A,
        SdlCode::B => Scancode::B,
        SdlCode::C => Scancode::C,
        SdlCode::D => Scancode::D,
        SdlCode::E => Scancode::E,
        SdlCode::F => Scancode::F,
        SdlCode::G => Scancode::G,
        SdlCode::H => Scancode::H,
        SdlCode::I => Scancode::I,
        SdlCode::J => Scancode::J,
        SdlCode::K => Scancode::K,
        SdlCode::L => Scancode::L,
        SdlCode::M => Scancode::M,
        SdlCode::N => Scancode::N,
        SdlCode::O => Scancode::O,
        SdlCode::P => Scancode::P,
        SdlCode::Q => Scancode::Q,
        SdlCode::R => Scancode::R,
        SdlCode::S => Scancode::S,
        SdlCode::T => Scancode::T,
        SdlCode::U => Scancode::U,
        SdlCode::V => Scancode::V,
        SdlCode::W => Scancode::W,
        SdlCode::X => Scancode::X,
        SdlCode::Y => Scancode::Y,
        SdlCode::Z => Scancode::Z,
        SdlCode::Num1 => Scancode::One,
        SdlCode::Num2 => Scancode::Two,
        SdlCode::Num3 => Scancode::Three,
        SdlCode::Num4 => Scancode::Four,
        SdlCode::Num5 => Scancode::Five,
        SdlCode::Num6 => Scancode::Six,
        SdlCode::Num7 => Scancode::Seven,
        SdlCode::Num8 => Scancode::Eight,
        SdlCode::Num9 => Scancode::Nine,
        SdlCode::Num0 => Scancode::Zero,
        SdlCode::Return => Scancode::Return,
        SdlCode::Escape => Scancode::Escape,
        SdlCode::Backspace => Scancode::Backspace,
        SdlCode::Tab => Scancode::Tab,
        SdlCode::Space => Scancode::Space,
        SdlCode::Minus => Scancode::Dash,
        SdlCode::Equals => Scancode::Equals,
        SdlCode::LeftBracket => Scancode::LeftBracket,
        SdlCode::RightBracket => Scancode::RightBracket,
        SdlCode::Backslash => Scancode::Backslash,
        SdlCode::NonUsHash => Scancode::AltHash,
        SdlCode::Semicolon => Scancode::SemiColon,
        SdlCode::Apostrophe => Scancode::Apostrophe,
        SdlCode::Grave => Scancode::Grave,
        SdlCode::Comma => Scancode::Comma,
        SdlCode::Period => Scancode::Period,
        SdlCode::Slash => Scancode::Slash,
        SdlCode::CapsLock => Scancode::CapsLock,
        SdlCode::F1 => Scancode::F1,
        SdlCode::F2 => Scancode::F2,
        SdlCode::F3 => Scancode::F3,
        SdlCode::F4 => Scancode::F4,
        SdlCode::F5 => Scancode::F5,
        SdlCode::F6 => Scancode::F6,
        SdlCode::F7 => Scancode::F7,
        SdlCode::F8 => Scancode::F8,
        SdlCode::F9 => Scancode::F9,
        SdlCode::F10 => Scancode::F10,
        SdlCode::F11 => Scancode::F11,
        SdlCode::F12 => Scancode::F12,
        SdlCode::PrintScreen => Scancode::PrintScreen,
        SdlCode::ScrollLock => Scancode::ScrollLock,
        SdlCode::Pause => Scancode::Pause,
        SdlCode::Insert => Scancode::Insert,
        SdlCode::Home => Scancode::Home,
        SdlCode::PageUp => Scancode::PageUp,
        SdlCode::Delete => Scancode::Delete,
        SdlCode::End => Scancode::End,
        SdlCode::PageDown => Scancode::PageDown,
        SdlCode::Right => Scancode::Right,
        SdlCode::Left => Scancode::Left,
        SdlCode::Down => Scancode::Down,
        SdlCode::Up => Scancode::Up,
        SdlCode::NumLockClear => Scancode::NumLock,
        SdlCode::KpDivide => Scancode::KpSlash,
        SdlCode::KpMultiply => Scancode::KpStar,
        SdlCode::KpMinus => Scancode::KpDash,
        SdlCode::KpPlus => Scancode::KpPlus,
        SdlCode::KpEnter => Scancode::KpEnter,
        SdlCode::Kp1 => Scancode::KpOne,
        SdlCode::Kp2 => Scancode::KpTwo,
        SdlCode::Kp3 => Scancode::KpThree,
        SdlCode::Kp4 => Scancode::KpFour,
        SdlCode::Kp5 => Scancode::KpFive,
        SdlCode::Kp6 => Scancode::KpSix,
        SdlCode::Kp7 => Scancode::KpSeven,
        SdlCode::Kp8 => Scancode::KpEight,
        SdlCode::Kp9 => Scancode::KpNine,
        SdlCode::Kp0 => Scancode::KpZero,
        SdlCode::KpPeriod => Scancode::KpPeriod,
        SdlCode::NonUsBackslash => Scancode::AltBackslash,
        SdlCode::Application => Scancode::Application,
        SdlCode::Power => Scancode::Power,
        SdlCode::KpEquals => Scancode::KpEquals,
        SdlCode::F13 => Scancode::F13,
        SdlCode::F14 => Scancode::F14,
        SdlCode::F15 => Scancode::F15,
        SdlCode::F16 => Scancode::F16,
        SdlCode::F17 => Scancode::F17,
        SdlCode::F18 => Scancode::F18,
        SdlCode::F19 => Scancode::F19,
        SdlCode::F20 => Scancode::F20,
        SdlCode::F21 => Scancode::F21,
        SdlCode::F22 => Scancode::F22,
        SdlCode::F23 => Scancode::F23,
        SdlCode::F24 => Scancode::F24,
        SdlCode::Execute => Scancode::Execute,
        SdlCode::Help => Scancode::Help,
        SdlCode::Menu => Scancode::Menu,
        SdlCode::Select => Scancode::Select,
        SdlCode::Stop => Scancode::Stop,
        SdlCode::Again => Scancode::Again,
        SdlCode::Undo => Scancode::Undo,
        SdlCode::Cut => Scancode::Cut,
        SdlCode::Copy => Scancode::Copy,
        SdlCode::Paste => Scancode::Paste,
        SdlCode::Find => Scancode::Find,
        SdlCode::Mute => Scancode::Mute,
        SdlCode::VolumeUp => Scancode::VolumeUp,
        SdlCode::VolumeDown => Scancode::VolumeDown,
        SdlCode::KpComma => Scancode::KpComma,
        SdlCode::KpEqualsAS400 => Scancode::KpEqualsAs400,
        SdlCode::International1 => Scancode::InternationalOne,
        SdlCode::International2 => Scancode::InternationalTwo,
        SdlCode::International3 => Scancode::InternationalThree,
        SdlCode::International4 => Scancode::InternationalFour,
        SdlCode::International5 => Scancode::InternationalFive,
        SdlCode::International6 => Scancode::InternationalSix,
        SdlCode::International7 => Scancode::InternationalSeven,
        SdlCode::International8 => Scancode::InternationalEight,
        SdlCode::International9 => Scancode::InternationalNine,
        SdlCode::Lang1 => Scancode::LangOne,
        SdlCode::Lang2 => Scancode::LangTwo,
        SdlCode::Lang3 => Scancode::LangThree,
        SdlCode::Lang4 => Scancode::LangFour,
        SdlCode::Lang5 => Scancode::LangFive,
        SdlCode::Lang6 => Scancode::LangSix,
        SdlCode::Lang7 => Scancode::LangSeven,
        SdlCode::Lang8 => Scancode::LangEight,
        SdlCode::Lang9 => Scancode::LangNine,
        SdlCode::AltErase => Scancode::AltErase,
        SdlCode::SysReq => Scancode::SysReq,
        SdlCode::Cancel => Scancode::Cancel,
        SdlCode::Clear => Scancode::Clear,
        SdlCode::Prior => Scancode::Prior,
        SdlCode::Return2 => Scancode::Return2,
        SdlCode::Separator => Scancode::Separator,
        SdlCode::Out => Scancode::Out,
        SdlCode::Oper => Scancode::Oper,
        SdlCode::ClearAgain => Scancode::ClearAgain,
        SdlCode::CrSel => Scancode::CrSel,
        SdlCode::ExSel => Scancode::ExSel,
        SdlCode::Kp00 => Scancode::KpZeroZero,
        SdlCode::Kp000 => Scancode::KpZeroZeroZero,
        SdlCode::ThousandsSeparator => Scancode::ThousandsSeparator,
        SdlCode::DecimalSeparator => Scancode::DecimalSeparator,
        SdlCode::CurrencyUnit => Scancode::CurrencyUnit,
        SdlCode::CurrencySubUnit => Scancode::CurrencySubUnit,
        SdlCode::KpLeftParen => Scancode::KpLeftParen,
        SdlCode::KpRightParen => Scancode::KpRightParen,
        SdlCode::KpLeftBrace => Scancode::KpLeftBrace,
        SdlCode::KpRightBrace => Scancode::KpRightBrace,
        SdlCode::KpTab => Scancode::KpTab,
        SdlCode::KpBackspace => Scancode::KpBackspace,
        SdlCode::KpA => Scancode::KpA,
        SdlCode::KpB => Scancode::KpB,
        SdlCode::KpC => Scancode::KpC,
        SdlCode::KpD => Scancode::KpD,
        SdlCode::KpE => Scancode::KpE,
        SdlCode::KpF => Scancode::KpF,
        SdlCode::KpXor => Scancode::KpXor,
        SdlCode::KpPower => Scancode::KpPower,
        SdlCode::KpPercent => Scancode::KpPercent,
        SdlCode::KpLess => Scancode::KpLess,
        SdlCode::KpGreater => Scancode::KpGreater,
        SdlCode::KpAmpersand => Scancode::KpAnd,
        SdlCode::KpDblAmpersand => Scancode::KpAndAnd,
        SdlCode::KpVerticalBar => Scancode::KpPipe,
        SdlCode::KpDblVerticalBar => Scancode::KpPipePipe,
        SdlCode::KpColon => Scancode::KpColon,
        SdlCode::KpHash => Scancode::KpHash,
        SdlCode::KpSpace => Scancode::KpSpace,
        SdlCode::KpAt => Scancode::KpAt,
        SdlCode::KpExclam => Scancode::KpExclamation,
        SdlCode::KpMemStore => Scancode::KpMemStore,
        SdlCode::KpMemRecall => Scancode::KpMemRecall,
        SdlCode::KpMemClear => Scancode::KpMemClear,
        SdlCode::KpMemAdd => Scancode::KpMemAdd,
        SdlCode::KpMemSubtract => Scancode::KpMemSubtract,
        SdlCode::KpMemMultiply => Scancode::KpMemMultiply,
        SdlCode::KpMemDivide => Scancode::KpMemDivide,
        SdlCode::KpPlusMinus => Scancode::KpPlusMinus,
        SdlCode::KpClear => Scancode::KpClear,
        SdlCode::KpClearEntry => Scancode::KpClearEntry,
        SdlCode::KpBinary => Scancode::KpBinary,
        SdlCode::KpOctal => Scancode::KpOctal,
        SdlCode::KpDecimal => Scancode::KpDecimal,
        SdlCode::KpHexadecimal => Scancode::KpHex,
        SdlCode::LCtrl => Scancode::LeftControl,
        SdlCode::LShift => Scancode::LeftShift,
        SdlCode::LAlt => Scancode::LeftAlt,
        SdlCode::LGui => Scancode::LeftMeta,
        SdlCode::RCtrl => Scancode::RightControl,
        SdlCode::RShift => Scancode::RightShift,
        SdlCode::RAlt => Scancode::RightAlt,
        SdlCode::RGui => Scancode::RightMeta,
        SdlCode::Mode => Scancode::Mode,
        SdlCode::AudioNext => Scancode::AudioNext,
        SdlCode::AudioPrev => Scancode::AudioPrev,
        SdlCode::AudioStop => Scancode::AudioStop,
        SdlCode::AudioPlay => Scancode::AudioPlay,
        SdlCode::AudioMute => Scancode::AudioMute,
        SdlCode::MediaSelect => Scancode::MediaSelect,
        SdlCode::Www => Scancode::WorldWideWeb,
        SdlCode::Mail => Scancode::Mail,
        SdlCode::Calculator => Scancode::Calculator,
        SdlCode::Computer => Scancode::Computer,
        SdlCode::AcSearch => Scancode::AppSearch,
        SdlCode::AcHome => Scancode::AppHome,
        SdlCode::AcBack => Scancode::AppBack,
        SdlCode::AcForward => Scancode::AppForward,
        SdlCode::AcStop => Scancode::AppStop,
        SdlCode::AcRefresh => Scancode::AppRefresh,
        SdlCode::AcBookmarks => Scancode::AppBookmark,
        SdlCode::BrightnessDown => Scancode::BrightnessDown,
        SdlCode::BrightnessUp => Scancode::BrightnessUp,
        SdlCode::DisplaySwitch => Scancode::DisplaySwitch,
        SdlCode::KbdIllumToggle => Scancode::KeyboardIllumToggle,
        SdlCode::KbdIllumDown => Scancode::KeyboardIllumDown,
        SdlCode::KbdIllumUp => Scancode::KeyboardIllumUp,
        SdlCode::Eject => Scancode::Eject,
        SdlCode::Sleep => Scancode::Sleep,
        SdlCode::App1 => Scancode::App1,
        SdlCode::App2 => Scancode::App2,
        SdlCode::Num => return None,
    };

    Some(code)
}
