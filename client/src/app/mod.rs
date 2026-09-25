//! Loop de ventana: winit, el menú o la carrera, y un redraw de wgpu.
//!
//! Sin argumentos arranca en el menú. Con `cargo run -p revvy-client -- <pista> [autos…]`
//! arranca directo en la carrera. Esc en la carrera vuelve a la sala del menú.

use std::sync::Arc;
use std::time::{Duration, Instant};

use revvy_formats::VisualMesh;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::KeyCode;
use winit::window::{Window, WindowId};

use crate::config::ClientConfig;
use crate::drive::{cli_race, DriveView, Race};
use crate::input::Input;
use crate::menu::MenuView;
use crate::render::{Gpu, ObjectMeshes};
use crate::ui::menu::MenuAction;

pub fn run(config: ClientConfig) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = App {
        config,
        window: None,
        gpu: None,
        input: Input::new(),
        menu: None,
        race: None,
        pending: None,
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
    /// Queda cargado durante la carrera para volver rápido.
    menu: Option<MenuView>,
    /// La carrera en curso. Sin carrera se ve el menú.
    race: Option<DriveView>,
    /// Iniciar Carrera: carga después de que un frame mostró "Cargando pista…". El `bool`
    /// dice si ese frame ya se dibujó.
    pending: Option<(Race, bool)>,
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
            Ok(gpu) => {
                tracing::info!("superficie wgpu lista");
                self.gpu = Some(gpu);
                self.window = Some(window);
            }
            Err(err) => {
                tracing::error!(%err, "no se pudo inicializar wgpu");
                event_loop.exit();
                return;
            }
        }
        let loaded = match cli_race(&self.config) {
            Some(race) => self.start_race(&race),
            None => self.open_menu(),
        };
        if let Err(err) = loaded {
            tracing::error!(%err, "no se pudo cargar la pista o el auto");
            event_loop.exit();
        }
        self.last_tick = Instant::now();
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
                self.load_pending_race();
                let now = Instant::now();
                let dt = now.saturating_duration_since(self.last_tick).as_secs_f32();
                self.last_tick = now;
                if self.race.is_some() {
                    self.redraw_race(event_loop, &window, dt);
                } else {
                    self.redraw_menu(event_loop, &window, dt);
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

impl App {
    /// Carga la carrera y la pone en la escena.
    fn start_race(&mut self, race: &Race) -> anyhow::Result<()> {
        let drive = DriveView::load(&self.config, race)?;
        if let Some(gpu) = self.gpu.as_mut() {
            let cars: Vec<_> = drive
                .cars()
                .iter()
                .map(|car| (car.parts.as_slice(), car.texture.as_ref()))
                .collect();
            let track = TrackScene {
                meshes: drive.track_meshes(),
                textures: drive.track_textures(),
                color_key: drive.color_key(),
                sky: drive.sky(),
                background: drive.background(),
            };
            let objects: Vec<_> = drive
                .object_kinds()
                .iter()
                .map(|kind| (kind.meshes.as_slice(), kind.textures.as_slice()))
                .collect();
            upload_scene(gpu, &track, &cars, &objects);
        }
        self.race = Some(drive);
        Ok(())
    }

    /// Deja la carrera (si había) y pone la pista del menú en la escena. El menú se carga
    /// la primera vez.
    fn open_menu(&mut self) -> anyhow::Result<()> {
        self.race = None;
        if self.menu.is_none() {
            self.menu = Some(MenuView::load(&self.config)?);
        }
        if let (Some(menu), Some(gpu)) = (self.menu.as_ref(), self.gpu.as_mut()) {
            let track = TrackScene {
                meshes: menu.track_meshes(),
                textures: menu.track_textures(),
                color_key: menu.color_key(),
                sky: menu.sky(),
                background: menu.background(),
            };
            upload_scene(gpu, &track, &[], &[]);
        }
        Ok(())
    }

    /// La carrera pedida en el menú, una vez que se vio "Cargando pista…".
    fn load_pending_race(&mut self) {
        let Some((race, shown)) = self.pending.take() else {
            return;
        };
        if !shown {
            self.pending = Some((race, true));
            return;
        }
        if let Err(err) = self.start_race(&race) {
            tracing::error!(%err, pista = %race.level, "no se pudo cargar la carrera");
            if let Some(menu) = self.menu.as_mut() {
                menu.state.cancel_loading();
            }
        }
        // La carga no cuenta como tiempo de juego.
        self.last_tick = Instant::now();
    }

    fn redraw_race(&mut self, event_loop: &ActiveEventLoop, window: &Window, dt: f32) {
        let (Some(race), Some(gpu)) = (self.race.as_mut(), self.gpu.as_mut()) else {
            return;
        };
        let to_menu = self.input.take_pressed(KeyCode::Escape);
        race.step(dt, &mut self.input);
        self.input.end_frame();
        let camera = race.camera();
        let models = race.car_models();
        let objects = race.object_models();
        let hud = race.hud();
        if let Err(err) = gpu.render(window, Some(&camera), &models, &objects, |ui| {
            crate::ui::show_drive(ui.ctx(), &hud)
        }) {
            tracing::error!(%err, "falló el frame");
            event_loop.exit();
            return;
        }
        // Después del frame: así el Esc no le llega también al menú.
        if to_menu {
            match self.open_menu() {
                Ok(()) => {
                    if let Some(menu) = self.menu.as_mut() {
                        menu.state.back_from_race();
                    }
                }
                Err(err) => {
                    tracing::error!(%err, "no se pudo cargar el menú");
                    event_loop.exit();
                }
            }
        }
    }

    fn redraw_menu(&mut self, event_loop: &ActiveEventLoop, window: &Window, dt: f32) {
        let (Some(menu), Some(gpu)) = (self.menu.as_mut(), self.gpu.as_mut()) else {
            return;
        };
        menu.step(dt);
        self.input.end_frame();
        let camera = menu.camera();
        let mut action = None;
        let frame = gpu.render(window, Some(&camera), &[], &[], |ui| {
            if let Some(done) = crate::ui::menu::show(ui, &mut menu.state) {
                action.get_or_insert(done);
            }
        });
        if let Err(err) = frame {
            tracing::error!(%err, "falló el frame");
            event_loop.exit();
            return;
        }
        match action {
            Some(MenuAction::Quit) => event_loop.exit(),
            Some(MenuAction::StartRace { track, cars }) => {
                // Los jugadores primero; detrás, los autos extra de la config.
                let cars = cars
                    .into_iter()
                    .chain(self.config.extra_cars.iter().cloned())
                    .collect();
                self.pending = Some((Race { level: track, cars }, false));
            }
            None => {}
        }
    }
}

/// La pista tal como la sube la escena.
struct TrackScene<'a> {
    meshes: &'a [VisualMesh],
    textures: &'a [(i16, image::RgbaImage)],
    color_key: bool,
    sky: Option<&'a [image::RgbaImage; 6]>,
    background: Option<[u8; 3]>,
}

/// Cambia lo que dibuja la escena: la pista, su cielo, los autos y los tipos de objeto
/// (en el menú, ni autos ni objetos).
fn upload_scene(
    gpu: &mut Gpu,
    track: &TrackScene,
    cars: &[(&[Vec<VisualMesh>], Option<&image::RgbaImage>)],
    objects: &[ObjectMeshes],
) {
    let device = gpu.device().clone();
    let queue = gpu.queue().clone();
    if let Some(scene) = gpu.scene_mut() {
        scene.upload_track(
            &device,
            &queue,
            track.meshes,
            track.textures,
            track.color_key,
        );
        match track.sky {
            Some(sky) => scene.upload_sky(&device, &queue, sky),
            None => scene.clear_sky(&device, &queue, track.background),
        }
        scene.upload_cars(&device, &queue, cars);
        scene.upload_objects(&device, &queue, objects);
    }
}
