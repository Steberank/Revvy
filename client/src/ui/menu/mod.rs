//! Menú principal y panel Offline, dibujados con egui encima de la pista de fondo.
//!
//! Teclado: ↑/↓ (o W/S) mueven la marca, ←/→ (o A/D) cambian números, Enter o Espacio
//! aceptan, Esc vuelve, Q y E cambian de pestaña. El mouse marca al pasar y acepta con
//! un click. Todo pasa por `MenuState`: acá solo se dibuja y se traduce la entrada.

mod state;

use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, CursorIcon, FontId, Key, Painter, Pos2, Rect,
    Response, Sense, Shape, Stroke, StrokeKind, Vec2,
};

use super::theme::{
    shadow_text, ACCENT, ACCENT_DARK, BUTTON, CARD, CARD_BORDER, CARD_EMPTY, DARK, DIM, HIGHLIGHT,
    PANEL, READY, TAB_IDLE, TEXT,
};
use state::{Command, MainOption, RoomItem, Screen, Setting, Tab, Target};
pub use state::{MenuAction, MenuState, TrackEntry};

const OPTION_HEIGHT: f32 = 46.0;
const TAB_HEIGHT: f32 = 36.0;
const ROW_HEIGHT: f32 = 48.0;
const FOOTER_HEIGHT: f32 = 40.0;

/// Teclas del menú. Las repeticiones cuentan: mantener ↓ baja por la lista.
const KEYS: [(Key, Command); 14] = [
    (Key::ArrowUp, Command::Up),
    (Key::W, Command::Up),
    (Key::ArrowDown, Command::Down),
    (Key::S, Command::Down),
    (Key::ArrowLeft, Command::Left),
    (Key::A, Command::Left),
    (Key::ArrowRight, Command::Right),
    (Key::D, Command::Right),
    (Key::Enter, Command::Accept),
    (Key::Space, Command::Accept),
    (Key::Escape, Command::Back),
    (Key::Backspace, Command::Back),
    (Key::Q, Command::PrevTab),
    (Key::E, Command::NextTab),
];

/// Dibuja el menú y aplica el teclado y el mouse del frame.
pub fn show(ui: &mut egui::Ui, state: &mut MenuState) -> Option<MenuAction> {
    let mut action = None;
    // En el orden en que llegaron: varias teclas en un mismo frame no se reordenan.
    let commands: Vec<Command> = ui.input(|input| {
        input
            .events
            .iter()
            .filter_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if !(modifiers.alt || modifiers.ctrl || modifiers.command) => KEYS
                    .iter()
                    .find(|(bound, _)| bound == key)
                    .map(|&(_, command)| command),
                _ => None,
            })
            .collect()
    });
    for command in commands {
        if let Some(done) = state.command(command) {
            action.get_or_insert(done);
        }
    }

    let mut menu = Menu {
        pointer_moved: ui.input(|input| input.pointer.delta() != egui::Vec2::ZERO),
        painter: ui.painter().clone(),
        layout: Layout::new(ui.max_rect()),
        ui,
        state,
        action,
    };
    menu.backdrop();
    menu.title();
    menu.main_options();
    if menu.state.screen == Screen::Offline {
        menu.offline_panel();
        if menu.state.can_start() {
            menu.start_button();
        }
    }
    if menu.state.loading {
        menu.loading();
    }
    menu.action
}

/// Dónde va cada cosa, según el tamaño de la ventana.
struct Layout {
    screen: Rect,
    margin: f32,
    /// Las opciones del menú principal, a la izquierda y al centro.
    column: Rect,
    /// El panel Offline, a la derecha de las opciones. Las pestañas van arriba y Iniciar
    /// Carrera abajo.
    panel: Rect,
}

impl Layout {
    fn new(screen: Rect) -> Self {
        let margin = (screen.width() * 0.04).clamp(20.0, 56.0);
        let height = MainOption::ALL.len() as f32 * OPTION_HEIGHT;
        let column = Rect::from_min_size(
            pos2(
                screen.left() + margin,
                screen.center().y - height * 0.5 + 24.0,
            ),
            vec2(240.0, height),
        );
        let panel = Rect::from_min_max(
            pos2(column.right() + margin * 0.75, screen.top() + margin + 64.0),
            pos2(screen.right() - margin, screen.bottom() - margin - 72.0),
        );
        Self {
            screen,
            margin,
            column,
            panel,
        }
    }
}

struct Menu<'a> {
    ui: &'a egui::Ui,
    painter: Painter,
    state: &'a mut MenuState,
    action: Option<MenuAction>,
    /// El mouse solo marca cuando se mueve: si queda quieto sobre una opción, no le gana
    /// al teclado.
    pointer_moved: bool,
    layout: Layout,
}

impl Menu<'_> {
    /// Área del mouse para `target`: al pasar lo marca y con un click lo acepta.
    fn interact(&mut self, rect: Rect, target: Target) -> Response {
        let response = self
            .ui
            .interact(rect, self.ui.id().with(("menu", target)), Sense::CLICK)
            .on_hover_cursor(CursorIcon::PointingHand);
        if response.hovered() && self.pointer_moved {
            self.state.point(target);
        }
        if response.clicked() {
            if let Some(done) = self.state.click(target) {
                self.action.get_or_insert(done);
            }
        }
        response
    }

    /// Oscurece la izquierda para que el texto se lea sobre la pista.
    fn backdrop(&self) {
        let screen = self.layout.screen;
        let right = screen.left() + (screen.width() * 0.45).max(420.0);
        let dark = Color32::from_black_alpha(170);
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(screen.left_top(), dark);
        mesh.colored_vertex(pos2(right, screen.top()), Color32::TRANSPARENT);
        mesh.colored_vertex(pos2(right, screen.bottom()), Color32::TRANSPARENT);
        mesh.colored_vertex(screen.left_bottom(), dark);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        self.painter.add(Shape::mesh(mesh));
    }

    fn title(&self) {
        let pos =
            self.layout.screen.left_top() + vec2(self.layout.margin, self.layout.margin * 0.7);
        shadow_text(
            &self.painter,
            pos,
            Align2::LEFT_TOP,
            "REVVY",
            FontId::proportional(60.0),
            TEXT,
        );
    }

    /// Las cinco opciones. Con el panel abierto quedan atenuadas y no responden.
    fn main_options(&mut self) {
        let active = self.state.screen == Screen::Main;
        for (i, option) in MainOption::ALL.into_iter().enumerate() {
            let rect = Rect::from_min_size(
                self.layout.column.min + vec2(0.0, i as f32 * OPTION_HEIGHT),
                vec2(self.layout.column.width(), OPTION_HEIGHT),
            );
            if active {
                self.interact(rect, Target::Main(option));
            }
            let marked = self.state.main == option;
            if marked {
                highlight(&self.painter, rect.shrink2(vec2(0.0, 4.0)));
            }
            let color = match (active, marked) {
                (_, true) => ACCENT,
                (true, false) => TEXT,
                (false, false) => DIM,
            };
            let pos = pos2(rect.left() + 18.0, rect.center().y);
            shadow_text(
                &self.painter,
                pos,
                Align2::LEFT_CENTER,
                option.label(),
                FontId::proportional(30.0),
                color,
            );
        }
    }

    fn offline_panel(&mut self) {
        let panel = self.layout.panel;
        self.tabs(panel);
        self.painter
            .rect_filled(panel, CornerRadius::same(10), PANEL);
        let content = Rect::from_min_max(
            panel.min + vec2(24.0, 24.0),
            panel.max - vec2(24.0, 12.0 + FOOTER_HEIGHT),
        );
        match self.state.tab {
            Tab::Room => self.room(content),
            Tab::Track => self.track_select(content),
            Tab::Settings => self.settings(content),
        }
        self.footer(panel);
    }

    /// Los pliegues de arriba del panel, con Q y E a los costados.
    fn tabs(&mut self, panel: Rect) {
        let top = panel.top() - TAB_HEIGHT;
        let middle = top + TAB_HEIGHT * 0.5;
        let font = FontId::proportional(19.0);
        let mut x = keycap(&self.painter, panel.left() + 14.0, middle, Cap::Key("Q")).right() + 8.0;
        for tab in Tab::ALL {
            let label = self
                .painter
                .layout_no_wrap(tab.label().to_string(), font.clone(), TEXT);
            let rect = Rect::from_min_size(pos2(x, top), vec2(label.size().x + 36.0, TAB_HEIGHT));
            let hovered = self.interact(rect, Target::Tab(tab)).hovered();
            let active = self.state.tab == tab;
            let corners = CornerRadius {
                nw: 8,
                ne: 8,
                sw: 0,
                se: 0,
            };
            self.painter
                .rect_filled(rect, corners, if active { PANEL } else { TAB_IDLE });
            if active {
                let line = Rect::from_min_size(
                    rect.left_top() + vec2(8.0, 0.0),
                    vec2(rect.width() - 16.0, 3.0),
                );
                self.painter
                    .rect_filled(line, CornerRadius::same(1), ACCENT);
            }
            let color = if active || hovered { TEXT } else { DIM };
            self.painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                tab.label(),
                font.clone(),
                color,
            );
            x = rect.right() + 6.0;
        }
        keycap(&self.painter, x + 2.0, middle, Cap::Key("E"));
    }

    /// Sala: una tarjeta por puesto local.
    fn room(&mut self, content: Rect) {
        let gap = 16.0;
        let slots = self.state.players.len();
        let width = (content.width() - gap * (slots as f32 - 1.0)) / slots as f32;
        for slot in 0..slots {
            let rect = Rect::from_min_size(
                content.min + vec2(slot as f32 * (width + gap), 0.0),
                vec2(width, content.height()),
            );
            self.player_card(slot, rect);
        }
    }

    /// De arriba abajo: nombre, auto (o el aviso de conectar un mando), Cambiar auto y,
    /// al final, Estoy listo.
    fn player_card(&mut self, slot: usize, rect: Rect) {
        let inner = rect.shrink(16.0);
        let line_y = inner.top() + 38.0;
        let change_car =
            Rect::from_min_size(pos2(inner.left(), line_y + 84.0), vec2(inner.width(), 40.0));
        let ready = Rect::from_min_size(
            pos2(inner.left(), inner.bottom() - 44.0),
            vec2(inner.width(), 44.0),
        );
        let buttons = [(RoomItem::ChangeCar, change_car), (RoomItem::Ready, ready)];
        let keyboard = self.state.keyboard() == Some(slot);
        if keyboard {
            for (index, (_, button)) in buttons.iter().enumerate() {
                self.interact(*button, Target::Item(index));
            }
        }

        let player = &self.state.players[slot];
        let connected = player.device.is_some();
        self.painter.rect_filled(
            rect,
            CornerRadius::same(8),
            if connected { CARD } else { CARD_EMPTY },
        );
        let border = if player.ready {
            Stroke::new(2.0, READY)
        } else {
            Stroke::new(1.0, CARD_BORDER)
        };
        self.painter
            .rect_stroke(rect, CornerRadius::same(8), border, StrokeKind::Inside);
        let name_color = if connected { TEXT } else { DIM };
        self.painter.text(
            inner.left_top(),
            Align2::LEFT_TOP,
            &player.name,
            FontId::proportional(22.0),
            name_color,
        );
        let line = [pos2(inner.left(), line_y), pos2(inner.right(), line_y)];
        self.painter
            .line_segment(line, Stroke::new(1.0, Color32::from_white_alpha(28)));
        if !connected {
            let message = "Conecte un mando para iniciar".to_string();
            let galley =
                self.painter
                    .layout(message, FontId::proportional(17.0), DIM, inner.width());
            self.painter
                .galley(pos2(inner.left(), line_y + 20.0), galley, DIM);
            return;
        }
        self.painter.text(
            pos2(inner.left(), line_y + 18.0),
            Align2::LEFT_TOP,
            "AUTO",
            FontId::proportional(12.0),
            DIM,
        );
        self.painter.text(
            pos2(inner.left(), line_y + 36.0),
            Align2::LEFT_TOP,
            &player.car_name,
            FontId::proportional(24.0),
            TEXT,
        );
        for (index, (item, rect)) in buttons.into_iter().enumerate() {
            let marked = keyboard && self.state.focus() == Some(Target::Item(index));
            let (label, fill, color) = match item {
                RoomItem::ChangeCar => ("Cambiar auto", BUTTON, TEXT),
                RoomItem::Ready if player.ready => ("Listo", READY, DARK),
                RoomItem::Ready => ("Estoy listo", BUTTON, TEXT),
            };
            button(&self.painter, rect, label, fill, color, marked);
        }
    }

    /// Seleccionar pista: la lista a la izquierda y el resumen de la pista a la derecha. Si
    /// no entran todas, la lista se corre con la marca (también con la rueda del mouse).
    fn track_select(&mut self, content: Rect) {
        let list = Rect::from_min_size(content.min, vec2(content.width() * 0.4, content.height()));
        let summary = Rect::from_min_max(pos2(list.right() + 28.0, content.top()), content.max);
        let hovering = self
            .ui
            .input(|input| input.pointer.hover_pos())
            .is_some_and(|pos| list.contains(pos));
        if hovering {
            let lines = self.ui.input(|input| {
                input
                    .events
                    .iter()
                    .map(|event| match event {
                        egui::Event::MouseWheel { unit, delta, .. } => match unit {
                            egui::MouseWheelUnit::Line => delta.y,
                            egui::MouseWheelUnit::Point => delta.y / ROW_HEIGHT,
                            egui::MouseWheelUnit::Page => delta.y * 5.0,
                        },
                        _ => 0.0,
                    })
                    .sum::<f32>()
            });
            self.state.wheel_tracks(lines);
        }
        let visible = ((list.height() - 16.0) / ROW_HEIGHT).floor().max(1.0) as usize;
        let first = self.state.follow_tracks(visible);
        let shown: Vec<(usize, Rect)> = (first..self.state.tracks.len())
            .take(visible)
            .enumerate()
            .map(|(row, i)| {
                let min = list.min + vec2(0.0, 8.0 + row as f32 * ROW_HEIGHT);
                (
                    i,
                    Rect::from_min_size(min, vec2(list.width(), ROW_HEIGHT))
                        .shrink2(vec2(8.0, 2.0)),
                )
            })
            .collect();
        for &(i, row) in &shown {
            self.interact(row, Target::Item(i));
        }

        self.painter.rect_filled(list, CornerRadius::same(8), CARD);
        for &(i, row) in &shown {
            if self.state.focus() == Some(Target::Item(i)) {
                highlight(&self.painter, row);
            }
            let chosen = self.state.selected_track == Some(i);
            let pos = pos2(row.left() + 18.0, row.center().y);
            let title = &self.state.tracks[i].title;
            let color = if chosen { ACCENT } else { TEXT };
            self.painter.text(
                pos,
                Align2::LEFT_CENTER,
                title,
                FontId::proportional(20.0),
                color,
            );
            if chosen {
                check_mark(
                    &self.painter,
                    pos2(row.right() - 22.0, row.center().y),
                    READY,
                );
            }
        }
        let total = self.state.tracks.len();
        if total > visible {
            // Barrita: qué parte de la lista se ve.
            let bar = Rect::from_min_max(
                pos2(list.right() - 5.0, list.top() + 8.0),
                pos2(list.right() - 2.0, list.bottom() - 8.0),
            );
            let top = bar.top() + bar.height() * first as f32 / total as f32;
            let height = bar.height() * visible as f32 / total as f32;
            let thumb = Rect::from_min_size(pos2(bar.left(), top), vec2(bar.width(), height));
            self.painter
                .rect_filled(bar, CornerRadius::same(1), Color32::from_white_alpha(20));
            self.painter.rect_filled(thumb, CornerRadius::same(1), DIM);
        }
        self.track_summary(summary);
    }

    /// Título de la pista y portada: `gfx/<pista>.bmp` o el `preview.png` de una `.glb`.
    fn track_summary(&mut self, area: Rect) {
        let Some(i) = self.state.previewed_track() else {
            let font = FontId::proportional(20.0);
            self.painter.text(
                area.center(),
                Align2::CENTER_CENTER,
                "Elija una pista",
                font,
                DIM,
            );
            return;
        };
        let chosen = self.state.selected_track == Some(i);
        let entry = &mut self.state.tracks[i];
        if entry.texture.is_none() {
            if let Some(cover) = entry.cover.take() {
                let name = format!("portada-{}", entry.id);
                entry.texture = Some(self.ui.ctx().load_texture(
                    name,
                    cover,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }

        self.painter.text(
            area.left_top(),
            Align2::LEFT_TOP,
            &entry.title,
            FontId::proportional(28.0),
            TEXT,
        );
        let (note, color) = if chosen {
            ("Pista elegida", READY)
        } else {
            ("Enter o click para elegirla", DIM)
        };
        let note_pos = area.left_top() + vec2(0.0, 38.0);
        self.painter.text(
            note_pos,
            Align2::LEFT_TOP,
            note,
            FontId::proportional(15.0),
            color,
        );

        let top = area.top() + 70.0;
        let side = area.width().min(area.bottom() - top).max(0.0);
        let cover = Rect::from_min_size(pos2(area.left(), top), vec2(side, side));
        match &entry.texture {
            Some(texture) => {
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                self.painter.image(texture.id(), cover, uv, Color32::WHITE);
                let border = Stroke::new(1.0, Color32::from_white_alpha(40));
                self.painter
                    .rect_stroke(cover, CornerRadius::ZERO, border, StrokeKind::Outside);
            }
            None => {
                self.painter.rect_filled(cover, CornerRadius::same(6), CARD);
                let font = FontId::proportional(18.0);
                self.painter.text(
                    cover.center(),
                    Align2::CENTER_CENTER,
                    "Sin portada",
                    font,
                    DIM,
                );
            }
        }
    }

    /// Configuración de sala: dos tildes y dos números.
    fn settings(&mut self, content: Rect) {
        let width = content.width().min(560.0);
        let rows: Vec<Rect> = (0..Setting::ALL.len())
            .map(|i| {
                Rect::from_min_size(
                    content.min + vec2(0.0, i as f32 * (ROW_HEIGHT + 6.0)),
                    vec2(width, ROW_HEIGHT),
                )
            })
            .collect();
        for (i, (setting, row)) in Setting::ALL.into_iter().zip(&rows).enumerate() {
            self.interact(*row, Target::Item(i));
            // Las flechas quedan encima de la fila: su click cambia el número.
            if matches!(setting, Setting::BotCount | Setting::Laps) && self.enabled(setting) {
                let (left, _, right) = stepper_rects(*row);
                for (rect, delta) in [(left, -1), (right, 1)] {
                    let id = self.ui.id().with(("menu-flecha", i, delta));
                    let response = self
                        .ui
                        .interact(rect, id, Sense::CLICK)
                        .on_hover_cursor(CursorIcon::PointingHand);
                    if response.clicked() {
                        self.state.nudge(i, delta);
                    }
                }
            }
        }

        for (i, (setting, row)) in Setting::ALL.into_iter().zip(rows).enumerate() {
            if self.state.focus() == Some(Target::Item(i)) {
                highlight(&self.painter, row);
            }
            let enabled = self.enabled(setting);
            let color = if enabled { TEXT } else { DIM };
            let pos = pos2(row.left() + 18.0, row.center().y);
            self.painter.text(
                pos,
                Align2::LEFT_CENTER,
                setting.label(),
                FontId::proportional(20.0),
                color,
            );
            let settings = self.state.settings;
            let check = pos2(row.right() - 28.0, row.center().y);
            match setting {
                Setting::Bots => checkbox(&self.painter, check, settings.bots),
                Setting::Powerups => checkbox(&self.painter, check, settings.powerups),
                Setting::BotCount => stepper(&self.painter, row, settings.bot_count, enabled),
                Setting::Laps => stepper(&self.painter, row, settings.laps, enabled),
            }
        }
    }

    /// Sin bots, la cantidad no se toca.
    fn enabled(&self, setting: Setting) -> bool {
        setting != Setting::BotCount || self.state.settings.bots
    }

    /// Esc para volver (también con click) y las teclas de la pestaña.
    fn footer(&mut self, panel: Rect) {
        let middle = panel.bottom() - FOOTER_HEIGHT * 0.5 - 6.0;
        let back = Rect::from_min_size(pos2(panel.left() + 20.0, middle - 14.0), vec2(104.0, 28.0));
        let hovered = self.interact(back, Target::Back).hovered();
        let right = keycap(&self.painter, back.left(), middle, Cap::Key("Esc")).right();
        let color = if hovered { TEXT } else { DIM };
        self.painter.text(
            pos2(right + 8.0, middle),
            Align2::LEFT_CENTER,
            "Volver",
            FontId::proportional(16.0),
            color,
        );

        let vertical = [Cap::Arrow(Vec2::UP), Cap::Arrow(Vec2::DOWN)];
        let horizontal = [Cap::Arrow(Vec2::LEFT), Cap::Arrow(Vec2::RIGHT)];
        let enter = [Cap::Key("Enter")];
        let mut hints: Vec<(&[Cap], &str)> = vec![(&vertical, "Moverse")];
        if self.state.tab == Tab::Settings {
            hints.push((&horizontal, "Cambiar"));
        }
        hints.push((&enter, "Aceptar"));
        // De derecha a izquierda: cada texto y, antes, sus teclas.
        let mut x = panel.right() - 20.0;
        for (caps, label) in hints.into_iter().rev() {
            let font = FontId::proportional(15.0);
            let text = self
                .painter
                .text(pos2(x, middle), Align2::RIGHT_CENTER, label, font, DIM);
            x = text.left() - 6.0;
            for &cap in caps.iter().rev() {
                x = keycap(
                    &self.painter,
                    x - keycap_width(&self.painter, cap),
                    middle,
                    cap,
                )
                .left()
                    - 4.0;
            }
            x -= 18.0;
        }
    }

    /// Abajo, bajo el panel, cuando todos están listos y hay pista.
    fn start_button(&mut self) {
        let size = vec2(300.0, 54.0);
        let y = self.layout.screen.bottom() - self.layout.margin * 0.6 - size.y * 0.5;
        let rect = Rect::from_center_size(pos2(self.layout.panel.center().x, y), size);
        let hovered = self.interact(rect, Target::Start).hovered();
        let marked = self.state.focus() == Some(Target::Start);
        let fill = if marked || hovered {
            ACCENT
        } else {
            ACCENT_DARK
        };
        self.painter.rect_filled(rect, CornerRadius::same(8), fill);
        if marked {
            let ring = Stroke::new(2.0, TEXT);
            self.painter.rect_stroke(
                rect.expand(4.0),
                CornerRadius::same(11),
                ring,
                StrokeKind::Outside,
            );
        }
        let font = FontId::proportional(24.0);
        self.painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Iniciar Carrera",
            font,
            DARK,
        );
    }

    fn loading(&self) {
        let screen = self.layout.screen;
        self.painter
            .rect_filled(screen, CornerRadius::ZERO, Color32::from_black_alpha(150));
        let font = FontId::proportional(32.0);
        shadow_text(
            &self.painter,
            screen.center(),
            Align2::CENTER_CENTER,
            "Cargando pista…",
            font,
            TEXT,
        );
    }
}

/// La marca: un fondo tenue y una barrita del color de acento a la izquierda.
fn highlight(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, CornerRadius::same(4), HIGHLIGHT);
    let bar = Rect::from_min_size(
        rect.left_top() + vec2(0.0, rect.height() * 0.2),
        vec2(4.0, rect.height() * 0.6),
    );
    painter.rect_filled(bar, CornerRadius::same(2), ACCENT);
}

/// Botón de tarjeta. Marcado lleva un borde del color de acento.
fn button(painter: &Painter, rect: Rect, label: &str, fill: Color32, color: Color32, marked: bool) {
    painter.rect_filled(rect, CornerRadius::same(6), fill);
    if marked {
        painter.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(2.0, ACCENT),
            StrokeKind::Inside,
        );
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(18.0),
        color,
    );
}

/// Lo que lleva una tecla dibujada. Las flechas se dibujan: la fuente no las trae.
#[derive(Clone, Copy)]
enum Cap {
    Key(&'static str),
    Arrow(Vec2),
}

fn keycap_width(painter: &Painter, cap: Cap) -> f32 {
    match cap {
        Cap::Key(key) => {
            painter
                .layout_no_wrap(key.to_string(), keycap_font(), TEXT)
                .size()
                .x
                .max(12.0)
                + 10.0
        }
        Cap::Arrow(_) => 22.0,
    }
}

fn keycap_font() -> FontId {
    FontId::proportional(13.0)
}

/// Una tecla dibujada, con el borde izquierdo en `left`.
fn keycap(painter: &Painter, left: f32, middle: f32, cap: Cap) -> Rect {
    let rect = Rect::from_min_size(
        pos2(left, middle - 11.0),
        vec2(keycap_width(painter, cap), 22.0),
    );
    painter.rect(
        rect,
        CornerRadius::same(4),
        Color32::from_black_alpha(120),
        Stroke::new(1.0, DIM),
        StrokeKind::Inside,
    );
    match cap {
        Cap::Key(key) => {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                key,
                keycap_font(),
                TEXT,
            );
        }
        Cap::Arrow(direction) => triangle(painter, rect.center(), direction, 4.5, TEXT),
    }
    rect
}

fn checkbox(painter: &Painter, center: Pos2, checked: bool) {
    let rect = Rect::from_center_size(center, vec2(24.0, 24.0));
    if checked {
        painter.rect_filled(rect, CornerRadius::same(5), ACCENT);
        check_mark(painter, rect.center(), DARK);
    } else {
        painter.rect_stroke(
            rect,
            CornerRadius::same(5),
            Stroke::new(2.0, DIM),
            StrokeKind::Inside,
        );
    }
}

fn check_mark(painter: &Painter, center: Pos2, color: Color32) {
    let points = vec![
        center + vec2(-6.0, 0.0),
        center + vec2(-2.0, 5.0),
        center + vec2(7.0, -6.0),
    ];
    painter.add(Shape::line(points, Stroke::new(2.5, color)));
}

/// Flecha izquierda, valor y flecha derecha, pegados a la derecha de la fila.
fn stepper_rects(row: Rect) -> (Rect, Rect, Rect) {
    let middle = row.center().y;
    let right = Rect::from_center_size(pos2(row.right() - 28.0, middle), vec2(32.0, 32.0));
    let value = Rect::from_center_size(pos2(right.left() - 26.0, middle), vec2(44.0, 32.0));
    let left = Rect::from_center_size(pos2(value.left() - 18.0, middle), vec2(32.0, 32.0));
    (left, value, right)
}

fn stepper(painter: &Painter, row: Rect, value: u32, enabled: bool) {
    let (left, value_rect, right) = stepper_rects(row);
    let color = if enabled {
        TEXT
    } else {
        DIM.gamma_multiply(0.6)
    };
    triangle(painter, left.center(), Vec2::LEFT, 6.5, color);
    painter.text(
        value_rect.center(),
        Align2::CENTER_CENTER,
        value.to_string(),
        FontId::proportional(22.0),
        color,
    );
    triangle(painter, right.center(), Vec2::RIGHT, 6.5, color);
}

/// Triángulo de lado `size` que apunta hacia `direction`.
fn triangle(painter: &Painter, center: Pos2, direction: Vec2, size: f32, color: Color32) {
    let side = vec2(-direction.y, direction.x) * size;
    let back = center - direction * size * 0.8;
    let points = vec![center + direction * size, back + side, back - side];
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}
