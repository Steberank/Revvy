//! Superficie wgpu: clear, resize y el renderer de egui encima.

use std::sync::Arc;

use anyhow::Context;
use egui_wgpu::ScreenDescriptor;
use winit::window::Window;

use crate::config::ClientConfig;

mod scene;
#[cfg(target_os = "linux")]
mod vulkan_icd;

pub use scene::{CameraView, ObjectMeshes, Scene, MAX_OBJECTS};

pub struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    clear: wgpu::Color,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    scene: Option<Scene>,
}

impl Gpu {
    pub fn new(window: Arc<Window>, client: &ClientConfig) -> anyhow::Result<Self> {
        #[cfg(target_os = "linux")]
        vulkan_icd::keep_only_working_drivers();

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .context("no se pudo crear la superficie wgpu")?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .context("no hay adaptador de GPU")?;
        let info = adapter.get_info();
        tracing::info!(
            gpu = %info.name,
            backend = ?info.backend,
            driver = %info.driver,
            "adaptador"
        );
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("revvy"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        }))
        .context("no se pudo abrir el device wgpu")?;

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .context("la superficie no es compatible con el adaptador")?;
        let caps = surface.get_capabilities(&adapter);
        if let Some(format) = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ]
        .into_iter()
        .find(|format| caps.formats.contains(format))
        {
            config.format = format;
        }
        surface.configure(&device, &config);

        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            config.format,
            egui_wgpu::RendererOptions::default(),
        );

        let [r, g, b, a] = client.clear_color;
        let scene = Scene::new(
            &device,
            &queue,
            config.format,
            size.width.max(1),
            size.height.max(1),
        );
        Ok(Self {
            surface,
            device,
            queue,
            config,
            clear: wgpu::Color { r, g, b, a },
            egui_ctx,
            egui_state,
            egui_renderer,
            scene: Some(scene),
        })
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn scene_mut(&mut self) -> Option<&mut Scene> {
        self.scene.as_mut()
    }

    pub fn on_window_event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        self.egui_state.on_window_event(window, event).consumed
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        if let Some(scene) = self.scene.as_mut() {
            scene.resize(&self.device, width, height);
        }
    }

    /// Un frame: la escena desde `camera` (o solo el clear), con los autos (`models`) y los
    /// objetos (tipo y matriz), y encima la UI de `ui`.
    pub fn render(
        &mut self,
        window: &Window,
        camera: Option<&CameraView>,
        models: &[glam::Mat4],
        objects: &[(usize, glam::Mat4)],
        ui: impl FnMut(&mut egui::Ui),
    ) -> anyhow::Result<()> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                let size = window.inner_size();
                self.resize(size.width, size.height);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(()),
        };

        let raw_input = self.egui_state.take_egui_input(window);
        let mut full_output = self.egui_ctx.run_ui(raw_input, ui);
        self.egui_state
            .handle_platform_output(window, full_output.platform_output);

        for (id, deltas) in &full_output.textures_delta.set {
            for image in deltas {
                self.egui_renderer
                    .update_texture(&self.device, &self.queue, *id, image);
            }
        }

        let pixels_per_point = window.scale_factor() as f32;
        let screen = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point,
        };
        let primitives = self
            .egui_ctx
            .tessellate(full_output.shapes, pixels_per_point);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("revvy-frame"),
            });
        let user_buffers = self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &primitives,
            &screen,
        );

        {
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let aspect = self.config.width as f32 / self.config.height.max(1) as f32;
            if let (Some(scene), Some(camera)) = (self.scene.as_ref(), camera) {
                scene.draw(
                    &self.queue,
                    &mut encoder,
                    &view,
                    self.clear,
                    aspect,
                    camera,
                    models,
                    objects,
                );
            } else {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("revvy-clear"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(self.clear),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                drop(_pass);
            }
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("revvy-ui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            self.egui_renderer.render(&mut pass, &primitives, &screen);
        }

        self.queue.submit(
            user_buffers
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        self.queue.present(frame);

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        full_output.textures_delta.clear();
        Ok(())
    }
}
