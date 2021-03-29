use std::{marker::PhantomData, time::Instant};

use crate::{
    window::{
        event::Event,
        scancode::Scancode,
        window::{Clipboard, SystemCursors},
    },
    CallOrder, EngineStateRef, EventResult, Layer,
};
use anyhow::Result;

pub struct ImguiLayer<T: Layer> {
    context: imgui::Context,
    frame_time: Instant,
    renderer: imgui_opengl_renderer::Renderer,
    current_cursor: SystemCursors,

    _child: PhantomData<T>,
}

impl<T: Layer + 'static> Layer for ImguiLayer<T> {
    fn new(state: &mut EngineStateRef) -> Result<Self> {
        let base_state = T::new(state)?;
        state.push_state(base_state);

        let mut context = imgui::Context::create();
        context.set_ini_filename(None);
        context.set_clipboard_backend(Box::new(ImguiClipboard(state.window.clipboard())));

        let io = context.io_mut();
        io.key_map[imgui::Key::Tab as usize] = Scancode::Tab as u32;
        io.key_map[imgui::Key::LeftArrow as usize] = Scancode::Left as u32;
        io.key_map[imgui::Key::RightArrow as usize] = Scancode::Right as u32;
        io.key_map[imgui::Key::UpArrow as usize] = Scancode::Up as u32;
        io.key_map[imgui::Key::DownArrow as usize] = Scancode::Down as u32;
        io.key_map[imgui::Key::PageUp as usize] = Scancode::PageUp as u32;
        io.key_map[imgui::Key::PageDown as usize] = Scancode::PageDown as u32;
        io.key_map[imgui::Key::Home as usize] = Scancode::Home as u32;
        io.key_map[imgui::Key::End as usize] = Scancode::End as u32;
        io.key_map[imgui::Key::Delete as usize] = Scancode::Delete as u32;
        io.key_map[imgui::Key::Backspace as usize] = Scancode::Backspace as u32;
        io.key_map[imgui::Key::Enter as usize] = Scancode::Return as u32;
        io.key_map[imgui::Key::Space as usize] = Scancode::Space as u32;
        io.key_map[imgui::Key::A as usize] = Scancode::A as u32;
        io.key_map[imgui::Key::C as usize] = Scancode::C as u32;
        io.key_map[imgui::Key::V as usize] = Scancode::V as u32;
        io.key_map[imgui::Key::X as usize] = Scancode::X as u32;
        io.key_map[imgui::Key::Y as usize] = Scancode::Y as u32;
        io.key_map[imgui::Key::Z as usize] = Scancode::Z as u32;
        io.key_map[imgui::Key::Insert as usize] = Scancode::Insert as u32;
        io.key_map[imgui::Key::KeyPadEnter as usize] = Scancode::KpEnter as u32;

        let renderer =
            imgui_opengl_renderer::Renderer::new(&mut context, |s| state.window.gl_loader(s));
        let frame_time = Instant::now();

        Ok(ImguiLayer {
            context,
            frame_time,
            renderer,
            current_cursor: SystemCursors::Arrow,
            _child: PhantomData::default(),
        })
    }

    fn handle_event(&mut self, state: &mut EngineStateRef, event: &Event) -> EventResult {
        // based upon the keys currently help down, tell imgui about ctrl/shift/etc
        let set_modifiers = |io: &mut imgui::Io| {
            let inp = |key| state.inputs.is_key_pressed(key);
            let ctrl = inp(Scancode::LeftControl) | inp(Scancode::RightControl);
            let alt = inp(Scancode::LeftAlt) | inp(Scancode::RightAlt);
            let shift = inp(Scancode::LeftShift) | inp(Scancode::RightShift);
            let meta = inp(Scancode::LeftMeta) | inp(Scancode::RightMeta);

            io.key_ctrl = ctrl;
            io.key_alt = alt;
            io.key_shift = shift;
            io.key_super = meta;
        };

        let handled = match event {
            Event::Scroll { y, x, .. } => {
                let io = self.context.io_mut();
                io.mouse_wheel = *y as f32;
                io.mouse_wheel_h = *x as f32;
                io.want_capture_mouse
            }
            Event::KeyDown { key } => {
                let io = self.context.io_mut();
                set_modifiers(io);
                io.keys_down[*key as usize] = true;
                io.want_capture_keyboard
            }
            Event::KeyUp { key } => {
                let io = self.context.io_mut();
                set_modifiers(io);
                io.keys_down[*key as usize] = false;
                io.want_capture_keyboard
            }
            Event::TextInput { ref text } => {
                let io = self.context.io_mut();
                for c in text.chars() {
                    io.add_input_character(c);
                }
                io.want_capture_keyboard
            }
            Event::MouseButton { .. } => {
                // mouse state handled in update/render, just check if it is used
                let io = self.context.io();
                io.want_capture_mouse
            }

            Event::Resize { .. } | Event::FocusGained | Event::FocusLost | Event::Quit => false,
        };

        if handled {
            EventResult::Handled
        } else {
            EventResult::Ignored
        }
    }

    // imgui does everything in the render function, no update needed
    fn update(&mut self, _state: &mut EngineStateRef, _dt: f32) {}

    fn render(&mut self, state: &mut EngineStateRef) -> Result<()> {
        let io = self.context.io_mut();

        let (w, h) = state.window.size();
        io.display_size = [w as f32, h as f32];
        // Todo: this makes imgui not dpi aware, fix this
        io.display_framebuffer_scale = [1.0, 1.0];

        io.mouse_down = [
            state.inputs.mouse_left(),
            state.inputs.mouse_right(),
            state.inputs.mouse_middle(),
            state.inputs.mouse_four(),
            state.inputs.mouse_five(),
        ];

        let (x, y) = state.inputs.mouse_position();
        io.mouse_pos = [x as f32, y as f32];

        let now = Instant::now();
        let delta = now - self.frame_time;
        let delta = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
        self.frame_time = now;

        self.context.io_mut().delta_time = delta;

        let ui = self.context.frame();

        ui.show_demo_window(&mut true);

        if !ui
            .io()
            .config_flags
            .contains(imgui::ConfigFlags::NO_MOUSE_CURSOR_CHANGE)
        {
            let cursor = match ui.mouse_cursor() {
                Some(cursor) if !ui.io().mouse_draw_cursor => match cursor {
                    imgui::MouseCursor::Arrow => SystemCursors::Arrow,
                    imgui::MouseCursor::TextInput => SystemCursors::TextInput,
                    imgui::MouseCursor::ResizeAll => SystemCursors::ResizeAll,
                    imgui::MouseCursor::ResizeNS => SystemCursors::ResizeNS,
                    imgui::MouseCursor::ResizeEW => SystemCursors::ResizeEW,
                    imgui::MouseCursor::ResizeNESW => SystemCursors::ResizeNESW,
                    imgui::MouseCursor::ResizeNWSE => SystemCursors::ResizeNWSE,
                    imgui::MouseCursor::Hand => SystemCursors::Hand,
                    imgui::MouseCursor::NotAllowed => SystemCursors::NotAllowed,
                },
                _ => SystemCursors::NoCursor,
            };

            if self.current_cursor != cursor {
                state.window.set_cursor(cursor);
                self.current_cursor = cursor;
            }
        }

        self.renderer.render(ui);

        Ok(())
    }

    /// in order to render ontop of the game, it needs defered rendering
    fn render_order(&self) -> CallOrder {
        CallOrder::Deferred
    }

    /// in order to read the other states it needs defered updates
    fn update_order(&self) -> CallOrder {
        CallOrder::Deferred
    }
}

struct ImguiClipboard(Box<dyn Clipboard>);

impl imgui::ClipboardBackend for ImguiClipboard {
    fn get(&mut self) -> Option<imgui::ImString> {
        self.0.get().map(imgui::ImString::new)
    }

    fn set(&mut self, value: &imgui::ImStr) {
        self.0.set(value.to_str());
    }
}
