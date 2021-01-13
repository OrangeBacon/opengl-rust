use std::time::Instant;

use crate::{main_loop::EngineState, Layer};
use anyhow::Result;
use sdl2::event::Event;

pub struct ImguiLayer {
    context: imgui::Context,
    imgui_sdl2: imgui_sdl2::ImguiSdl2,
    frame_time: Instant,
    renderer: imgui_opengl_renderer::Renderer,
}

impl Layer for ImguiLayer {
    fn new(state: &EngineState) -> Result<Self> {
        let mut context = imgui::Context::create();
        context.set_ini_filename(None);

        let imgui_sdl2 = imgui_sdl2::ImguiSdl2::new(&mut context, &state.window);
        let renderer = imgui_opengl_renderer::Renderer::new(&mut context, |s| {
            state.video.gl_get_proc_address(s) as _
        });
        let frame_time = Instant::now();

        Ok(ImguiLayer {
            context,
            imgui_sdl2,
            frame_time,
            renderer,
        })
    }

    fn handle_event(&mut self, event: &Event, _state: &EngineState) -> bool {
        self.imgui_sdl2.handle_event(&mut self.context, event);

        false
    }

    fn render(&mut self, state: &EngineState) {
        self.imgui_sdl2
            .prepare_frame(self.context.io_mut(), &state.window, &state.mouse_state);

        let now = Instant::now();
        let delta = now - self.frame_time;
        let delta = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
        self.frame_time = now;

        self.context.io_mut().delta_time = delta;

        let ui = self.context.frame();
        ui.show_demo_window(&mut true);

        self.imgui_sdl2.prepare_render(&ui, &state.window);
        self.renderer.render(ui);
    }
}
