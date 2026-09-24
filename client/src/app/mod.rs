//! Loop de ventana: winit, un frame de ECS y un redraw de wgpu.

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::config::ClientConfig;
use crate::drive::DriveView;
use crate::input::Input;
use crate::render::Gpu;

pub fn run(config: ClientConfig) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = App {
        config,
        window: None,
        gpu: None,
        input: Input::new(),
        map: None,
        last_tick: Instant::now(),
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    config: ClientConfig,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    input: Input,
    map: Option<DriveView>,
    last_tick: Instant,
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
            Ok(mut gpu) => {
                tracing::info!("superficie wgpu lista");
                match DriveView::load(&self.config) {
                    Ok(map) => {
                        let device = gpu.device().clone();
                        let queue = gpu.queue().clone();
                        if let Some(scene) = gpu.scene_mut() {
                            scene.upload_track(
                                &device,
                                &queue,
                                map.track_meshes(),
                                map.track_textures(),
                                map.color_key(),
                            );
                            if let Some(sky) = map.sky() {
                                scene.upload_sky(&device, &queue, sky);
                            }
                            let cars: Vec<_> = map
                                .cars()
                                .iter()
                                .map(|car| (car.parts.as_slice(), car.texture.as_ref()))
                                .collect();
                            scene.upload_cars(&device, &queue, &cars);
                        }
                        self.map = Some(map);
                        self.gpu = Some(gpu);
                        self.window = Some(window);
                        self.last_tick = Instant::now();
                    }
                    Err(err) => {
                        tracing::error!(%err, "no se pudo cargar la pista o el auto");
                        event_loop.exit();
                    }
                }
            }
            Err(err) => {
                tracing::error!(%err, "no se pudo inicializar wgpu");
                event_loop.exit();
            }
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.input.device_event(&event);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = self.window.clone() else {
            return;
        };
        self.input.window_event(&event);
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
                let now = Instant::now();
                let dt = now.saturating_duration_since(self.last_tick).as_secs_f32();
                self.last_tick = now;
                if let Some(map) = self.map.as_mut() {
                    map.step(dt, &mut self.input);
                }
                self.input.end_frame();
                let camera = self.map.as_ref().map(|map| map.camera());
                let models = self.map.as_ref().map(|map| map.car_models()).unwrap_or_default();
                let hud = self.map.as_ref().map(|map| map.hud()).unwrap_or_default();
                if let Some(gpu) = self.gpu.as_mut() {
                    if let Err(err) = gpu.render(&window, camera.as_ref(), &models, &hud) {
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
