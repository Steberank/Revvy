//! Loop de ventana: winit, un frame de ECS y un redraw de wgpu.

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::config::ClientConfig;
use crate::ecs::FrameLoop;
use crate::input::Gamepads;
use crate::render::Gpu;

pub fn run(config: ClientConfig) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = App {
        config,
        window: None,
        gpu: None,
        gamepads: Gamepads::new(),
        frames: FrameLoop::new(),
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    config: ClientConfig,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    gamepads: Gamepads,
    frames: FrameLoop,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(&self.config.window_title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.window_width,
                self.config.window_height,
            ));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                tracing::error!(%err, "no se pudo crear la ventana");
                event_loop.exit();
                return;
            }
        };
        match Gpu::new(window.clone(), &self.config) {
            Ok(gpu) => {
                tracing::info!("superficie wgpu lista");
                self.gpu = Some(gpu);
                self.window = Some(window);
            }
            Err(err) => {
                tracing::error!(%err, "no se pudo inicializar wgpu");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if let Some(gpu) = self.gpu.as_mut() {
            let _ = gpu.on_window_event(&window, &event);
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                self.gamepads.poll();
                self.frames.tick();
                tracing::trace!(frame = self.frames.index(), "tick");
                if let Some(gpu) = self.gpu.as_mut() {
                    if let Err(err) = gpu.render(&window) {
                        tracing::error!(%err, "falló el frame");
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(16),
        ));
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
