use std::time::Instant;

use crate::{
    main_loop::EngineState,
    window::{event::Event, scancode::Scancode, window::Clipboard},
    EventResult, Layer,
};
use anyhow::Result;

pub struct ImguiLayer {
    context: imgui::Context,
    frame_time: Instant,
    renderer: imgui_opengl_renderer::Renderer,
}

impl Layer for ImguiLayer {
    fn new(state: &EngineState) -> Result<Self> {
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
        })
    }

    fn handle_event(&mut self, _state: &mut EngineState, event: &Event) -> EventResult {
        match event {
            Event::Scroll { y, x, .. } => {
                let io = self.context.io_mut();
                io.mouse_wheel = *y as f32;
                io.mouse_wheel_h = *x as f32;
                EventResult::Handled
            }
            Event::KeyDown { key } => {
                let io = self.context.io_mut();
                io.keys_down[*key as usize] = true;
                if io.want_capture_keyboard {
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }
            Event::KeyUp { key } => {
                let io = self.context.io_mut();
                io.keys_down[*key as usize] = false;
                if io.want_capture_keyboard {
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }
            Event::TextInput { ref text } => {
                let io = self.context.io_mut();
                for c in text.chars() {
                    io.add_input_character(c);
                }
                if io.want_capture_keyboard {
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }
            Event::MouseButton { .. } => {
                // mouse state handled in update/render, just check if it is used
                let io = self.context.io();

                if io.want_capture_mouse {
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }

            Event::Resize { .. } | Event::FocusGained | Event::FocusLost | Event::Quit => {
                EventResult::Ignored
            }
        }
    }

    // imgui does everything in the render function, no update needed
    fn update(&mut self, _state: &EngineState, _dt: f32) {}

    fn render(&mut self, _state: &EngineState) {
        let now = Instant::now();
        let delta = now - self.frame_time;
        let delta = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
        self.frame_time = now;

        self.context.io_mut().delta_time = delta;

        let ui = self.context.frame();
        ui.show_demo_window(&mut true);

        self.renderer.render(ui);
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
