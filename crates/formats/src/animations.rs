//! Objetos animados de las pistas de RVGL (`custom_animations.txt`, tipo 76 del `.fob`):
//! cada objeto es un esqueleto de hasta 16 huesos, cada uno con su modelo, que se mueve por
//! keyframes. En metros y con los ejes de Revvy.
//!
//! - Los keyframes suman: cada uno traslada y gira sus huesos desde donde quedaron, en
//!   `Time` segundos y con su curva. El ascensor de fair2 sube, espera, baja y espera con
//!   +4600, 0, −4600 y 0.
//! - Un loop sigue desde donde terminó la vuelta anterior; ida y vuelta deshace lo hecho;
//!   una sola vez queda en el último keyframe.
//! - Re-Volt gira al revés que la regla de la mano derecha (multiplica vectores fila), así
//!   que en Revvy el ángulo va negado. Se ve en la soga de wildland: con +8.6° sobre X los
//!   banderines bajan con la soga.
//! - El reloj es el de la carrera. `PreCountdown` no cambia nada: Revvy no tiene cuenta
//!   regresiva.
//! - Los sonidos, chispas y luces de los keyframes se leen y todavía no se usan, y tampoco
//!   los triggers: una animación que espera uno queda quieta.

use std::collections::BTreeMap;

use glam::{Mat4, Quat, Vec3};

use crate::axes;
use crate::mesh::VisualMesh;

/// Los objetos animados de una pista.
#[derive(Clone, Debug, Default)]
pub struct TrackAnimations {
    /// Las mallas de cada modelo que usan los huesos, con las páginas de la pista.
    pub models: Vec<Vec<VisualMesh>>,
    pub animations: Vec<Animation>,
    pub objects: Vec<AnimatedObject>,
}

impl TrackAnimations {
    /// Los huesos con modelo de todos los objetos: lo más que puede devolver `instances`.
    pub fn bone_count(&self) -> usize {
        self.objects
            .iter()
            .filter_map(|object| self.animations.get(object.animation))
            .map(|animation| {
                animation
                    .bones
                    .iter()
                    .filter(|bone| bone.model.is_some())
                    .count()
            })
            .sum()
    }

    /// Cada hueso que se ve a los `time` segundos de la largada: su modelo (índice en
    /// `models`) y su matriz en el mundo.
    pub fn instances(&self, time: f32) -> Vec<(usize, Mat4)> {
        let mut out = Vec::with_capacity(self.bone_count());
        for object in &self.objects {
            let Some(animation) = self.animations.get(object.animation) else {
                continue;
            };
            let base = Mat4::from_rotation_translation(object.rot, object.pos);
            let pose = animation.pose(time - object.delay);
            for (bone, (matrix, visible)) in animation.bones.iter().zip(pose) {
                if let (Some(model), true) = (bone.model, visible) {
                    out.push((model, base * matrix));
                }
            }
        }
        out
    }
}

/// Un objeto animado de la pista.
#[derive(Clone, Debug)]
pub struct AnimatedObject {
    /// Índice en `TrackAnimations::animations`.
    pub animation: usize,
    pub pos: Vec3,
    pub rot: Quat,
    /// Arranca esta cantidad de segundos después de la largada.
    pub delay: f32,
}

#[derive(Clone, Debug)]
pub struct Animation {
    /// El hueso 0 es el cuerpo principal; cada hueso va después de su padre.
    pub bones: Vec<Bone>,
    pub keyframes: Vec<Keyframe>,
    pub mode: AnimationMode,
    /// Espera un trigger para arrancar; hasta entonces queda en reposo.
    pub needs_trigger: bool,
}

#[derive(Clone, Debug)]
pub struct Bone {
    pub parent: Option<usize>,
    /// Índice en `TrackAnimations::models`. `None`: no se dibuja.
    pub model: Option<usize>,
    /// Pose de reposo respecto del padre (sin padre, respecto del objeto).
    pub offset: Vec3,
    pub offset_rotation: Quat,
}

#[derive(Clone, Debug)]
pub struct Keyframe {
    /// Segundos desde el keyframe anterior.
    pub time: f32,
    pub easing: Easing,
    pub moves: Vec<BoneMove>,
}

/// Lo que un keyframe le hace a un hueso, sumado a donde quedó.
#[derive(Clone, Debug)]
pub struct BoneMove {
    pub bone: usize,
    /// En los ejes de reposo del hueso (m).
    pub translation: Vec3,
    /// Eje unitario y ángulo (rad), en los ejes de reposo del hueso.
    pub rotation: Option<(Vec3, f32)>,
    /// Aparece o desaparece desde este keyframe.
    pub visible: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationMode {
    /// Vuelve a empezar desde donde terminó.
    Loop,
    /// Una sola vez: queda en el último keyframe.
    Once,
    /// Ida y vuelta.
    PingPong,
}

/// Cómo avanza el movimiento de un keyframe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    Linear,
    /// Arranca suave y termina de golpe, como algo que cae.
    SmoothStart,
    /// Arranca de golpe y termina suave.
    SmoothEnd,
    Smooth,
    /// Se pasa un poco y vuelve.
    Overshoot,
}

impl Easing {
    /// Qué parte del movimiento hizo en `u` (0 a 1) del tiempo del keyframe.
    pub fn apply(self, u: f32) -> f32 {
        let u = u.clamp(0.0, 1.0);
        match self {
            Self::Linear => u,
            Self::SmoothStart => u * u,
            Self::SmoothEnd => 1.0 - (1.0 - u) * (1.0 - u),
            Self::Smooth => u * u * (3.0 - 2.0 * u),
            Self::Overshoot => {
                // `easeOutBack`.
                let c = 1.701_58;
                let v = u - 1.0;
                1.0 + (c + 1.0) * v * v * v + c * v * v
            }
        }
    }
}

/// Traslación, giro y visibilidad de cada hueso respecto de su pose de reposo.
struct State {
    translation: Vec<Vec3>,
    rotation: Vec<Quat>,
    visible: Vec<bool>,
}

impl State {
    fn rest(bones: usize) -> Self {
        Self {
            translation: vec![Vec3::ZERO; bones],
            rotation: vec![Quat::IDENTITY; bones],
            visible: vec![true; bones],
        }
    }

    /// `laps` vueltas seguidas, si una desde el reposo deja esto: los giros y las
    /// traslaciones se repiten, y la visibilidad queda como al final de una.
    fn repeated(self, laps: f32) -> Self {
        Self {
            translation: self.translation.into_iter().map(|t| t * laps).collect(),
            rotation: self
                .rotation
                .into_iter()
                .map(|q| {
                    let (axis, angle) = q.to_axis_angle();
                    Quat::from_axis_angle(axis, angle * laps)
                })
                .collect(),
            visible: self.visible,
        }
    }
}

impl Animation {
    /// Lo que dura una vuelta.
    pub fn cycle(&self) -> f32 {
        self.keyframes
            .iter()
            .map(|keyframe| keyframe.time.max(0.0))
            .sum()
    }

    /// Un hueso que no se mueve nunca: ningún keyframe lo traslada ni lo gira, ni a él ni a
    /// sus padres.
    pub fn is_still(&self, bone: usize) -> bool {
        let mut current = Some(bone);
        while let Some(index) = current {
            let moves = self.moves().any(|step| {
                step.bone == index
                    && (step.translation != Vec3::ZERO
                        || step.rotation.is_some_and(|(_, angle)| angle != 0.0))
            });
            if moves {
                return false;
            }
            current = self
                .bones
                .get(index)
                .and_then(|bone| bone.parent)
                .filter(|&parent| parent < index);
        }
        true
    }

    /// Un hueso que ningún keyframe esconde.
    pub fn never_hidden(&self, bone: usize) -> bool {
        !self
            .moves()
            .any(|step| step.bone == bone && step.visible == Some(false))
    }

    fn moves(&self) -> impl Iterator<Item = &BoneMove> {
        self.keyframes.iter().flat_map(|keyframe| &keyframe.moves)
    }

    /// Cada hueso a los `time` segundos de arrancar, respecto del objeto, y si se ve.
    pub fn pose(&self, time: f32) -> Vec<(Mat4, bool)> {
        let mut state = State::rest(self.bones.len());
        if !self.needs_trigger && time > 0.0 {
            let cycle = self.cycle();
            match self.mode {
                AnimationMode::Loop if cycle > 0.0 => {
                    let laps = (time / cycle).floor();
                    if laps >= 1.0 {
                        let mut lap = State::rest(self.bones.len());
                        self.play(&mut lap, f32::INFINITY);
                        state = lap.repeated(laps);
                    }
                    self.play(&mut state, (time - laps * cycle).clamp(0.0, cycle));
                }
                AnimationMode::PingPong if cycle > 0.0 => {
                    let t = time % (2.0 * cycle);
                    self.play(&mut state, if t > cycle { 2.0 * cycle - t } else { t });
                }
                // Una sola vez, o un loop que no dura nada: queda en el último keyframe.
                _ => self.play(&mut state, time),
            }
        }
        let mut out: Vec<(Mat4, bool)> = Vec::with_capacity(self.bones.len());
        for (i, bone) in self.bones.iter().enumerate() {
            let local = Mat4::from_rotation_translation(bone.offset_rotation, bone.offset)
                * Mat4::from_rotation_translation(state.rotation[i], state.translation[i]);
            let matrix = match bone.parent.filter(|&parent| parent < i) {
                Some(parent) => out[parent].0 * local,
                None => local,
            };
            out.push((matrix, state.visible[i]));
        }
        out
    }

    /// Los keyframes de una vuelta hasta `t` segundos, sobre `state`. Cada uno arranca
    /// cuando termina el anterior; los que duran 0 pasan de una.
    fn play(&self, state: &mut State, t: f32) {
        let mut start = 0.0;
        for keyframe in &self.keyframes {
            let duration = keyframe.time.max(0.0);
            if t < start || (t == start && duration > 0.0) {
                break;
            }
            let done = if t >= start + duration {
                1.0
            } else {
                keyframe.easing.apply((t - start) / duration)
            };
            for step in &keyframe.moves {
                let bone = step.bone;
                if bone >= state.visible.len() {
                    continue;
                }
                state.translation[bone] += step.translation * done;
                if let Some((axis, angle)) = step.rotation {
                    state.rotation[bone] =
                        Quat::from_axis_angle(axis, angle * done) * state.rotation[bone];
                }
                if let Some(visible) = step.visible {
                    state.visible[bone] = visible;
                }
            }
            start += duration;
        }
    }
}

/// `custom_animations.txt` leído. Los huesos todavía tienen el id de `MODEL` del archivo:
/// al cargar los modelos pasa a ser su índice en `TrackAnimations::models`.
#[derive(Debug, Default)]
pub(crate) struct AnimationFile {
    /// `MODEL id "nombre"`: el modelo, sin extensión.
    pub models: BTreeMap<usize, String>,
    /// Por `Slot`, que es lo que elige el objeto del `.fob` con `flags[0]`.
    pub animations: BTreeMap<i32, Animation>,
    /// Huesos de keyframes con sonido, chispas o luz, que todavía no se usan.
    pub effects: usize,
}

pub(crate) fn parse(text: &str) -> AnimationFile {
    let root = blocks(text);
    let mut file = AnimationFile::default();
    for (_, values) in root.keys.iter().filter(|(key, _)| key == "model") {
        match (values.first().and_then(|id| id.parse().ok()), values.get(1)) {
            (Some(id), Some(name)) => {
                file.models.insert(id, name.clone());
            }
            _ => tracing::warn!(?values, "MODEL ilegible en custom_animations.txt"),
        }
    }
    for block in root.blocks("ANIMATION") {
        let Some(slot) = block.number("slot") else {
            tracing::warn!("animación sin Slot en custom_animations.txt: se saltea");
            continue;
        };
        match animation(block, &mut file.effects) {
            Some(animation) => {
                file.animations.insert(slot as i32, animation);
            }
            None => tracing::warn!(slot, "animación sin huesos: se saltea"),
        }
    }
    file
}

fn animation(block: &Block, effects: &mut usize) -> Option<Animation> {
    let mut found: Vec<(usize, &Block)> = block
        .blocks("BONE")
        .filter_map(|bone| Some((file_id(bone.number("boneid")?)?, bone)))
        .collect();
    found.sort_by_key(|&(id, _)| id);
    found.dedup_by_key(|&mut (id, _)| id);
    if found.is_empty() {
        return None;
    }
    // Id del archivo → índice. El padre tiene que tener un id menor.
    let index: BTreeMap<usize, usize> = found
        .iter()
        .enumerate()
        .map(|(i, &(id, _))| (id, i))
        .collect();
    let bones = found
        .iter()
        .map(|&(bone_id, bone)| Bone {
            parent: bone
                .number("parent")
                .and_then(file_id)
                .filter(|&parent| parent < bone_id)
                .and_then(|parent| index.get(&parent).copied()),
            model: bone.number("modelid").and_then(file_id),
            offset: axes::position(bone.vec3("offsettranslation").unwrap_or_default()),
            offset_rotation: rotation(bone, "offsetrotationaxis", "offsetrotationamount")
                .map_or(Quat::IDENTITY, |(axis, angle)| {
                    Quat::from_axis_angle(axis, angle)
                }),
        })
        .collect();

    // Por `FrameNr`; los que no lo dicen van al final, en el orden del archivo.
    let mut frames: Vec<&Block> = block.blocks("KEYFRAME").collect();
    frames.sort_by(|a, b| {
        let frame = |block: &Block| block.number("framenr").unwrap_or(f32::MAX);
        frame(a).total_cmp(&frame(b))
    });
    let mut keyframes = Vec::with_capacity(frames.len());
    for frame in frames {
        let mut moves = Vec::new();
        for step in frame.blocks("BONE") {
            let Some(&bone) = step
                .number("boneid")
                .and_then(file_id)
                .and_then(|bone| index.get(&bone))
            else {
                continue;
            };
            if has_effects(step) {
                *effects += 1;
            }
            moves.push(BoneMove {
                bone,
                translation: axes::position(step.vec3("translation").unwrap_or_default()),
                rotation: rotation(step, "rotationaxis", "rotationamount"),
                visible: step.flag("visible"),
            });
        }
        keyframes.push(Keyframe {
            time: frame.number("time").unwrap_or(0.0).max(0.0),
            easing: match frame.number("type").map(|kind| kind as i32) {
                Some(1) => Easing::SmoothStart,
                Some(2) => Easing::SmoothEnd,
                Some(3) => Easing::Smooth,
                Some(4) => Easing::Overshoot,
                _ => Easing::Linear,
            },
            moves,
        });
    }

    Some(Animation {
        bones,
        keyframes,
        mode: match block.number("mode").map(|mode| mode as i32) {
            Some(1) => AnimationMode::Once,
            Some(2) => AnimationMode::PingPong,
            _ => AnimationMode::Loop,
        },
        needs_trigger: block.flag("needstrigger").unwrap_or(false),
    })
}

/// Un id del archivo: los negativos (`ModelID −1`) son ninguno.
fn file_id(value: f32) -> Option<usize> {
    (value >= 0.0).then_some(value as usize)
}

/// Eje y ángulo en grados de Re-Volt → eje y ángulo en radianes de Revvy, con el ángulo
/// negado. Sin eje, el de la muestra de RVGL: hacia arriba.
fn rotation(block: &Block, axis: &str, amount: &str) -> Option<(Vec3, f32)> {
    let degrees = block.number(amount).filter(|&degrees| degrees != 0.0)?;
    let axis = axes::direction(block.vec3(axis).unwrap_or([0.0, -1.0, 0.0])).try_normalize()?;
    Some((axis, -degrees.to_radians()))
}

/// Un sonido (`SfxID`), chispas o una luz (`Type`) que no sea −1.
fn has_effects(step: &Block) -> bool {
    step.blocks.iter().any(|effect| match effect.name.as_str() {
        "SFX" => effect.number("sfxid").is_some_and(|sfx| sfx >= 0.0),
        "SPARK" | "LIGHT" => effect.number("type").is_some_and(|kind| kind >= 0.0),
        _ => false,
    })
}

/// Un bloque `NOMBRE { … }`: sus claves con sus valores y los bloques de adentro. La raíz
/// no tiene nombre.
#[derive(Debug, Default)]
struct Block {
    name: String,
    keys: Vec<(String, Vec<String>)>,
    blocks: Vec<Block>,
}

impl Block {
    /// Los valores de la clave (sin mayúsculas). Si se repite, vale la última.
    fn values(&self, key: &str) -> Option<&[String]> {
        self.keys
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, values)| values.as_slice())
    }

    fn number(&self, key: &str) -> Option<f32> {
        self.values(key)?.first()?.parse().ok()
    }

    fn vec3(&self, key: &str) -> Option<[f32; 3]> {
        match self.values(key)? {
            [x, y, z, ..] => Some([x.parse().ok()?, y.parse().ok()?, z.parse().ok()?]),
            _ => None,
        }
    }

    fn flag(&self, key: &str) -> Option<bool> {
        match self.values(key)?.first()?.to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        }
    }

    /// Los bloques de adentro con ese nombre (en mayúsculas).
    fn blocks<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Block> {
        self.blocks.iter().filter(move |block| block.name == name)
    }
}

enum Token {
    Open,
    Close,
    Word(String),
}

/// Palabras, textos entre comillas y llaves de una línea, sin el comentario (`;`).
fn tokens(line: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ';' => break,
            '{' => out.push(Token::Open),
            '}' => out.push(Token::Close),
            '"' => out.push(Token::Word(
                chars.by_ref().take_while(|&c| c != '"').collect(),
            )),
            c if c.is_whitespace() => {}
            c => {
                let mut word = c.to_string();
                while let Some(c) =
                    chars.next_if(|&c| !c.is_whitespace() && !matches!(c, ';' | '"' | '{' | '}'))
                {
                    word.push(c);
                }
                out.push(Token::Word(word));
            }
        }
    }
    out
}

/// El archivo como árbol de bloques. Una clave toma los valores que la siguen en su línea;
/// un bloque se abre con `NOMBRE {`, o con la llave sola en la línea de abajo.
fn blocks(text: &str) -> Block {
    let mut stack = vec![Block::default()];
    for line in text.lines() {
        let mut tokens = tokens(line).into_iter().peekable();
        while let Some(token) = tokens.next() {
            match token {
                Token::Open => {
                    let parent = stack.last_mut().expect("siempre está la raíz");
                    let name = parent.keys.pop().map(|(key, _)| key).unwrap_or_default();
                    stack.push(Block {
                        name: name.to_ascii_uppercase(),
                        ..Block::default()
                    });
                }
                Token::Close => close(&mut stack),
                Token::Word(word) => {
                    if tokens
                        .next_if(|token| matches!(token, Token::Open))
                        .is_some()
                    {
                        stack.push(Block {
                            name: word.to_ascii_uppercase(),
                            ..Block::default()
                        });
                        continue;
                    }
                    let mut values = Vec::new();
                    while let Some(Token::Word(value)) =
                        tokens.next_if(|token| matches!(token, Token::Word(_)))
                    {
                        values.push(value);
                    }
                    let block = stack.last_mut().expect("siempre está la raíz");
                    block.keys.push((word.to_ascii_lowercase(), values));
                }
            }
        }
    }
    while stack.len() > 1 {
        close(&mut stack);
    }
    stack.pop().unwrap_or_default()
}

/// Cierra el bloque abierto y lo guarda en su padre. En la raíz, una llave de más no hace
/// nada.
fn close(stack: &mut Vec<Block>) {
    if stack.len() > 1 {
        if let Some(block) = stack.pop() {
            stack
                .last_mut()
                .expect("siempre está la raíz")
                .blocks
                .push(block);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::*;

    fn bone() -> Bone {
        Bone {
            parent: None,
            model: Some(0),
            offset: Vec3::ZERO,
            offset_rotation: Quat::IDENTITY,
        }
    }

    fn lift() -> Animation {
        let up = |dy: f32| Keyframe {
            time: 1.0,
            easing: Easing::Linear,
            moves: vec![BoneMove {
                bone: 0,
                translation: Vec3::new(0.0, dy, 0.0),
                rotation: None,
                visible: None,
            }],
        };
        Animation {
            bones: vec![bone()],
            keyframes: vec![up(10.0), up(0.0), up(-10.0), up(0.0)],
            mode: AnimationMode::Loop,
            needs_trigger: false,
        }
    }

    fn height(animation: &Animation, time: f32) -> f32 {
        animation.pose(time)[0].0.w_axis.y
    }

    #[test]
    fn keyframes_add_up_like_the_fair_lift() {
        let lift = lift();
        assert_eq!(lift.cycle(), 4.0);
        assert!((height(&lift, 0.5) - 5.0).abs() < 1e-5);
        assert!((height(&lift, 1.5) - 10.0).abs() < 1e-5, "espera arriba");
        assert!((height(&lift, 2.5) - 5.0).abs() < 1e-5);
        assert!(height(&lift, 3.5).abs() < 1e-5, "espera abajo");
        assert!((height(&lift, 4.5) - 5.0).abs() < 1e-5, "otra vuelta");
        assert!(!lift.is_still(0));
    }

    #[test]
    fn a_loop_goes_on_from_where_it_ended() {
        // Un molino: 90° por vuelta, siempre para el mismo lado.
        let mill = Animation {
            bones: vec![bone()],
            keyframes: vec![Keyframe {
                time: 1.0,
                easing: Easing::Linear,
                moves: vec![BoneMove {
                    bone: 0,
                    translation: Vec3::ZERO,
                    rotation: Some((Vec3::Y, FRAC_PI_2)),
                    visible: None,
                }],
            }],
            mode: AnimationMode::Loop,
            needs_trigger: false,
        };
        let blade = |time: f32| mill.pose(time)[0].0.transform_vector3(Vec3::X);
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            (blade(1.5) - Vec3::new(-half, 0.0, -half)).length() < 1e-5,
            "135°"
        );
        assert!((blade(2.0) - Vec3::NEG_X).length() < 1e-5, "180°");
        assert!((blade(4.0) - Vec3::X).length() < 1e-4, "una vuelta entera");
    }

    #[test]
    fn modes_stop_come_back_or_wait() {
        let mut once = lift();
        once.keyframes.truncate(1);
        once.mode = AnimationMode::Once;
        assert!((height(&once, 9.0) - 10.0).abs() < 1e-5, "queda arriba");
        let mut ping = lift();
        ping.keyframes.truncate(1);
        ping.mode = AnimationMode::PingPong;
        assert!((height(&ping, 1.5) - 5.0).abs() < 1e-5, "de vuelta");
        assert!(height(&ping, 2.0).abs() < 1e-5, "volvió");
        let mut waiting = lift();
        waiting.needs_trigger = true;
        assert_eq!(height(&waiting, 0.5), 0.0);
    }

    #[test]
    fn children_follow_their_parent() {
        let mut arm = lift();
        arm.bones.push(Bone {
            parent: Some(0),
            model: Some(1),
            offset: Vec3::new(1.0, 0.0, 0.0),
            offset_rotation: Quat::IDENTITY,
        });
        arm.keyframes[0].moves.push(BoneMove {
            bone: 1,
            translation: Vec3::ZERO,
            rotation: Some((Vec3::Y, FRAC_PI_2)),
            visible: Some(false),
        });
        let pose = arm.pose(1.0);
        let tip = pose[1].0.transform_point3(Vec3::new(1.0, 0.0, 0.0));
        // Sube con el padre y gira 90° sobre Y: la punta queda en −Z.
        assert!((tip - Vec3::new(1.0, 10.0, -1.0)).length() < 1e-5, "{tip}");
        assert!(!pose[1].1, "desde el keyframe que lo esconde");
        assert!(!arm.is_still(1) && arm.never_hidden(0) && !arm.never_hidden(1));
    }

    #[test]
    fn objects_wait_their_delay() {
        let track = TrackAnimations {
            models: vec![Vec::new()],
            animations: vec![lift()],
            objects: vec![AnimatedObject {
                animation: 0,
                pos: Vec3::X,
                rot: Quat::IDENTITY,
                delay: 1.0,
            }],
        };
        assert_eq!(track.bone_count(), 1);
        let at = |time: f32| track.instances(time)[0].1.w_axis.truncate();
        assert!((at(0.5) - Vec3::X).length() < 1e-6, "todavía no arrancó");
        assert!((at(1.5) - Vec3::new(1.0, 5.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn easing_reaches_the_target() {
        for easing in [
            Easing::Linear,
            Easing::SmoothStart,
            Easing::SmoothEnd,
            Easing::Smooth,
            Easing::Overshoot,
        ] {
            assert!(easing.apply(0.0).abs() < 1e-6, "{easing:?}");
            assert!((easing.apply(1.0) - 1.0).abs() < 1e-5, "{easing:?}");
        }
        assert!(Easing::Overshoot.apply(0.9) > 1.0, "se pasa");
    }

    const SAMPLE: &str = r#"
; Muestra con lo que RVGL permite escribir distinto.
MODEL   0   "pole_1"
MODEL   1   "flag part" ; con espacio
SFX     0   "siren"

ANIMATION {
  Slot                      3
  Mode                      2
  NeedsTrigger              true
  BONE {
    BoneID                  0
    ModelID                 0
  }
  BONE {
    BoneID                  1
    ModelID                 1
    Parent                  0
    OffsetTranslation       0.000 -430.000 0.000
  }
  BONE {
    BoneID                  2
    ModelID                 -1
    Parent                  5
  }
  KEYFRAME {
    FrameNr                 1
    Time                    0.500
    Type                    4
    BONE {
      BoneID                1
      Visible               false
      SFX {
        SfxID               0
        Range               10
        Looping             false
      }
      LIGHT {
        Type                -1
      }
    }
  }
  KEYFRAME
  {
    FrameNr                 0
    Time                    1.000
    Type                    3
    BONE {
      BoneID                1
      Translation           0.000 -100.000 0.000
      RotationAxis          0.000 1.000 0.000
      RotationAmount        20.000
    }
    BONE {
      BoneID                9
      RotationAmount        5.000
    }
  }
}
"#;

    #[test]
    fn the_file_reads_blocks_models_and_keyframes() {
        let file = parse(SAMPLE);
        assert_eq!(file.models.get(&0).map(String::as_str), Some("pole_1"));
        assert_eq!(file.models.get(&1).map(String::as_str), Some("flag part"));
        assert_eq!(file.effects, 1, "el sonido; la luz está apagada");
        let animation = &file.animations[&3];
        assert_eq!(animation.mode, AnimationMode::PingPong);
        assert!(animation.needs_trigger);
        assert_eq!(animation.bones.len(), 3);
        let flag = &animation.bones[1];
        assert_eq!((flag.parent, flag.model), (Some(0), Some(1)));
        assert!(
            (flag.offset - Vec3::new(0.0, 2.15, 0.0)).length() < 1e-6,
            "arriba"
        );
        let orphan = &animation.bones[2];
        assert_eq!(
            (orphan.parent, orphan.model),
            (None, None),
            "padre inválido, sin modelo"
        );

        // Ordenados por `FrameNr`, aunque el 0 esté después y con la llave abajo.
        let [first, second] = animation.keyframes.as_slice() else {
            panic!("dos keyframes: {:?}", animation.keyframes);
        };
        assert_eq!((first.time, first.easing), (1.0, Easing::Smooth));
        assert_eq!(first.moves.len(), 1, "el hueso 9 no existe");
        let step = &first.moves[0];
        assert!((step.translation - Vec3::new(0.0, 0.5, 0.0)).length() < 1e-6);
        let (axis, angle) = step.rotation.unwrap();
        assert!((axis - Vec3::NEG_Y).length() < 1e-6 && (angle + 20f32.to_radians()).abs() < 1e-6);
        assert_eq!(
            (second.easing, second.moves[0].visible),
            (Easing::Overshoot, Some(false))
        );
    }

    #[test]
    fn pennants_hang_along_the_sagging_rope() {
        // La soga de wildland: el primer banderín va 20 unidades adelante, girado +8.6° sobre
        // X, donde la soga baja (la Y de Re-Volt crece con Z). El borde de arriba del
        // banderín (+Z del modelo) tiene que bajar con ella.
        let file = parse(
            "ANIMATION {\n Slot 20\n BONE {\n  BoneID 0\n  ModelID 60\n }\n BONE {\n  BoneID 1\n  \
             ModelID 59\n  Parent 0\n  OffsetTranslation 0.000 1.000 20.000\n  \
             OffsetRotationAxis 1.000 0.000 0.000\n  OffsetRotationAmount 8.600\n }\n}\n",
        );
        let edge = file.animations[&20].pose(0.0)[1]
            .0
            .transform_vector3(Vec3::Z);
        assert!(edge.z > 0.9 && edge.y < -0.1, "{edge}");
    }
}
