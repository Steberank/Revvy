# Revvy — Roadmap

Plan de implementación derivado de `revvy-arquitectura.md`. Cada bloque apunta a la sección de arquitectura que define el contrato. El orden de las fases es el orden de construcción; dentro de una fase, los ítems son el trabajo concreto.

**Qué es Revvy:** un juego propio, retrocompatible con pistas y autos de Re-Volt. **No es un port** (§0). El contenido de Re-Volt pasa por una capa de traducción y corre en el motor de Revvy (render wgpu, física Rapier + vehículo propio, sonido Kira), el mismo que usan las pistas `.glb` y los autos propios. Carrera, reglas, poderes, bots y red se construyen sobre ese motor, nunca sobre estructuras de Re-Volt.

**Fuera de v1** (queda documentado al final, no bloquea el resto): Rhai, visiboxes, cámaras de replay, decimate automático de colisión, descarga de autos custom, migración de host, S3/MinIO, varias instancias de game server, modo reversed.

**Árbitro:** quien decide rayitos, poderes, vueltas y respawn. Si `sim_authority: Server`, el servidor de juego. Si `Client`, el host. El dedicated server siempre existe para lobby, mapas y cierre de sala.

**Rutas:** `levels/`, `cars/`, `wavs/` y `cache/` son relativas a la raíz de contenido: `content/` en el repo, `content_root` en `config/client.toml` (§3).

---

## Fase 0 — Workspace y esqueleto (§1, §2, §3)

Objetivo: un `cargo build` del workspace que compile cliente desktop, server y crates vacíos, con logging y configs.

### 0.1 Núcleo (§1.1)

- [x] `Cargo.toml` workspace: `crates/core`, `formats`, `net`, `physics`, `bots`, `stats`, `client`, `server`.
- [x] Rust edition 2021 (2024 cuando el toolchain del CI lo soporte).
- [x] Dependencias compartidas: `tokio`, `glam`, `serde`, `ron`, `tracing`, `tracing-subscriber`, `anyhow`, `thiserror`.
- [x] `tracing` inicializado en `client/src/main.rs` y `server/src/main.rs`.

### 0.2 Carpetas (§3)

Crear el árbol tal cual la arquitectura, aunque los módulos estén vacíos:

- [x] `crates/core/src/{components,systems,vehicle,rules,powerups}`
- [x] `crates/formats/src` + `crates/formats/schema/layout.ron` (ejemplo canónico, puede empezar stub)
- [x] `crates/net/src/{messages,quic.rs,codec.rs}`
- [x] `crates/physics/src/{vehicle_controller,collision_events,jump,surfaces,force_field}`
- [x] `crates/bots/src/{waypoints,steering,difficulty}`
- [x] `crates/stats/src/{events,race_summary,session_summary}`
- [x] `client/src/{app,render,input,ui,audio,network,assets_pipeline,ecs,editor,platform}`
- [x] `client/assets/` (fuentes, UI, `pickups/bolt.glb`, `cars/placeholder.glb`). No tracks/cars de contenido.
- [x] `content/{levels,cars,wavs,gfx}`: raíz de contenido, con las carpetas hermanas como en la del juego. Antes era `ServerREVOLT/`.
- [x] `server/src/{game,network,api,maps_storage,db,cache,bots}` + `server/migrations/`
- [x] `tools/{map-packager,car-packager,asset-inspector,track-editor}`
- [x] `dcc/{blender,blockbench}/revvy_export.md` (checklist vacío que se llena en fase 8)
- [x] `docs/{protocol.md,formats/,layout-schema.md,gameplay-rules-schema.md}`
- [x] `config/{server.toml,client.toml}` y `config/rules/{default.ron,antigrav_turbo.ron,powerups.default.ron,surfaces.default.ron}`

### 0.3 Cliente mínimo (§1.2)

- [x] Ventana `winit` + surface `wgpu` (clear color, resize).
- [x] Loop con `bevy_ecs` standalone (un `Schedule` por frame, sin Bevy completo).
- [x] `egui` + `egui-wgpu` + `egui-winit`: pantalla vacía “Revvy”.
- [x] `gilrs` estuvo en el esqueleto. El cliente de la fase 2 no lo enlaza.
- [x] Feature `track-editor` en `client/Cargo.toml`, default ON en desktop, ausente en mobile (`#[cfg(feature = "track-editor")]`).

### 0.4 Binario server (§2)

- [x] Un binario con dos routers: Quinn (juego) y Axum (HTTP). v1 en el mismo proceso.
- [x] Axum responde `GET /health`.
- [x] Quinn acepta una conexión y la loguea.

**Hecho cuando:** `cargo build --workspace` en desktop; el cliente abre ventana; el server escucha HTTP y QUIC.

---

## Fase 1 — Formatos legacy y `TrackAsset` (§1.5, §6.4–6.5)

Objetivo: copiar una pista Re-Volt a `levels/<id>/` y obtener meshes + colisión + layout unificado en memoria. Sin convertir archivos.

Referencia de layouts binarios: código y docs de RVGL. Parsers en `crates/formats`.

### 1.1 Trait común

- [x] `Track`, `CarDef`, `TrackAsset` en `formats/src/lib.rs`.
- [x] `TrackAsset { visual, collision }` — el server puede pedir solo `collision`.
- [x] Loader:
  1. Si `track.toml` tiene `format = "revvy-glb-v1"` → pipeline glTF (fase 8; por ahora error “no implementado”).
  2. Si no, `.inf` + `.w` + `.ncp` → pipeline legacy.
- [x] Carpetas mixtas (glb + `.w`) se rechazan.

### 1.2 Parsers de geometría y colisión

- [x] `.w` mundo / instancias (`world.rs`)
- [x] `.prm` mesh (`prm.rs`)
- [x] `.ncp` del mundo y de cada instancia del `.fin` (`ncp.rs`, `fin.rs`). Nombre de 8 letras. Sin lista fija de props.
- [x] `.bmp` texturas (`bmp.rs`) vía crate `image`. En las páginas de pista, el negro puro es color key.
- [x] Conversión de ejes al parse: se niegan X e Y, 5 mm por unidad (hasta la fase 2 se usó 1 cm). Un solo Y-up diestro para física y render. §6.5.

### 1.3 Parsers de layout de carrera → `TrackLayout`

Adaptar a las structs de §7.4 (no exponer el binario crudo al resto del juego):

- [x] `.taz` → `zones` (`taz.rs`)
- [x] `.pan` → `pos_nodes` (`pan.rs`), links `-1` = sin conexión, cap histórico 4 no se impone en el tipo nuevo
- [x] `.fan` → `ai_nodes` (`fan.rs`): layout de `FILE_AINODE`, verde izquierda, rojo derecha, racing line, overtaking, paredes y `AIN_TYPE` → `AiFlags` / `speed_limit`
- [x] `.fob` pickups → `pickups` (`fob.rs`); si hay tablas custom, `pickup_odds` / `PickupSpawn.odds` (lock de host)
- [x] `.fld` → `force_fields` (`fld.rs`)
- [x] `.fin` / start pos → `start_grid`
- [x] Triggers de reposition → `kill_volumes` cuando el tipo es kill
- [x] Superficies del `.ncp` → `SurfaceType` en el collider

### 1.4 Parsers que se leen pero no gobiernan v1

Implementar lo suficiente para no crashear al abrir un level stock; el gameplay puede ignorarlos hasta fases posteriores:

- [x] `.vis`, `.cam`, `.inf` (pista), `.lit` / `.li-`, `.por`, `.pro`
- [x] Notas en `docs/formats/` por archivo, con lo que se confirmó contra RVGL

### 1.5 Autos legacy

- [x] `.prm` body/wheel + `parameters.txt` → `CarDef`
- [x] No reescribir el archivo. Claves desconocidas quedan en el mapa (warning). Lo que falta lo rellena `CAR 0-28` de `CARINFO.TXT`.
- [x] `CARINFO.TXT` de Re-Volt versionado en `crates/formats/revolt/`: un clon nuevo compila sin `rvsource/`. Sus defaults son solo para autos de Re-Volt; `load_car` rechaza una carpeta con `car.toml`.

**Hecho cuando:** un test carga una pista real de `levels/` (p. ej. una con `.w`, `.ncp`, `.fan`, `.pan`, `.taz`) y cuenta meshes, zonas y nodos sin panic. Un auto `.prm`+`.inf` produce un `CarDef`.

---

## Fase 2 — Manejo en un mapa legacy (§1.2, §1.9, §1.10, §6.5)

Objetivo: abrir una pista Re-Volt y manejar un auto encima como en el juego, con su sonido. `cargo run -p revvy-client -- nhood1 phim_calcure` carga nhood1 con el Calcure. Para llegar rápido a ese manejo, 2.5 y 2.6 portaron directo el motor de Re-Volt; 2.7 lo reemplazó por el motor de Revvy (Rapier + vehículo propio) y el port quedó como referencia en los tests.

### 2.1 Render (`client/src/render`)

- [x] Pipeline opaco: posición, normal, UV, textura `.bmp`.
- [x] Cámara libre con **C**: WASD según la mirada, mouse para girar, Q baja, E sube, Shift acelera. Las flechas siguen manejando. Por defecto va la de persecución (2.7).
- [x] Upload de `TrackAsset.visual` (`.w` y `.prm` de instancias). `Collision` no se dibuja.
- [x] Luz = textura × gouraud del archivo. Sin sol ni hemisferio. `.lit` espera.
- [x] Color key de mapa: texel RGB 0 no se dibuja; el resto de la cara sí. Un vértice negro no esconde la cara (techo del túnel).
- [x] Skybox en el orden de `RenderSkybox`, ya pasado a Y-up. §6.5.

### 2.2 Traducción

- [x] `load_track(dir)` y `load_car(dir)` son la capa. `level` y `car` en `config/client.toml`, o los argumentos (`cargo run -p revvy-client -- nhood1 phim_calcure`). Un nombre se busca en `levels/` o `cars/`; una ruta se usa tal cual.
- [x] Cada instancia del `.fin` carga su `.ncp` por nombre, incluidos los recortados a 8 letras. Sin lista de props.
- [x] Escala: una unidad de Re-Volt mide 5 mm (`REVOLT_TO_METERS` = 0.005, un solo factor para `revvy-formats` y la física), según el velocímetro de `units.h`. A esa escala la gravedad de Re-Volt es 11 m/s² y su mph es el real. La cámara libre y los planos de recorte se ajustaron a la escala nueva.
- [x] `TrackAsset.legacy` (`LegacyLevel`) trae los datos sin convertir para la física y el sonido: el `.ncp` con su grilla, el `.ncp` de cada instancia, los objetos del `.fob` y la largada del `.inf`. Hoy solo lo usa el port de referencia en los tests (2.7).

### 2.3 ECS (presente, el manejo todavía no lo usa)

- [x] Componentes: `Transform`, `Velocity`, `PowerupSlot` vacío, `CarId`.
- [x] `drive_schedule` en `revvy-core` copia una pose. El cliente no lo corre: el auto vive en `DriveView`.

### 2.4 Render del auto (`client/src/render`, `load_car`)

El chasis y las ruedas salen de `parameters.txt` (`MODEL`, `TPAGE`) y se dibujan como `DrawCar`, en la pose que deja la física de 2.5.

- [x] Chasis en `Pos + BodyOffset`, con la matriz del cuerpo.
- [x] Una malla por rueda, según el `ModelNum` de su `WHEEL`. La `TPAGE` va sin color key.
- [x] Cada rueda en `WPos`: su `Offset1` más el recorrido de suspensión sobre el eje up del auto, recortado a `MaxPos`. No se despega del buje.
- [x] Matriz de rueda `RotX(AngPos) · RotY(TurnAngle) · W`. En las ruedas `IsTurnable`, `TurnAngle` es el volante × `SteerRatio`. `AngPos` integra la velocidad angular de la física: la rueda gira con la velocidad y también patina o se traba.
- [x] Las ruedas derechas copian la matriz de la izquierda de su eje, como el original.
- [x] Una matriz de modelo por pieza (uniform con offset dinámico): la pista, el chasis y las cuatro ruedas.

### 2.5 Manejo de referencia: port de Re-Volt (`crates/physics/src/revolt`, §1.9)

Fue el motor de la fase 2 hasta 2.7; hoy es la referencia de manejo de los tests. Port de `rvsource/Xbox/Src`, ramas `_PC` y modo Simulación (con un solo auto, igual que Arcade): `newcoll.cpp`, `body.cpp`, `car.cpp`, `wheel.cpp`, `control.cpp`, `move.cpp` y `camera.cpp`. Corre en el espacio de Re-Volt (unidades de 5 mm, Y abajo) con las constantes tal cual; solo la cámara y el dibujo pasan a Revvy. Los números salen de `parameters.txt` encima de `CAR 0-28` en `CARINFO.TXT`; no se hardcodea el Calcure.

- [x] Datos: `CarInfo` tipado (cuerpo, ruedas, resortes, motor) con las unidades del archivo, las esferas del `.hul` y las líneas `;)` de RVGL.
- [x] Mundo: polígonos `NEWCOLLPOLY` del `.ncp` con la grilla del archivo. Las instancias del `.fin` entran con `RotTransPlane`, se reparten con 70 de margen y cada celda se ordena por altura (`InitCollGrid`).
- [x] Motor: `EngineRate` lleva el voltaje; el torque (`EngineVolt × EngineRatio`) se apaga al llegar a `TopSpeed` (`MPH2OGU_SPEED`), como en `CarWheelImpulse2`. Frenar es voltaje negativo más `AxleFriction`.
- [x] Dirección: respuesta cúbica; `SteerRate` ×4 al centrar o invertir y ×0.5 al seguir doblando; `SteerRatio` por rueda. `SteerMod` se lee, pero la versión PC no lo usa.
- [x] Suspensión: `SpringDampedForce` (`Stiffness`, `Damping`), `MaxPos` y el golpe con `Restitution` (negativa: se come el impacto). El chasis no se da vuelta en una rampa chica ni se siente rígido.
- [x] Grip: `StaticFriction`, `KineticFriction` y `Grip` de la rueda, con el cono de fricción del fuente (estático hasta que desliza, después cinético). Las ruedas patinan y derrapan (`WHEEL_SPIN`, `WHEEL_SLIDE`).
- [x] Grip por superficie: los 27 materiales de `COL_MaterialInfo`, con el índice del `.ncp` tal cual: rugosidad, agarre, dureza y corrugado. Hielo, tierra y asfalto no comparten el mismo agarre.
- [x] Cuerpo: esferas del `.hul` resueltas juntas con gradiente conjugado, gravedad 2200, `Resistance`, `AngRes`, `ResMod` y `DownForceMod`. El piso lo aguantan las ruedas; las esferas no lo vuelven resbaloso.
- [x] Frame como `gameloop.cpp`: mandos con el `TimeStep` del frame (tope 10/72 s) y después `1 + TimeStep × 150` pasos de colisión y movimiento.
- [x] Largada: `STARTPOS` y `STARTROT` del `.inf`, más el slot 0 de `CarGridStarts`.
- [x] Cámara de persecución `CAM_FOLLOW_BEHIND`: choca con el mundo y se acorta si pierde de vista al auto. FOV de `GeomPers` 512, unos 50° en vertical.
- [x] **R** endereza con `MOV_RightCar`, solo con el auto dado vuelta.
- [x] Tests con el Calcure en nhood1 (`crates/physics/tests/calcure_nhood1.rs`): se asienta en las cuatro ruedas, acelera hacia adelante con las ruedas girando y dobla hacia el lado pedido.
- [x] `collision_events` y `allow_jump`: el port no los usa. El golpe (`BangMag`) va al sonido; `allow_jump` sigue en falso.

Fuera del port por ahora, sin bloquear esta fase: marcas de derrape, chispas y polvo, antena, env map del chasis, choques entre autos, aceite, speedups y catch-up. Los choques entre autos los resuelve Rapier en 2.7.

### 2.6 Sonido (`client/src/audio`, §1.10)

Kira es la salida. Encima corre el comportamiento de `sfx.cpp` de Re-Volt PC (rama `OLD_AUDIO`). Desde 2.7 trabaja en metros, con el estado del vehículo de Revvy.

- [x] Mezclador: cada sonido 3D tiene volumen 0–127, frecuencia en Hz y posición. Una vez por frame se recalculan la atenuación por distancia y rango, el paneo y el Doppler respecto de la cámara (`GetSfxSettings3D`). Los loops fuera de rango se cortan y vuelven a arrancar al entrar.
- [x] `sfx_volume` en `config/client.toml`: 90, como `SFX_DEFAULT_VOL`. Sin dispositivo de audio, el juego corre mudo y el HUD lo avisa.
- [x] Motor según `Revs`: el `SFXENGINE` del auto (clave de RVGL) o el de su clase. El Calcure es eléctrico y no trae uno propio: suena `wavs/moto.wav`.
- [x] Acelerar sube el volumen y el tono del motor. Frenar fuerte y derrapar suenan con `skid_normal` o `skid_rough`, según el material. Roce: `scrape.wav`. Volante: servo. Golpe (`BangMag` > 500): `hit2.wav`.
- [x] Nivel: nhood1 usa el banco `wavs/hood/` (`SfxLevel`). Suenan los `3DSOUND` del `.fob`, en loop o cada 10–30 s, y los regadores.
- [ ] `basketball.wav` y `roadcone.wav`: esperan a que la pelota y los conos del nivel sean objetos físicos.
- [ ] Música y bocina. Los sonidos de armas van con la fase 5.

### 2.7 Motor de Revvy y capa de traducción (§0, §1.9, §1.10)

El manejo y el sonido de 2.5–2.6 con el flujo correcto: contenido de Re-Volt → traducción → motor de Revvy. Un auto de Re-Volt y uno propio corren juntos en el mismo motor, y lo mismo una pista de Re-Volt y una `.glb`.

- [x] Tipos del motor, en metros, Y arriba y SI: `VehicleParams` (con `ChassisShape`), `CarSound`, `SurfaceType` con su perfil físico, `TrackSounds` (banco y emisores) y la grilla de largada. El cliente solo ve tipos de Revvy; `LegacyLevel` y `CarDef.revolt` los lee solo el port de referencia.
- [x] Superficies: `SurfaceType` cubre los 27 materiales de Re-Volt en el orden de su índice (`from_revolt` uno a uno). `revvy-physics::surfaces` da su perfil con los valores de `COL_MaterialInfo` en metros.
- [x] Traducción de pista: el `.ncp` del mundo y de las instancias → hasta tres `TriMesh` de Rapier (común, solo cámara, solo objetos) con la superficie de cada triángulo. El frente de cada triángulo sale de la normal de su polígono; en una instancia espejada se invierte el orden.
- [x] Grilla de largada entera: los cuatro tipos de `CarGridStarts` (hasta 12 puestos). Corregido el yaw de `STARTROT`, que quedaba mirando para atrás: ahora mira hacia la zona 1, como el juego.
- [x] Traducción de auto: `parameters.txt` + `CARINFO.TXT` → `VehicleParams`, cada valor convertido según su dimensión. `AxleFriction` se separa en dos (freno del eje y `spin_damping`), porque Re-Volt lo usa con dos dimensiones distintas. Las esferas del `.hul` van a la piel del chasis y sus cascos convexos (ahora se leen), a los choques entre autos.
- [x] Física: `rapier3d` de vuelta en `revvy-physics` (`world.rs`), con el vehículo de Revvy encima (`vehicle_controller.rs`): ruedas esfera contra los triángulos cercanos, suspensión, cono de fricción, motor con fade, volante, downforce y resistencias. Paso fijo de 1/180 s, mandos a 60 Hz, gravedad 11 m/s² e interpolación para dibujar.
- [x] Choques entre autos de cualquier origen, resueltos por Rapier como en el modo Simulación: casco contra casco, rueda contra carrocería y rueda contra rueda. Fricción y rebote del chasis contra la pista por superficie, con un hook; *contact clustering* apagado para que cada contacto sea de un triángulo.
- [x] Cámara de persecución de Revvy (`camera.rs`), en metros, contra la pista con la misma prueba de esfera y un rayo para la vista.
- [x] Enderezar (**R**) en el motor de Revvy: el chasis pasa a cinemático, sube y gira hasta quedar derecho. Funciona cada vez que el auto queda dado vuelta (`righting.rs`).
- [x] Sonido en metros: un juego de sonidos por auto con su `VehicleSound`, y los emisores de `TrackSounds`.
- [x] Cliente sin tipos de Re-Volt: `DriveView` usa `PhysicsWorld`, `ChaseCamera` y `VehicleSound`. Varios autos en la grilla (`car` + `extra_cars` en la config, o por argumentos) y **Tab** para cambiar el que se maneja. El render dibuja hasta 32 autos, los de una sala.
- [x] Parecido con el port (`revvy_vs_port.rs`): asentamiento a 1 mm, aceleración a menos de 0,5 mph de 0 a 37 mph, giro a pocos grados. Donde más se separan es en los golpes contra paredes. Probado a mano.
- [x] Contenido propio de prueba (`tools/test-content/generate.py`): el auto `revvy_buggy` (`car.toml` + `body.glb` + `collision.glb`) y la pista `revvy_arena` (`.glb` con asfalto, hielo, pasto, tierra, piedritas, rampas y muros). Adelanta partes de 8.4 y 8.5.
- [x] Convivencia (`revvy_content.rs`): el Calcure y el buggy en nhood1 (el buggy empuja al Calcure), los dos en la arena `.glb`, y el hielo agarra menos que el asfalto.
- [x] El port salió del runtime: el juego no lo usa. Queda como referencia de los tests; se puede borrar cuando ya no haga falta comparar.

**Hecho cuando:** se recorre nhood1 (árboles sin rectángulo negro, cielo cerrado, túnel con techo, primera curva a la derecha); el Calcure se maneja como en Re-Volt, con su velocidad, agarre y suspensión, y ruedas que giran con la velocidad y doblan con el volante; y al acelerar, frenar y derrapar suena como en el juego, igual que los objetos de nhood1. **Cumplido:** primero con el port (2.4–2.6) y después con el motor de Revvy (2.7), probado a mano en nhood1 y en la arena `.glb`, con el Calcure y el buggy propio en la misma pista.

---

## Fase 3 — Carrera local: vueltas, respawn, HUD (§4.7, §7.4, §7.5, §7.12)

Objetivo: la pista legacy ya adaptada a `TrackLayout` gobierna progreso y respawn. Sin editor todavía.

### 3.1 Progreso

- [ ] Cada `TrackZone` es sensor. Zona `0` = meta.
- [ ] `pos_nodes` → `RacePath` (distancia a meta, standings).
- [ ] Una vuelta cuenta al cruzar zona 0 en orden, con el grafo de zonas completo.
- [ ] `GameplayRules.laps` (de `config/rules/default.ron`) decide el corte. `track.toml` laps es metadata de pista; la sala manda.
- [ ] Al terminar: pantalla `Results` corta y vuelta a un estado “lobby local” **con el mapa todavía dibujado de fondo**.

### 3.2 Wrong way y volcar

- [ ] Orden inverso de zonas/POS → popup HUD **«Wrong Way !»** pulsante (`client/src/ui/hud/wrong_way.rs`). No respawnea.
- [ ] Keybind volcar: roll/pitch a 0, XZ se mantiene. Sin contacto usable → respawn corto en el mismo punto. Avance: **R** ya endereza como `MOV_RightCar` si el auto está dado vuelta y toca algo (2.5; en el motor de Revvy, 2.7). Falta el respawn corto.

### 3.3 Respawn (§7.12)

Destino: último `PosNode` válido de la `TrackZone` reciente, auto derecho, i-frames cortos. `PhysicsWorld::place_vehicle` ya pone un auto quieto en una pose (2.7).

- [ ] Kill volume: sensor, al entrar respawn (en legacy, triggers de reposition de tipo kill).
- [ ] Fuera de **todas** las TrackZones más de `off_track_secs` (~1.5 s) → respawn. Las zonas son el volumen jugable.
- [ ] Fuera del AABB mundo (`Collision` + margen) → respawn inmediato.
- [ ] Keybind / botón manual.

**Hecho cuando:** se completan N vueltas, el popup de contramano aparece al ir al revés, caerse del mapa y el botón manual devuelven al último nodo sano.

---

## Fase 4 — Reglas, piso, campos de fuerza, mods de auto (§1.6, §7.8–7.10)

Todavía offline. Los datos pueden venir del layout legacy adaptado o de un `layout.ron` escrito a mano (el editor llega en fase 9).

### 4.1 `GameplayRules`

- [ ] Recurso ECS cargado de RON: `laps`, `allow_jump`, `sim_authority` (ignorado offline), `late_join_mode`, `disconnect_bot_replace`, `reconnect_secs`, vector de turbo, `pickups_enabled`, `pickup_odds: Option<PowerupOdds>`.
- [ ] Preset `config/rules/antigrav_turbo.ron` cambia el vector de turbo sin recompilar.
- [ ] Schema en `docs/gameplay-rules-schema.md`.

### 4.2 Superficies (`physics/surfaces.rs`, §7.9)

- [ ] Enum `SurfaceType`: `Road`, `Dirt`, `Ice`, `Grass`, `Metal`, `Wood`, `Sand`, más los materiales de Re-Volt sin equivalente (2.7). Desconocido → `Road`.
- [ ] `config/rules/surfaces.default.ron`: `friction`, `lateral_grip`, `rolling_resist`, `speed_factor` por tipo (números afinables).
- [ ] Override de pista: `layout.surface_effects` pisa solo las claves presentes.
- [ ] `surface_volumes`: OBB que pisa el material del mesh. Solape: gana el id más alto.
- [ ] Prioridad del tipo en el contacto: volumen → material de colisión → `Road`.
- [ ] Composición con mods: `grip_efectivo = car.grip × mod_en_superficie × surface_effect.lateral_grip`.
- [ ] Defaults de `surfaces.default.ron` alineados con `COL_MaterialInfo` para las superficies que vienen de Re-Volt (2.7), así una pista legacy agarra como en el juego. Los números actuales (hielo con 0.15 de fricción) se pusieron sin esa referencia.

### 4.3 Campos de fuerza (`physics/force_field.rs`, §7.8)

- [ ] `GravityScale(f32)`, `Wind(Vec3)`, `ConstantForce(Vec3)`.
- [ ] Cada tick, overlap auto↔OBB suma aceleración al vehículo, sobre la gravedad del motor (11 m/s², 2.7).
- [ ] En legacy los alimenta `.fld`.

### 4.4 `param_mods` (`core/vehicle/param_mod.rs`, §7.10)

- [ ] `ParamMod { target: All | Car(id), op: Add | Percent | Set, stat, surface: Option<SurfaceType>, value }`.
- [ ] Orden: todas las filas `All`, después `Car(este_id)`, dentro del grupo el orden de la lista.
- [ ] `Add`: `x + value`. `Percent`: `x * (1 + value/100)`. `Set`: reemplaza.
- [ ] Clamp (`Engine >= 0`). Rechazar stat desconocido y `Percent <= -100`.
- [ ] `surface: Some` no se hornea: se aplica en el tick si el contacto resuelto es esa superficie.
- [ ] `Car("no_existe")` es no-op si ese auto no corre.

**Hecho cuando:** un `.fld` o un RON de prueba cambia la gravedad en una caja; hielo (superficie del `.ncp` o un volumen) se siente; un `ParamMod` de `calcure` no altera a los demás.

---

## Fase 5 — Pickups y poderes (§7.7, §7.7.2, §7.11)

### 5.1 Slot

- [ ] `PowerupSlot::Empty | Occupied { kind, charges, extra }`.
- [ ] Un solo slot. No hay timer de inventario salvo `HotPotato` (10 s desde que entra).
- [ ] Rayito: mesh `client/assets/pickups/bolt.glb`, sensor. Slot vacío → sorteo, se oculta, respawn a `respawn_secs`. Slot lleno → ese auto no solapa (lo traspasa); el rayito sigue para otros.
- [ ] Árbitro local en esta fase (el mismo proceso). En red, fase 7.

### 5.2 Odds (3 capas + host)

- [ ] `PowerupKind` cerrado (tabla de abajo). Pesos `u32` relativos, `p_i = w_i / sum`. `w = 0` no sale. Clave ausente en tabla custom = 0 (reemplazo, no merge).
- [ ] `config/rules/powerups.default.ron` = capa 3. Pesos iniciales editables; no hardcodear el sorteo.
- [ ] Capa 2: `layout.pickup_odds`. Capa 1: `PickupSpawn.odds`.
- [ ] Resolución:

```
locked = layout.pickup_odds.is_some() || algún spawn con odds
odds = spawn.odds ?? layout.pickup_odds ?? (si !locked { room.pickup_odds }) ?? defaults
```

- [ ] `validate`: todos `w >= 0` y `sum > 0`.
- [ ] UI de sala `client/src/ui/room/pickup_odds.rs` (puede ser local hasta tener red):
  - Pista libre: sliders ON, muestran `room.pickup_odds` o defaults. **Reset to Default** pone `None` solo si había `Some`.
  - Pista locked: banner *«No se puede modificar las probabilidades en esta pista»*, sliders y Reset OFF, se muestra la tabla de la pista.
  - Cambiar de pista **no** borra `room.pickup_odds`. Al volver a una libre, reaparecen las del host.
  - Rechazar `SetPickupOdds` (incluido reset) si locked, también en el proceso local para no divergir de la regla de red.

### 5.3 Los 11 poderes

Vectores y radios en `powerups.default.ron` (magnitudes afinables). Uso = acción `powerup`.


| Kind            | Entregable                                                                                                                                                                                  |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `WaterBalloon`  | Proyectil, 3 cargas. Impacto en auto = empuje. Impacto en pared = explota y gasta la carga.                                                                                                 |
| `HomingRocket3` | Igual, 3 cargas, persigue al rival más cercano adelante / con línea de visión.                                                                                                              |
| `HomingRocket1` | Igual, 1 carga.                                                                                                                                                                             |
| `OilSlick`      | Decal/volumen en el piso. Quien lo pisa pierde grip mientras solapa. Grace ~0.5 s para el autor.                                                                                            |
| `Electric`      | Pulso de radio R. Otros en el radio no aceleran 4 s.                                                                                                                                        |
| `Shockwave`     | Hitbox grande, empuje siempre hacia arriba, rebota en pared (no explota).                                                                                                                   |
| `Battery`       | +aceleración y +tope 10 s.                                                                                                                                                                  |
| `FakeBolt`      | Deja un rayo falso (mismo mesh). Otro auto: VFX explosión + empuje arriba. No otorga poder. El autor no lo dispara.                                                                         |
| `HotPotato`     | Timer 10 s al recoger. Explota sobre quien la tiene. Se pasa al **tocar** a otro; el receptor hereda el tiempo restante. 3 s de inmunidad al **pase** (no bloquea aceite ni otros poderes). |
| `Star`          | `Electric` a todos los demás, sin radio.                                                                                                                                                    |
| `HeavyBall`     | Cuerpo dinámico, mucha masa, sale con la velocidad del auto. Choca paredes; a alta velocidad rebota.                                                                                        |


- [ ] VFX mínimos (egui/partículas simples) para explosión, electricidad y aceite. Audio posicional por impacto con el mezclador de 2.6.
- [ ] `PowerupUsedEvent` ya tipado en `revvy-stats` (persistencia en fase 10).

**Hecho cuando:** cada poder se puede forzar desde un spawn de prueba; un slot lleno atraviesa el rayito; la papa caliente no se puede devolver durante 3 s; Reset y lock de odds se comportan como el ejemplo de §7.7.1.

---

## Fase 6 — Bots (§1.7, §5)

- [ ] `AiPath` unificado desde `ai_nodes` (legacy ya adaptado o RON). Interpolación `left`/`right`/`racing_t`/`overtaking_t`. Flags: `Racing`, `Slowdown`, `SpeedLimit`, `PickupRoute`, `Careful`, `WallLeft`, `WallRight`. Ramas = atajos.
- [ ] `PickupRoute` solo sesga la línea hacia rayitos; no crea pickups.
- [ ] Steering: pursuit + avoidance. Dificultad = datos (`difficulty.rs`), no otro binario.
- [ ] Hasta 32 bots. `room` (aunque sea local) llena cupos vacíos.
- [ ] Misma máquina de slot y uso de poderes que un humano.
- [x] Muchos autos a la vez en el motor de Revvy, con choques entre ellos (2.7; el render dibuja hasta 32).
- [ ] En esta fase los simula el proceso local. En red: host si `Client`, server si `Server` (fase 7).

**Hecho cuando:** 8 bots completan vueltas en una pista con `.fan`, recogen rayitos y gastan al menos bombucha y aceite sin trabarse en un nodo sin link.

---

## Fase 7 — Sala, red y autoridad (§1.3, §2, §4, §4.6–4.8, §6.6.1)

Objetivo: dos clientes desktop en la misma sala. Singleplayer es la misma máquina de estados con un humano (puede ser in-process, sin salir a internet, pero el flujo de lobby es el de §4.1).

### 7.1 Protocolo (`crates/net`, `docs/protocol.md`)

- [ ] Mensajes postcard: `Input`, `Snapshot` / `VehicleState`, `MapSync` (`MapRequired`, `AssetReady`), `LobbyEvent`.
- [ ] `LobbyEvent`: `SetTrack`, `SetReady`, `SetRules`, `SetPickupOdds`, `StartRace`, `ForceStart`, `PlayerJoined`, `PlayerLeft`.
- [ ] Codec: postcard, zstd opcional en payloads grandes.
- [ ] Quinn: canal de juego + stream dedicado de mapas (no bloquea inputs).

### 7.2 Lobby (§4.1)

- [ ] Host crea sala → `room_id` (Axum puede devolverlo; el canal es Quinn).
- [ ] Pista default = primera del catálogo. En local sin catálogo, primera carpeta válida de `levels/`. **No se descarga en el lobby.**
- [ ] Cada humano entra en `LobbyStatus::Waiting`. Bots no tienen Listo.
- [ ] `SetReady` ↔ puede volver a Waiting. Cambiar pista o reglas resetea todos los Listo a Waiting.
- [ ] Iniciar deshabilitado hasta que todos los humanos estén Listo. El host no pulsa Listo: Iniciar es su confirmación. Singleplayer: un humano puede iniciar ya.
- [ ] Config visible para invitados: `sim_authority`, `laps`, turbo, odds (según lock), `late_join_mode`, `disconnect_bot_replace`, cantidad de bots.
- [ ] Al Iniciar, la config se **congela** hasta volver al lobby.

### 7.3 Loading y 3-2-1 (§4.2, §4.3)

Dos “ready” distintos en código y UI:


| UI        | Código                 | Significado                        |
| --------- | ---------------------- | ---------------------------------- |
| Esperando | `LobbyStatus::Waiting` | No confirmó                        |
| Listo     | `LobbyStatus::Ready`   | Confirmó; no implica tener el mapa |
| (interno) | `AssetStatus::Ready`   | Mapa cargado                       |


- [ ] `MapRequired { id, hash, size, version }` solo después de Iniciar. Timer 15 s.
- [ ] Resolución local: `levels/<id>/` mismo hash, si no `cache/server/<id>/`, si no descarga (la descarga real es fase 8; en esta fase alcanza con `levels/` local y un error claro si falta).
- [ ] Cuando todos los que corren tienen `AssetReady`: countdown **3, 2, 1** con el mismo tick de server, replicado.
- [ ] A los 15 s, si falta alguien, el host ve **Iniciar de todos modos**.
- [ ] `late_join_mode: Spectator` — sin auto, cámara spectator, slot no corre (se puede llenar con bot). `AssetReady` tardío no spawnea.
- [ ] `late_join_mode: Racer` — slot de grilla reservado; al llegar `AssetReady` spawnea en marcha, sin rewind.

### 7.4 Autoridad (§4.8) — se elige en config y se congela al Iniciar

**`Client` (amigos)**

- [ ] Cada dueño integra su física y manda `VehicleState`. Los demás interpolan.
- [ ] El host simula bots y emite sus estados.
- [ ] El host arbitra rayitos, poderes, vueltas, respawn.
- [ ] Auto custom: se acepta la pose del dueño.

**`Server` (competitivo)**

- [ ] Clientes mandan `Input`. El server corre la física de Revvy (`revvy-physics`: Rapier + vehículo, §1.9) y manda snapshots.
- [ ] El server simula bots y arbitra reglas.
- [ ] Validación de rangos de input/velocidad (anti-cheat básico, §9).
- [ ] Auto no oficial: hull `cars/_default/` en el server, no el `.inf` custom.

En ambos modos el server de sala hace lobby, `MapRequired` y “host se fue → cierra”. En `Client` el server no simula 32 autos.

### 7.5 Autos custom (§6.6.1)

- [ ] Catálogo oficial = set de `car_id` publicado. Cualquier otro id local es custom.
- [ ] Dueño ve su mesh y (en `Client`) siente su física.
- [ ] Los demás: `client/assets/cars/placeholder.glb`.
- [ ] Nametag y lista de sala: a la derecha del nombre, `! Custom Car`.
- [ ] No se descarga el `.glb` custom en v1.

### 7.6 Desconexión (§4.6)

- [ ] No-host: auto a vel 0, idle, slot reservado. Reconexión con el mismo `player_id`/token restaura pose, vueltas, poder, standings.
- [ ] Si `disconnect_bot_replace`: un bot conduce ese auto (poderes y vueltas incluidos). Al volver, el humano hereda el estado del bot.
- [ ] `reconnect_secs` default 60: después, el idle (o el bot) sigue hasta el fin de carrera.
- [ ] Host se desconecta: sala cerrada, todos al menú. Sin migración de host.

### 7.7 Fin de carrera en red (§4.7)

- [ ] `laps` de la sala. Al completar, el corredor terminó.
- [ ] Cuando terminan los humanos (o el host corta): `Results` breve → lobby con **el mapa de fondo**, todos en Waiting, config editable, pista = la última (no el default). No se vuelve a bajar el mapa.

### 7.8 Grilla

- [ ] `start_grid[i]` del layout = spawn del slot `i`. Mínimo 1, pensado hasta 32.

**Hecho cuando:** dos PCs (o dos procesos) hacen Waiting → Listo → Iniciar → 3-2-1 en `Client` y en `Server`; un custom car se ve como placeholder con el warning; si el no-host se cae, el auto queda idle y al reconectar sigue; si el host se cae, la sala muere; al terminar se vuelve al lobby con el mundo detrás.

---

## Fase 8 — Streaming de mapas, caché desktop/mobile, catálogo (§4.4, §4.5, §6, §8)

### 8.1 Reserva de disco — solo desktop (`reserve.rs`)

- [ ] Al arrancar: `cache/reserve.dat` de exactamente 100 MB reales (`fallocate` / ceros, no sparse).
- [ ] Si no se puede crear: bloquear online y singleplayer con catálogo. Mensaje de 100 MB libres. `levels/` a mano sigue usable.
- [ ] Al descargar: borrar la reserva, escribir `cache/server/<id>/`.
- [ ] Al terminar la descarga o al purgar la pista vieja: recrear `reserve.dat`.

Techo esperado:


| Estado                                    | Disco extra                     |
| ----------------------------------------- | ------------------------------- |
| Idle / lobby                              | 100 MB reserva                  |
| Post-carrera, una pista en caché          | ≤ 100 MB pista + 100 MB reserva |
| Loading de otra pista (pinned + incoming) | ≤ 200 MB en pistas, reserva 0   |
| Tras el 3-2-1 de la pista nueva           | ≤ 100 MB pista + 100 MB reserva |


La reserva es solo de mapas, no de autos. **No existe en Android/iOS.**

### 8.1b Caché en RAM — mobile

- [ ] El `.zst` entra a un `Vec<u8>` (o equivalente), se descomprime en RAM y eso es *pinned* / *incoming*. Cero escrituras a `cache/server/` y cero `reserve.dat`.
- [ ] Mismas transiciones que §4.4: rematch no re-descarga mientras el proceso viva; al 3-2-1 de otra pista se libera el buffer anterior; salir de la sala libera todo.
- [ ] Si el alloc falla: la descarga no entra, mensaje de memoria insuficiente. `levels/` copiado a mano sigue en disco.
- [ ] El SO puede matar la app en background: al reabrir, la pista del server ya no está (hay que bajarla de nuevo).

### 8.2 Ciclo de caché `origin: server`

`levels/` manual **nunca** se borra.

- [ ] Elegir pista en el lobby no descarga ni borra.
- [ ] Iniciar la misma pista: carga de caché, 0 bytes.
- [ ] Iniciar otra: bajar a *incoming* sin borrar *pinned*. Máximo 2 carpetas server. Si el host cambia de idea otra vez en el lobby antes de largar, se reemplaza incoming.
- [ ] Al empezar el 3-2-1 de una pista distinta: borrar pinned, incoming pasa a pinned, recrear reserva.
- [ ] Fin de carrera: la caché **se queda** (rematch).
- [ ] Salir de la sala o morir el proceso: soltar la caché. Desktop deja `reserve.dat`. Mobile no deja archivos.

### 8.3 Paquete y catálogo

- [ ] `map-packager`: carpeta legacy **o** `revvy-glb-v1` → `.zst` + manifest (hash, tamaño, versión, autor). Falla si > 100 MB.
- [ ] Pista legacy: `map-packager` suma el banco de `wavs/` que usa el nivel (§1.10).
- [ ] `server/maps_storage`: filesystem + fila en Postgres (metadata). Upload admin en `api/maps.rs` vuelve a chequear 100 MB.
- [ ] `GET` catálogo para el lobby (orden estable: la primera entrada es el default).
- [ ] Stream QUIC del `.zst`, descompresión, marca `origin: server`.
- [ ] Hash match en `levels/` o en caché → `AssetReady` sin bajar.
- [ ] Copia manual completa (`track.toml` + `visual.glb` + `layout.ron`, o carpeta Re-Volt) cuenta como local. `.blend` / `.bbmodel` no viajan.

### 8.4 glTF de pistas nuevas (§6.1–6.5, §6.7)

- [x] Crate `gltf` solo en `revvy-formats` (`glb.rs`, que usan pistas y autos). (2.7)
- [x] Escena obligatoria, nombres case-sensitive (2.7):

```
Visual      # se dibuja
Collision   # no se dibuja; material = SurfaceType
Props       # opcional
```

- [x] Falta `Collision` → rechazar la pista (no usar `Visual` como collider). (2.7)
- [x] Unidades 1 m, Y-up. Origin ≠ largada: la grilla sale de `layout.ron`. (2.7)
- [ ] Un solo `visual.glb` (no `collision.glb` aparte). `.gltf` sueltos se hornean a `.glb` en `map-packager`.
- [ ] `extras.revvy.collider = trimesh | cuboid | convex`. Default trimesh. Cuboid = AABB / `half_extents`, sin TriMesh. Convex para props. TriMesh estático solo para piso manejable.
- [x] `Collision` entra al mismo motor que una pista traducida: colliders de Rapier con su `SurfaceType`. (2.7)
- [ ] Atributos de Collision: solo `POSITION` (+ material). Ignorar UV/normales si vinieron; `map-packager` avisa.
- [ ] Presupuesto en `map-packager`:
  - tris Collision / Visual: warn > 15 %, **error** ≥ 50 %
  - payload Collision / glb: warn > 5 %
  - error si la raíz Collision es un único convex
  - error si el `.zst` > 100 MB
- [ ] Meshopt + `KHR_mesh_quantization` solo en `Visual`. Nunca cuantizar Collision.
- [ ] `track.toml`: `id`, `name`, `author`, `laps` (metadata), `format = "revvy-glb-v1"`, `env`, más `preview.png`.
- [x] Server carga el glb pidiendo solo collision (`TrackLoad::collision_only`). Cliente pide visual + collision. (2.7)
- [ ] No ensamblar módulos de Blockbench en runtime: un glb por pista.
- [ ] No leer custom properties salvo los nombres de nodo y `extras.revvy.collider`.
- [ ] Checklists `dcc/blender/revvy_export.md` y `dcc/blockbench/revvy_export.md`: unidades, nodos, presupuesto, strip de UVs en Collision, “el piso no se decima a ciegas”.

### 8.5 Autos nuevos (§6.6) y packager

- [x] `cars/<id>/{car.toml, body.glb, collision.glb}`. Si falta collision, hull desde el body (peor, warning). (2.7)
- [x] `car.toml` con los parámetros del vehículo de Revvy (`[vehicle]`), el mismo tipo que sale de traducir un `parameters.txt`: manejo lo más parecido posible a Re-Volt, sin código de Re-Volt, sin `CARINFO.TXT` ni defaults de Re-Volt. Chasis desde `collision.glb`, escala de radiocontrol. (2.7)
- [ ] `car-packager` análogo, para el catálogo oficial.
- [ ] `cars/_default/` hull usado por el server cuando el auto es custom en modo `Server`.
- [ ] `asset-inspector`: abre legacy o `.glb` y muestra counts / errores de nombres.

**Hecho cuando:** un cliente desktop sin la pista la baja al Iniciar, el rematch no re-descarga, la pista anterior se suelta en el 3-2-1, y sin 100 MB de disco no entra al catálogo. En mobile el mismo flujo usa solo RAM (sin archivos de caché). Un `.glb` sin nodo Collision no se publica; un Collision copiado del Visual (≥ 50 % tris) falla el packager.

---

## Fase 9 — Editor de pista desktop (§7, §8)

No es plugin de Blender ni de Blockbench. No se compila en mobile ni en el server. Feature `track-editor`. Menú **Editor de pista** y `cargo run -p track-editor -- --track levels/<id>`.

El editor no modela meshes. Reabrir después de reexportar el `.glb` conserva `layout.ron`.

### 9.1 Infra

- [ ] `gizmo.rs` (wgpu, no gizmos de egui en la escena): OBB, esferas verde/rojo, polilíneas, flechas de grilla y de fuerza, rayito, wireframe de Collision (toggle, default on).
- [ ] `picking.rs`: rayo de mouse contra colliders de gizmos.
- [ ] Flycam WASD + mouse y cámara de auto.
- [ ] egui solo para paneles.
- [ ] `save.rs` escribe `layout.ron`. `validate.rs` bloquea el guardado si falla.
- [ ] Orden de modos con candado (el siguiente no abre si el actual no valida), salvo pickups, campos, superficies, car params y kill volumes, que no bloquean el camino principal:
  1. StartGrid (≥ 1, hasta 32)
  2. PosNodes (links, default práctico 8; `distance` y `total_distance` se calculan al guardar; `start_node` = meta)
  3. TrackZones (OBB, id secuencial, 0 = meta)
  4. AiNodes
  5. Pickups + odds de autor
  6. ForceFields
  7. Surfaces
  8. CarParams
  9. KillVolumes
- [ ] Ghost opcional: Time Trial, `G` resample a AiNodes, `Shift+G` racing line, `Ctrl+Shift+G` overtaking.
- [ ] Sin cap de 1024 nodos.
- [ ] Pista a medias abre en el editor; **no** entra a carrera si `validate` falla.

### 9.2 AiNodes

- [ ] Par verde (izq) / rojo (der). `racing_t` y `overtaking_t` en 0, 1.
- [ ] Flags: `Racing`, `Slowdown`, `SpeedLimit(f32)`, `PickupRoute`, `Careful`, `WallLeft`, `WallRight`.
- [ ] `next[]` con ramas (atajos).

### 9.3 Pickups de autor (no es el panel del host)

- [ ] Clic en el piso = `PickupSpawn { pos, respawn_secs, odds }`.
- [ ] Panel: capa 3 solo lectura + “Usar como base”. Capa 2 toggle de pista. Capa 1 por rayito (checkbox, anillo en el gizmo).
- [ ] La UI lista **todos** los `PowerupKind` al copiar la capa inferior.

### 9.4 Surfaces, forces, params, kills

- [ ] Force field: mismos OBB, flecha del vector, kinds de §7.8. En carrera el VFX es opcional; en editor el OBB siempre se ve.
- [ ] Surfaces: tabla de overrides + OBB de `surface_volumes`. Tint de wireframe (cian hielo, marrón dirt, …). Aviso si hay `ParamMod` All+Ice+Grip **y** override de `Ice.lateral_grip`.
- [ ] CarParams: tabla Target / Stat / Op / Valor / Superficie, drag para reordenar, preview antes/después (y columna “en hielo”). Combo de `cars/` locales + id tipeado a mano.
- [ ] KillVolumes: OBB de pozo. No reemplazan a las TrackZones (salirse de todas también respawnea; eso ya está en fase 3).

### 9.5 Lo que el editor no hace

- [ ] No genera Collision por decimate (botón “piso desde Visual” queda como idea posterior, nunca asset final sin revisar).
- [ ] No empaqueta ni sube (`map-packager`).
- [ ] No edita visiboxes ni cámaras de replay.

**Hecho cuando:** una pista glTF nueva se puede correr de punta a punta solo con lo que el editor guardó (grilla, zonas, IA, un rayito con odds propias, un campo, un kill volume), y el wireframe de Collision muestra un desajuste a propósito.

---

## Fase 10 — Persistencia, stats, auth (§1.4, §5, §9)

- [ ] Postgres + `sqlx` + `sqlx-cli` migraciones: cuentas, metadata de mapas/autos, `race_summary`, `session_summary`.
- [ ] Redis (`deadpool-redis` o `fred`): presencia, salas activas, rate-limit. Pub/sub entre instancias **no** en v1.
- [ ] Auth liviana: JWT (`jsonwebtoken`) o token de sesión en Redis. Alcanza para `player_id` estable en la reconexión.
- [ ] `revvy-stats`: `CollisionEvent`, `PowerupUsedEvent`, `LapEvent` → al cerrar la carrera el server escribe `race_summary`. Agregado de sesión cada 12–16 carreras. `GET` en `api/stats.rs`.
- [ ] Leaderboards de solo lectura en Axum.
- [ ] `api/maps.rs`: listado, upload admin, 100 MB.
- [ ] Almacenamiento: filesystem + tabla. Diseñar el manifest para poder mudar a S3/MinIO sin tocar el protocolo.

**Hecho cuando:** una carrera online deja filas de vueltas y poderes; el mismo usuario se reconecta y el server lo reconoce; el catálogo sale de Postgres.

---

## Fase 11 — Mobile, CI, empaquetado (§1.8, §5, §9)

- [ ] `cargo-mobile2` (o `xbuild`) Android/iOS. Mismo `winit` + `wgpu`.
- [ ] `client/src/platform`: overlay táctil (egui) y scheme de input. No se piden permisos de storage para mapas del server.
- [ ] Pistas bajadas del server solo en RAM (§8.1b). Si no hay memoria para el paquete, la descarga falla con mensaje claro.
- [ ] Feature `track-editor` **off** en esos targets.
- [ ] Gamepad sigue siendo `gilrs` en PC; en mobile no es requisito.
- [ ] `cross` para el server Linux amd64/arm64.
- [ ] GitHub Actions: `cargo test --workspace` + build desktop. Empaquetado mobile cuando el slice de fase 7 corra en desktop.

**Hecho cuando:** un build Android entra al lobby, toca Listo, recibe el mapa en RAM (sin `cache/server/` ni `reserve.dat`) y corre una carrera corta. El binario no contiene el editor.

---

## Fase 12 — Contenido jugable mínimo para cerrar v1

No es código de engine; es el paquete con el que se prueba el roadmap entero.

- [ ] Al menos 1 pista legacy que cargue sin warnings fatales (zonas, POS, AI, un par de rayitos).
- [ ] Al menos 1 pista `revvy-glb-v1` hecha en Blender o Blockbench, con Collision dentro del presupuesto, `layout.ron` completo y `preview.png`.
- [ ] Autos oficiales suficientes para llenar una grilla chica + `cars/_default/` + el placeholder.
- [ ] `powerups.default.ron` y `surfaces.default.ron` con números jugables (aunque se afinen después).
- [ ] `default.ron` de sala: `sim_authority = Client` por defecto en “partida con amigos”; el host puede pasar a `Server`.

---

## Explícitamente después de v1 (§1.6, §7.6, §9)

- [ ] **Rhai** (`crates/rules_engine`) para modos que no alcanzan con números de `GameplayRules`.
- [ ] Visiboxes y cámaras de replay en el mismo `layout.ron` (parsers `.vis` / `.cam` ya pueden existir desde la fase 1).
- [ ] Botón de editor “crear piso desde Visual” como borrador, nunca como Collision final sin revisar.
- [ ] Migración de host si el host se cae.
- [ ] Descarga del mesh custom a los demás (hoy es placeholder).
- [ ] Varias instancias de game server coordinadas por Redis pub/sub.
- [ ] Mapas en S3/MinIO.
- [ ] Salto habilitado en algún modo (`allow_jump: true`) cuando el handling esté cerrado.
- [ ] Ragdoll de props si hace falta, en el mismo Rapier del motor.
- [ ] **Modo reversed**: cargar la subcarpeta `reversed/` de una pista legacy. Las pistas custom no son consistentes: en seis, el `.fan` es el de la pista normal con los links dados vuelta, y el verde queda a la derecha del sentido de carrera. La traducción tendría que normalizarlos (ver `docs/formats/fan.md`).

---

## Orden de ataque (resumen)


| Fase | Qué se puede jugar al terminarla                                            |
| ---- | --------------------------------------------------------------------------- |
| 0    | Ventana vacía y server que responde                                         |
| 1    | Tests de parseo de una pista Re-Volt real                                   |
| 2    | Calcure en nhood1 como en Re-Volt, sobre el motor de Revvy (2.7)            |
| 3    | Vueltas, wrong way, respawn, volcar                                         |
| 4    | Hielo, viento, mods de `calcure`                                            |
| 5    | Los 11 poderes y el lock de odds                                            |
| 6    | Bots que corren y usan poderes                                              |
| 7    | Dos jugadores, autoridad Client/Server, custom car, desconexión             |
| 8    | Descarga al Iniciar; disco+reserva en desktop, RAM en mobile; pistas `.glb` |
| 9    | Editor MAKEITGOOD                                                           |
| 10   | Stats y cuentas                                                             |
| 11   | Android/iOS sin editor                                                      |
| 12   | Un paquete de contenido para enseñar el loop completo                       |


No empezar por el editor ni por mobile: las fases 2–3 son el juego; 7 es el multijugador; 8–9 son el pipeline de artistas. 2.7 va antes que la fase 3: carrera, reglas, poderes, bots y red se construyen sobre el motor de Revvy, no sobre el port.