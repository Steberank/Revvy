//! Estado del menú, sin dibujar: la opción marcada, la pestaña de Offline que se ve, quién
//! está listo y qué pista se eligió. La UI lo dibuja y le pasa lo que hacen el teclado y
//! el mouse.

/// Puestos de jugadores locales en una sala Offline: el teclado y, más adelante, mandos.
pub const LOCAL_SLOTS: usize = 4;
/// Autos de una sala (§5 de la arquitectura): los bots completan hasta acá.
const MAX_CARS: u32 = 32;
const MAX_LAPS: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MainOption {
    Offline,
    Multiplayer,
    Profile,
    Options,
    Quit,
}

impl MainOption {
    pub const ALL: [Self; 5] = [
        Self::Offline,
        Self::Multiplayer,
        Self::Profile,
        Self::Options,
        Self::Quit,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Offline => "Offline",
            Self::Multiplayer => "Multiplayer",
            Self::Profile => "Perfil",
            Self::Options => "Opciones",
            Self::Quit => "Salir",
        }
    }
}

/// Pestañas del panel Offline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    Room,
    Track,
    Settings,
}

impl Tab {
    pub const ALL: [Self; 3] = [Self::Room, Self::Track, Self::Settings];

    pub fn label(self) -> &'static str {
        match self {
            Self::Room => "Sala",
            Self::Track => "Seleccionar pista",
            Self::Settings => "Configuración de sala",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Room => 0,
            Self::Track => 1,
            Self::Settings => 2,
        }
    }
}

/// Opciones de la tarjeta del jugador del teclado, de arriba abajo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomItem {
    ChangeCar,
    Ready,
}

impl RoomItem {
    pub const ALL: [Self; 2] = [Self::ChangeCar, Self::Ready];
}

/// Filas de Configuración de sala.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Bots,
    BotCount,
    Laps,
    Powerups,
}

impl Setting {
    pub const ALL: [Self; 4] = [Self::Bots, Self::BotCount, Self::Laps, Self::Powerups];

    pub fn label(self) -> &'static str {
        match self {
            Self::Bots => "Bots",
            Self::BotCount => "Cantidad de bots",
            Self::Laps => "Cantidad de vueltas",
            Self::Powerups => "Poderes",
        }
    }
}

/// Con qué juega un jugador local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Keyboard,
}

#[derive(Clone, Debug)]
pub struct PlayerSlot {
    pub name: String,
    /// `None`: el puesto espera un mando.
    pub device: Option<Device>,
    /// Id dentro de `cars/` y el nombre del auto.
    pub car_id: String,
    pub car_name: String,
    pub ready: bool,
}

pub struct TrackEntry {
    pub id: String,
    pub title: String,
    /// `gfx/<id>.bmp`. La UI la pasa a `texture` la primera vez que la dibuja.
    pub cover: Option<egui::ColorImage>,
    pub texture: Option<egui::TextureHandle>,
}

impl TrackEntry {
    pub fn new(id: &str, title: &str, cover: Option<egui::ColorImage>) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            cover,
            texture: None,
        }
    }
}

/// Configuración de sala. Todavía no cambia la carrera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomSettings {
    pub bots: bool,
    pub bot_count: u32,
    pub laps: u32,
    pub powerups: bool,
}

impl Default for RoomSettings {
    fn default() -> Self {
        Self {
            bots: true,
            bot_count: 7,
            laps: 3,
            powerups: true,
        }
    }
}

/// Una tecla ya traducida.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Up,
    Down,
    Left,
    Right,
    Accept,
    Back,
    PrevTab,
    NextTab,
}

/// Lo que el mouse puede marcar o tocar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Main(MainOption),
    Tab(Tab),
    /// Una opción de la pestaña que se ve, en el orden de `items`.
    Item(usize),
    Start,
    Back,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuAction {
    Quit,
    /// La pista y los autos de los jugadores conectados, en orden.
    StartRace {
        track: String,
        cars: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Main,
    Offline,
}

pub struct MenuState {
    pub screen: Screen,
    /// La opción marcada del menú principal.
    pub main: MainOption,
    pub tab: Tab,
    /// La marca de cada pestaña. Pasando las opciones está Iniciar Carrera.
    cursors: [usize; 3],
    pub players: Vec<PlayerSlot>,
    pub tracks: Vec<TrackEntry>,
    pub selected_track: Option<usize>,
    /// La primera pista visible de la lista.
    track_scroll: usize,
    /// Rueda del mouse sobre la lista, en líneas, hasta completar una.
    wheel: f32,
    pub settings: RoomSettings,
    /// Se pidió la carrera: el menú queda tapado hasta que cargue.
    pub loading: bool,
}

impl MenuState {
    /// El teclado ocupa el primer puesto; los demás esperan un mando.
    pub fn new(player_name: &str, car_id: &str, car_name: &str, tracks: Vec<TrackEntry>) -> Self {
        let keyboard = PlayerSlot {
            name: player_name.to_string(),
            device: Some(Device::Keyboard),
            car_id: car_id.to_string(),
            car_name: car_name.to_string(),
            ready: false,
        };
        let empty = (2..=LOCAL_SLOTS).map(|n| PlayerSlot {
            name: format!("Jugador {n}"),
            device: None,
            car_id: String::new(),
            car_name: String::new(),
            ready: false,
        });
        Self {
            screen: Screen::Main,
            main: MainOption::Offline,
            tab: Tab::Room,
            cursors: [0; 3],
            players: std::iter::once(keyboard).chain(empty).collect(),
            tracks,
            selected_track: None,
            track_scroll: 0,
            wheel: 0.0,
            settings: RoomSettings::default(),
            loading: false,
        }
    }

    /// Hay pista elegida y todos los jugadores conectados están listos.
    pub fn can_start(&self) -> bool {
        let mut connected = self
            .players
            .iter()
            .filter(|player| player.device.is_some())
            .peekable();
        self.selected_track.is_some()
            && connected.peek().is_some()
            && connected.all(|player| player.ready)
    }

    /// El puesto que maneja el teclado.
    pub fn keyboard(&self) -> Option<usize> {
        self.players
            .iter()
            .position(|player| player.device == Some(Device::Keyboard))
    }

    /// Opciones de la pestaña, sin contar Iniciar Carrera. En la sala solo tiene opciones
    /// la tarjeta del teclado.
    pub fn items(&self, tab: Tab) -> usize {
        match tab {
            Tab::Room if self.keyboard().is_some() => RoomItem::ALL.len(),
            Tab::Room => 0,
            Tab::Track => self.tracks.len(),
            Tab::Settings => Setting::ALL.len(),
        }
    }

    /// Lo marcado en la pestaña que se ve: una opción o Iniciar Carrera.
    pub fn focus(&self) -> Option<Target> {
        let items = self.items(self.tab);
        let len = items + usize::from(self.can_start());
        let cursor = self.cursors[self.tab.index()].min(len.checked_sub(1)?);
        Some(if cursor == items {
            Target::Start
        } else {
            Target::Item(cursor)
        })
    }

    /// La pista del resumen: la marcada en la lista o, si no, la elegida.
    pub fn previewed_track(&self) -> Option<usize> {
        match (self.tab, self.focus()) {
            (Tab::Track, Some(Target::Item(i))) => Some(i),
            _ => self.selected_track,
        }
    }

    /// La primera pista a mostrar en una lista de `visible` filas: se corre lo justo para
    /// que la marca quede a la vista.
    pub fn follow_tracks(&mut self, visible: usize) -> usize {
        let visible = visible.max(1);
        if let (Tab::Track, Some(Target::Item(i))) = (self.tab, self.focus()) {
            if i < self.track_scroll {
                self.track_scroll = i;
            } else if i >= self.track_scroll + visible {
                self.track_scroll = i + 1 - visible;
            }
        }
        self.track_scroll = self
            .track_scroll
            .min(self.tracks.len().saturating_sub(visible));
        self.track_scroll
    }

    /// Rueda del mouse sobre la lista de pistas: cada línea mueve la marca una pista,
    /// sin salir de la lista.
    pub fn wheel_tracks(&mut self, lines: f32) {
        if self.loading || self.tab != Tab::Track {
            return;
        }
        self.wheel += lines;
        let steps = self.wheel.trunc();
        self.wheel -= steps;
        let Some(Target::Item(i)) = self.focus() else {
            return;
        };
        let last = self.tracks.len() as i32 - 1;
        // Rueda hacia arriba (positiva) sube por la lista.
        self.cursors[Tab::Track.index()] = (i as i32 - steps as i32).clamp(0, last) as usize;
    }

    /// Los bots completan la sala hasta `MAX_CARS` autos.
    pub fn max_bots(&self) -> u32 {
        let connected = self
            .players
            .iter()
            .filter(|player| player.device.is_some())
            .count();
        MAX_CARS.saturating_sub(connected as u32)
    }

    pub fn command(&mut self, command: Command) -> Option<MenuAction> {
        if self.loading {
            return None;
        }
        match (self.screen, command) {
            (Screen::Main, Command::Up) => self.main = wrap(MainOption::ALL, self.main, -1),
            (Screen::Main, Command::Down) => self.main = wrap(MainOption::ALL, self.main, 1),
            (Screen::Main, Command::Accept) => return self.activate(self.main),
            (Screen::Main, _) => {}
            (Screen::Offline, Command::Up) => self.move_cursor(-1),
            (Screen::Offline, Command::Down) => self.move_cursor(1),
            (Screen::Offline, Command::Left) => self.adjust(-1),
            (Screen::Offline, Command::Right) => self.adjust(1),
            (Screen::Offline, Command::Accept) => return self.accept(),
            (Screen::Offline, Command::Back) => self.screen = Screen::Main,
            (Screen::Offline, Command::PrevTab) => self.tab = wrap(Tab::ALL, self.tab, -1),
            (Screen::Offline, Command::NextTab) => self.tab = wrap(Tab::ALL, self.tab, 1),
        }
        None
    }

    /// El mouse pasó por encima: queda marcado, como con las flechas. Con el panel
    /// abierto, el menú principal no responde.
    pub fn point(&mut self, target: Target) {
        if self.loading {
            return;
        }
        let tab = self.tab.index();
        match (self.screen, target) {
            (Screen::Main, Target::Main(option)) => self.main = option,
            (Screen::Offline, Target::Item(i)) if i < self.items(self.tab) => self.cursors[tab] = i,
            (Screen::Offline, Target::Start) if self.can_start() => {
                self.cursors[tab] = self.items(self.tab)
            }
            _ => {}
        }
    }

    /// Un click: marca y acepta.
    pub fn click(&mut self, target: Target) -> Option<MenuAction> {
        if self.loading {
            return None;
        }
        match (self.screen, target) {
            (Screen::Main, Target::Main(option)) => {
                self.main = option;
                self.activate(option)
            }
            (Screen::Offline, Target::Tab(tab)) => {
                self.tab = tab;
                None
            }
            (Screen::Offline, Target::Back) => {
                self.screen = Screen::Main;
                None
            }
            (Screen::Offline, Target::Item(i)) if i < self.items(self.tab) => {
                self.point(target);
                self.accept()
            }
            (Screen::Offline, Target::Start) if self.can_start() => {
                self.point(target);
                self.accept()
            }
            _ => None,
        }
    }

    /// Flechas de una fila de Configuración de sala, como ← y →.
    pub fn nudge(&mut self, item: usize, delta: i32) {
        if self.screen == Screen::Offline && self.tab == Tab::Settings {
            self.point(Target::Item(item));
            self.adjust(delta);
        }
    }

    /// Al volver de una carrera: la sala, con todos esperando.
    pub fn back_from_race(&mut self) {
        self.screen = Screen::Offline;
        self.tab = Tab::Room;
        self.loading = false;
        for player in &mut self.players {
            player.ready = false;
        }
    }

    /// La carrera no cargó: el menú vuelve a responder.
    pub fn cancel_loading(&mut self) {
        self.loading = false;
    }

    fn activate(&mut self, option: MainOption) -> Option<MenuAction> {
        match option {
            MainOption::Offline => self.screen = Screen::Offline,
            MainOption::Quit => return Some(MenuAction::Quit),
            // Llegan más adelante.
            MainOption::Multiplayer | MainOption::Profile | MainOption::Options => {}
        }
        None
    }

    fn move_cursor(&mut self, delta: i32) {
        let Some(focus) = self.focus() else { return };
        let items = self.items(self.tab);
        let last = items + usize::from(self.can_start()) - 1;
        let cursor = match focus {
            Target::Item(i) => i,
            _ => items,
        };
        self.cursors[self.tab.index()] = cursor.saturating_add_signed(delta as isize).min(last);
    }

    fn accept(&mut self) -> Option<MenuAction> {
        match (self.tab, self.focus()?) {
            (_, Target::Start) => return self.start(),
            (Tab::Room, Target::Item(i)) => match RoomItem::ALL[i] {
                // Elegir auto llega más adelante.
                RoomItem::ChangeCar => {}
                RoomItem::Ready => {
                    if let Some(slot) = self.keyboard() {
                        self.players[slot].ready = !self.players[slot].ready;
                    }
                }
            },
            (Tab::Track, Target::Item(i)) => self.selected_track = Some(i),
            (Tab::Settings, Target::Item(i)) => match Setting::ALL[i] {
                Setting::Bots => self.settings.bots = !self.settings.bots,
                Setting::Powerups => self.settings.powerups = !self.settings.powerups,
                Setting::BotCount | Setting::Laps => {}
            },
            _ => {}
        }
        None
    }

    fn adjust(&mut self, delta: i32) {
        let Some(Target::Item(i)) = self.focus() else {
            return;
        };
        if self.tab != Tab::Settings {
            return;
        }
        match Setting::ALL[i] {
            Setting::BotCount if self.settings.bots => {
                self.settings.bot_count = step(self.settings.bot_count, delta, self.max_bots());
            }
            Setting::Laps => self.settings.laps = step(self.settings.laps, delta, MAX_LAPS),
            _ => {}
        }
    }

    fn start(&mut self) -> Option<MenuAction> {
        if !self.can_start() {
            return None;
        }
        let track = self.tracks[self.selected_track?].id.clone();
        let cars = self
            .players
            .iter()
            .filter(|player| player.device.is_some())
            .map(|player| player.car_id.clone())
            .collect();
        self.loading = true;
        Some(MenuAction::StartRace { track, cars })
    }
}

/// El vecino de `current` en `all`, dando la vuelta.
fn wrap<T: Copy + PartialEq, const N: usize>(all: [T; N], current: T, delta: i32) -> T {
    let index = all.iter().position(|item| *item == current).unwrap_or(0) as i32;
    all[(index + delta).rem_euclid(N as i32) as usize]
}

/// Sube o baja un valor entre 1 y `max`.
fn step(value: u32, delta: i32, max: u32) -> u32 {
    value.saturating_add_signed(delta).clamp(1, max.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu() -> MenuState {
        let tracks = vec![TrackEntry::new("nhood1", "Toys in the Hood 1", None)];
        MenuState::new("Jugador 1", "phim_calcure", "Calcure", tracks)
    }

    fn press(menu: &mut MenuState, commands: &[Command]) -> Option<MenuAction> {
        let mut action = None;
        for &command in commands {
            if let Some(done) = menu.command(command) {
                action = Some(done);
            }
        }
        action
    }

    #[test]
    fn main_menu_wraps_and_quit_exits() {
        let mut menu = menu();
        assert_eq!(menu.main, MainOption::Offline);
        menu.command(Command::Up);
        assert_eq!(menu.main, MainOption::Quit);
        assert_eq!(menu.command(Command::Accept), Some(MenuAction::Quit));
        menu.command(Command::Down);
        assert_eq!(menu.main, MainOption::Offline);
    }

    #[test]
    fn multiplayer_profile_and_options_do_nothing() {
        let mut menu = menu();
        for option in [
            MainOption::Multiplayer,
            MainOption::Profile,
            MainOption::Options,
        ] {
            assert_eq!(menu.click(Target::Main(option)), None);
            assert_eq!((menu.screen, menu.main), (Screen::Main, option));
        }
    }

    #[test]
    fn offline_opens_the_room_and_escape_goes_back() {
        let mut menu = menu();
        menu.command(Command::Accept);
        assert_eq!((menu.screen, menu.tab), (Screen::Offline, Tab::Room));
        // Con el panel abierto, el menú principal no responde al mouse.
        menu.point(Target::Main(MainOption::Quit));
        assert_eq!(menu.click(Target::Main(MainOption::Quit)), None);
        menu.command(Command::Back);
        assert_eq!(
            (menu.screen, menu.main),
            (Screen::Main, MainOption::Offline)
        );
    }

    #[test]
    fn only_the_keyboard_player_is_connected() {
        let menu = menu();
        assert_eq!(menu.players.len(), LOCAL_SLOTS);
        assert_eq!(menu.players[0].device, Some(Device::Keyboard));
        assert_eq!(menu.players[0].car_name, "Calcure");
        assert!(menu.players[1..]
            .iter()
            .all(|player| player.device.is_none()));
        // Los puestos libres no tienen opciones: solo las de la tarjeta del teclado.
        assert_eq!(menu.items(Tab::Room), RoomItem::ALL.len());
    }

    #[test]
    fn start_needs_everyone_ready_and_a_track() {
        let mut menu = menu();
        press(
            &mut menu,
            &[Command::Accept, Command::Down, Command::Accept],
        );
        assert!(menu.players[0].ready, "Estoy listo es la segunda opción");
        assert!(!menu.can_start(), "falta la pista");
        press(&mut menu, &[Command::NextTab, Command::Accept]);
        assert_eq!(menu.selected_track, Some(0));
        assert!(menu.can_start());

        // Iniciar Carrera queda debajo de la última opción.
        let action = press(&mut menu, &[Command::Down, Command::Accept]);
        let expected = MenuAction::StartRace {
            track: "nhood1".into(),
            cars: vec!["phim_calcure".into()],
        };
        assert_eq!(action, Some(expected));
        assert!(menu.loading);
        assert_eq!(menu.command(Command::Back), None);
        assert_eq!(menu.screen, Screen::Offline, "mientras carga no responde");
    }

    #[test]
    fn ready_goes_back_to_waiting_and_hides_start() {
        let mut menu = menu();
        menu.selected_track = Some(0);
        press(
            &mut menu,
            &[Command::Accept, Command::Down, Command::Accept],
        );
        assert!(menu.can_start());
        press(&mut menu, &[Command::Down]);
        assert_eq!(menu.focus(), Some(Target::Start));
        // El mouse vuelve a Listo y lo toca: la marca no puede quedar en un botón oculto.
        menu.click(Target::Item(1));
        assert!(!menu.players[0].ready);
        assert!(!menu.can_start());
        assert_eq!(menu.focus(), Some(Target::Item(1)));
        assert_eq!(menu.click(Target::Start), None);
        assert!(
            !menu.players[0].ready,
            "un Iniciar Carrera oculto no toca nada"
        );
    }

    #[test]
    fn tabs_cycle_both_ways() {
        let mut menu = menu();
        menu.command(Command::Accept);
        menu.command(Command::PrevTab);
        assert_eq!(menu.tab, Tab::Settings);
        menu.command(Command::NextTab);
        assert_eq!(menu.tab, Tab::Room);
        menu.click(Target::Tab(Tab::Track));
        assert_eq!(menu.tab, Tab::Track);
        assert_eq!(menu.previewed_track(), Some(0));
    }

    #[test]
    fn settings_toggle_and_stay_in_range() {
        let mut menu = menu();
        press(&mut menu, &[Command::Accept, Command::PrevTab]);
        press(&mut menu, &[Command::Down, Command::Right]);
        assert_eq!(menu.settings.bot_count, 8);
        // Sin bots, la cantidad no se toca.
        press(
            &mut menu,
            &[Command::Up, Command::Accept, Command::Down, Command::Right],
        );
        assert!(!menu.settings.bots);
        assert_eq!(menu.settings.bot_count, 8);
        // Vueltas entre 1 y 20.
        press(&mut menu, &[Command::Down]);
        press(&mut menu, &[Command::Left; 5]);
        assert_eq!(menu.settings.laps, 1);
        press(&mut menu, &[Command::Right; 40]);
        assert_eq!(menu.settings.laps, MAX_LAPS);
        press(&mut menu, &[Command::Down, Command::Accept]);
        assert!(!menu.settings.powerups);
        menu.nudge(2, -1);
        assert_eq!(menu.settings.laps, MAX_LAPS - 1);
    }

    #[test]
    fn a_long_track_list_follows_the_mark() {
        let tracks = (0..20)
            .map(|i| TrackEntry::new(&format!("pista{i}"), &format!("Pista {i}"), None))
            .collect();
        let mut menu = MenuState::new("Jugador 1", "phim_calcure", "Calcure", tracks);
        press(&mut menu, &[Command::Accept, Command::NextTab]);
        assert_eq!(menu.follow_tracks(5), 0);
        press(&mut menu, &[Command::Down; 7]);
        assert_eq!(menu.follow_tracks(5), 3, "la marca queda abajo de todo");
        press(&mut menu, &[Command::Up; 5]);
        assert_eq!(menu.follow_tracks(5), 2, "y arriba de todo al subir");
        // La rueda mueve la marca de a una pista por línea, sin pasar a Iniciar Carrera.
        menu.wheel_tracks(-0.5);
        assert_eq!(menu.focus(), Some(Target::Item(2)));
        menu.wheel_tracks(-2.5);
        assert_eq!(menu.focus(), Some(Target::Item(5)));
        menu.wheel_tracks(40.0);
        assert_eq!(menu.focus(), Some(Target::Item(0)));
        menu.selected_track = Some(0);
        menu.players[0].ready = true;
        menu.wheel_tracks(-100.0);
        assert_eq!(menu.focus(), Some(Target::Item(19)));
        assert_eq!(menu.follow_tracks(5), 15);
    }

    #[test]
    fn back_from_race_returns_to_the_room_waiting() {
        let mut menu = menu();
        menu.selected_track = Some(0);
        press(
            &mut menu,
            &[
                Command::Accept,
                Command::Down,
                Command::Accept,
                Command::Down,
            ],
        );
        assert!(menu.command(Command::Accept).is_some());
        menu.back_from_race();
        assert!(!menu.loading);
        assert_eq!((menu.screen, menu.tab), (Screen::Offline, Tab::Room));
        assert!(!menu.players[0].ready);
        assert_eq!(menu.selected_track, Some(0), "la pista queda elegida");
    }
}
