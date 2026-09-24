# Revvy — Stack Tecnológico y Arquitectura

> Juego de carreras de radiocontrol propio, en Rust, **retrocompatible con pistas y autos de Re-Volt**. No es un port (§0). Sin motor gráfico con interfaz (nada de Godot/Unity), con streaming de mapas, crossplay PC/Mobile, bots, stats y reglas editables por sala.

---

## 0. Qué es Revvy: un juego retrocompatible, no un port

Revvy es un juego de carreras propio, con su motor y su formato de contenido. **No es un port de Re-Volt**: es **retrocompatible** con sus pistas y sus autos. Una pista o un auto de Re-Volt se copia a `content/` tal cual y se juega, igual que el contenido nuevo (pistas `.glb`, autos con `car.toml`).

La retrocompatibilidad es una capa de traducción, no un segundo motor:

```
 Contenido Re-Volt                         Contenido Revvy
 .w .prm .ncp .fin .fob .inf               visual.glb  layout.ron
 parameters.txt .hul .wav                  car.toml  body.glb  collision.glb
         │                                         │
         ▼                                         ▼
 revvy-formats: parsers + traducción       revvy-formats: loaders
 (ejes, unidades, materiales,              (glTF, car.toml)
  parámetros del auto, sonidos)                    │
         │                                         │
         └────────────────────┬────────────────────┘
                              ▼
            tipos de Revvy (metros, Y arriba, SI):
            TrackAsset · vehículo · superficies · emisores
                              │
                              ▼
                       motor de Revvy
   render wgpu · física Rapier + vehículo Revvy · sonido Kira
                   red · reglas · bots
```

- **Un solo motor.** Render, física, sonido, red, reglas y bots son de Revvy y no saben de dónde vino el contenido. En una misma sala conviven un auto de Re-Volt y uno propio, sobre una pista de Re-Volt o una `.glb`.
- **Todo en el espacio de Revvy.** Metros, Y arriba, unidades SI (§6.5). La traducción convierte ejes, unidades, materiales y parámetros una sola vez, al cargar. Ningún sistema del motor lee estructuras de Re-Volt ni corre en su espacio (unidades de 5 mm, Y abajo).
- **Lo que no entra se resuelve en Revvy.** Si algo del legado no se puede expresar (un material, un parámetro del auto), se amplía el tipo de Revvy. No se abre un camino especial para Re-Volt dentro del motor.
- **`rvsource` es referencia, no motor.** Documenta los formatos y el comportamiento al que el juego se acerca: el manejo, la cámara y el sonido de Re-Volt. El motor lo imita con código propio. Los datos de Re-Volt, como `CARINFO.TXT`, solo completan contenido de Re-Volt (§6.5).
- **Meta de manejo.** Rapier es el motor de física de todo el juego. Un auto de Re-Volt traducido se maneja lo más parecido posible a Re-Volt, aunque no sea idéntico. Un auto propio usa el mismo modelo con sus parámetros.

**Estado actual.** Desde la fase 2.7 el juego cumple estas reglas: las pistas y los autos de Re-Volt entran traducidos y corren en el motor de Revvy junto con el contenido propio de prueba (`revvy_arena`, `revvy_buggy`). El port de Re-Volt de la fase 2.5 ya no corre en el juego; queda como referencia de manejo en los tests (§1.9).

---

## 1. Stack tecnológico completo

### 1.1 Núcleo del lenguaje
- **Rust** (edition 2021/2024) — workspace con múltiples crates.
- **tokio** — runtime async (server y networking del cliente).
- **glam** — álgebra lineal (vectores, matrices, quaterniones), estándar de facto con wgpu/bevy_ecs/rapier.
- **serde** + **ron** — (de)serialización general; RON para `GameplayRules` y `layout.ron` (zones/nodos de pistas nuevas).
- **tracing** + **tracing-subscriber** — logging estructurado, tanto cliente como server.
- **anyhow** / **thiserror** — manejo de errores.

### 1.2 Cliente — Render / Simulación / Input

El corte que corre hoy es **manejo en una pista**: `load_track` + `load_car` → la pista (de Re-Volt o `.glb`) con su cielo y uno o más autos (de Re-Volt o propios) en el motor de Revvy (§1.9), con cámara de persecución (o libre) y sonido (§1.10). gilrs sigue en el stack de destino; el cliente todavía no lo enlaza.

- **wgpu** — renderizado (Vulkan/Metal/DX12/GLES vía naga, cubre PC y mobile).
- **winit** — ventana + eventos de input (PC), y base para Android/iOS.
- **bevy_ecs** — ECS *standalone* (no el motor Bevy completo, solo el crate de ECS) para entidades del juego: autos, pickups, checkpoints, bots.
- **rapier3d** — física del motor de Revvy: cuerpos rígidos, colisiones, queries (ruedas, cámara, picking) y props. Encima corre el modelo de vehículo propio de Revvy (§1.9).
- **Kira** — audio: motor, SFX posicional, música (§1.10).
- **egui** + **egui-wgpu** + **egui-winit** — UI inmediata: menús, HUD, lobby, pantallas de stats.
- **gilrs** — soporte de gamepads (PC/consola-like), independiente de winit.
- **image** — carga de texturas `.bmp` (formato nativo de Re-Volt) y otros formatos modernos si se agregan.
- **gltf** (crate Khronos) — carga de pistas/autos **nuevos** en glTF 2.0 binario (`.glb`). No se usa para el legado Re-Volt.

### 1.3 Networking
- **Quinn** (QUIC) — canal de juego en tiempo real: snapshots, inputs, sincronización de sala, transferencia de mapas.
- **postcard** — serialización binaria compacta para todos los mensajes de red (rápida, sin overhead de schema).
- **zstd** (crate `zstd`) — compresión de paquetes de mapas/autos y opcionalmente de snapshots grandes.
- **Axum** — API HTTP/REST: catálogo de mapas, auth, leaderboards, consulta de stats históricas (todo lo que no necesita baja latencia).

### 1.4 Backend / Persistencia
- **PostgreSQL** vía **sqlx** (async, compile-time checked queries) — stats por carrera/sesión, cuentas, leaderboards, metadata de mapas/autos.
- **Redis** vía **deadpool-redis** o **fred** — cache de salas activas, presencia, rate-limiting, pub/sub si en el futuro escalás a múltiples instancias de servidor de juego.
- **sqlx-cli** — migraciones versionadas.

### 1.5 Compatibilidad legacy (Re-Volt)
- Crate propio `revvy-formats` con parsers para: `.ncp` (malla de colisión), `.prm` (mesh de auto/pieza de pista), `.w` (mundo/instancias de pista), `.vis` (datos de visibilidad/oclusión), `.fan` (AI Nodes), `.pan` (POS Nodes), `.taz` (Track Zones), `.cam` (cámaras de replay), `.fin` (parámetros/límites de pista), `.fld`, `.fob` (objetos/props instanciados), `.inf` (parámetros de auto o info de pista según contexto), `.li-`/`.lit` (iluminación), `.por` (portales de visibilidad), `.pro` (perfil/ruta de IA), `.bmp` (texturas).
- Recomendación práctica: **RVGL** (el fan-remake open source de Re-Volt) ya tiene reverse-engineering documentado y código abierto de estos formatos — usalo como referencia cruzada para no reinventar el parsing desde cero y evitar errores sutiles de layout binario.
- Estrategia para el **legado**: **no convertir** los archivos a un formato propio. Se parsean tal cual y se traducen en memoria al cargar (§0), así una pista/auto copiada manualmente a `levels/` o `cars/` funciona sin pasos intermedios.
- Las pistas **nuevas** (Blender / Blockbench) **no** se exportan a `.w`/`.ncp`/`.prm`. Van en glTF 2.0 (`.glb`) + sidecar de layout. Ver secciones 6–8. El runtime unifica ambos orígenes detrás del trait `TrackAsset`.
- Capa de traducción ya en uso: carpeta Re-Volt → `load_track(dir)` → `TrackAsset` en Y-up. Otro mapa u otro auto no piden código nuevo: la pista se elige en el menú (Offline → Seleccionar pista lista todas las de `levels/`) y el auto del teclado es `car` en `config/client.toml`. Con argumentos se saltea el menú: `cargo run -p revvy-client -- nhood1 phim_calcure`. Ver §6.5.
- La traducción entrega todo en tipos de Revvy:
  - mallas, texturas y cielo o, si la pista no tiene, el color de fondo (`FOGCOLOR` del `.inf`, como `SetBackgroundColor`);
  - la colisión de `TrackAsset.collision`: triángulos en Y arriba con el frente del lado de la normal del polígono (en una instancia espejada se invierte el orden, como con `RotTransPlane`), su `SurfaceType` y si son solo de cámara o solo de objetos;
  - la grilla de largada entera (`CarGridStarts`);
  - los sonidos de la pista (`TrackSounds`: banco y emisores);
  - `CarDef.vehicle` (`VehicleParams` en SI) y `CarDef.sound`.

  `TrackAsset.legacy` (`LegacyLevel`) y `CarDef.revolt` (el `CAR_INFO` crudo) siguen saliendo, pero solo los lee el port de referencia en los tests.
- Las líneas de `parameters.txt` que empiezan con `;)` son claves de RVGL que el Re-Volt original toma como comentario (`SFXENGINE`, `TCARBOX`, `Flippable`…). Revvy las lee como RVGL.
- El **modo reversed** queda fuera de v1: las subcarpetas `reversed/` de las pistas custom no son consistentes entre sí (`docs/formats/fan.md`). El loader lee solo la carpeta de la pista y no entra a `reversed/`.

### 1.6 Reglas editables / modos custom
- Config data-driven en **RON** o **TOML** por sala (`GameplayRules`). Vueltas, `late_join_mode`, `sim_authority`, bot al desconectar, odds de pickups, turbo, etc. viven ahí, no hardcodeados.
- **Autoridad de simulación** (`GameplayRules.sim_authority`): `Client` (con amigos) o `Server` (competitivo). Ver §4.8. El canal Quinn de sala existe en ambos; lo que cambia es quién integra la física del auto.
- Odds de poderes de **autoría de pista** (capas 1–2), campos de fuerza, piso, mods de auto y volúmenes de respawn van en `layout.ron` (§7). El **host** pisa las odds **solo si la pista no trae tabla propia** (§7.7.1).
- Opcional a futuro: **Rhai** (scripting embebido, puro Rust, sandboxeado) para lógica de modos custom más allá de simples valores numéricos.

### 1.7 Bots
- Sin librería externa: steering behaviors simples (pursuit, avoidance) que manejan el mismo auto de `revvy-physics` que el jugador (§1.9), con niveles de dificultad como datos, no código distinto por nivel.
- Fuente de ruta: en pistas **legacy**, los AI Nodes de `.fan` (y POS de `.pan`). En pistas **nuevas**, el grafo de `layout.ron`. `revvy-bots` consume un `AiPath` ya unificado.
- Los bots **recogen y usan** poderes con las mismas reglas de slot que un humano (§7.7.2). En `sim_authority: Client` los bots los simula el **host**; en `Server`, el servidor de juego.

### 1.8 Empaquetado / Build / Mobile
- **cargo-mobile2** (o **xbuild**) — compilar y empaquetar cliente para Android/iOS reusando el mismo código base (winit ya soporta ambos backends). El feature `track-editor` **no** se activa en estos targets.
- **cross** — cross-compilation para targets de servidor (Linux ARM/x86).
- CI sugerido: GitHub Actions / cargo-dist para builds multiplataforma.

### 1.9 Física: motor de Revvy y traducción de Re-Volt

La física es una sola para todo el contenido: **Rapier** (cuerpos rígidos, colisiones, queries) con un **modelo de vehículo propio de Revvy** encima, en metros y con Y arriba. Un auto de Re-Volt y uno propio, o una pista `.ncp` y una `.glb`, entran al motor con los mismos tipos y comparten la simulación. Los choques entre autos y contra props los resuelve Rapier para todos.

- **Vehículo de Revvy.** El chasis es un cuerpo de Rapier con su collider. Las ruedas no son cuerpos: cada una es una esfera que se prueba contra los triángulos de la pista que tiene cerca (el BVH de la malla de Rapier), con la regla de `SphereCollPoly`: la cara, o un solo borde. Tiene resorte y amortiguador con recorrido máximo, y fricción de rueda con cono estático/cinético y agarre. El torque de motor se apaga al llegar a la velocidad tope. Suma fricción de eje, volante con tasa y respuesta, downforce y resistencias. El código es de Revvy y se tiene que parecer lo más posible al comportamiento de Re-Volt (*Comportamiento de referencia*, abajo). Idéntico no va a ser: Rapier resuelve los choques del chasis de otra forma, así que paredes, aterrizajes y vuelcos se ajustan a mano.
- **Modo de referencia: Simulación.** Re-Volt tiene cuatro modos. Con un solo auto, Simulación y Arcade manejan igual: la diferencia es el choque entre autos (`DetectCarCarColls`).
  - **Arcade** reduce cada auto a una o dos esferas sacadas de su caja.
  - **Simulación** choca los cascos convexos del `.hul` entre sí (`DetectHullHullColls`), más rueda contra carrocería y rueda contra rueda.
  - **Consola** además anula el giro que dan los golpes contra paredes, y **Kids** baja el volante y la velocidad tope.

  Revvy se inspira en **Simulación**: en Rapier, cada auto choca con sus formas reales. No usa las esferas de Arcade ni los cambios de Consola y Kids.
- **Escala y gravedad.** Una unidad de Re-Volt mide **5 mm**. Lo dice su propio velocímetro: `units.h` convierte a km/h con `OGU2KPH_SPEED` 0.018 y a mph con `OGU2MPH_SPEED` 0.01118, y las dos dan 0,005 m/s por unidad/s. A esa escala:
  - la gravedad de Re-Volt (2200 unidades/s²) es **11 m/s²**, y esa es la gravedad del motor de Revvy;
  - el mph de Re-Volt es el real;
  - el Calcure mide 68 × 30 cm, con ruedas de 11 cm y 2,6 kg: un auto a radiocontrol.

  Como es solo un cambio de unidad, el contenido traducido se siente igual sin compensar nada. Los campos de fuerza escalan la gravedad (§7.8).
- **Paso y mandos.** El motor avanza en pasos fijos de 1/180 s, los mismos en el host, el server y los clientes (Re-Volt a 60 cuadros por segundo da tres pasos así). El dibujo interpola entre los dos últimos. Los mandos se leen a 60 Hz fijos: el volante de Re-Volt da su primer paso desde el centro ×4 por cuadro, así que la frecuencia de lectura es parte del manejo.
- **Chasis en Rapier.** Masa e inercia de `VehicleParams`; las resistencias son el amortiguamiento de Rapier (120 × resistencia da el mismo factor por paso). Las esferas tocan el mundo; los cascos convexos y una esfera por rueda chocan con otros autos (rueda contra carrocería y rueda contra rueda). La fricción y el rebote contra la pista salen de la superficie del triángulo, con un hook de Rapier; en las paredes resbala más y rebota un poco más. El *contact clustering* de Rapier está apagado, para que cada contacto sea de un triángulo.
- **Enderezar.** Con `R`, si el auto está dado vuelta (`up.y ≤ 0.3`) y toca algo, el chasis pasa a cinemático, sube 25 cm y gira hasta quedar derecho mirando para donde miraba; después vuelve a ser dinámico.
- **Falta.** Marcas de derrape, chispas y polvo, la antena, el env map del chasis, aceite, speedups y catch-up (el port tampoco los tiene).
- **Traducción de Re-Volt** (`revvy-formats`, una vez al cargar):

  | Re-Volt | Revvy |
  | --- | --- |
  | `.ncp` del mundo y de las instancias del `.fin` (unidades de 5 mm, Y abajo) | `TriMesh` estático de Rapier (m, Y arriba); cada quad, dos triángulos |
  | material del polígono (27 de `COL_MaterialInfo`) | superficie de Revvy: fricción, agarre, dureza, corrugado, velocidad de cinta |
  | flags de instancia: sin colisión de objetos / sin cámara | grupos de colisión: solo cámara / solo objetos |
  | `parameters.txt` + defaults de `CARINFO.TXT` | parámetros del vehículo de Revvy, en SI |
  | esferas y cascos convexos del `.hul` | colliders del chasis: las esferas chocan con el mundo y los cascos con otros autos, como en Simulación |
  | `STARTPOS`, `STARTROT`, `STARTGRID` | `start_grid` |

  Los parámetros del auto se convierten según su dimensión: largos, velocidades y aceleraciones ×0.005; inercias ×2,5 × 10⁻⁵. La masa ya está en kg, y la rigidez y la amortiguación de los resortes y los coeficientes de fricción no cambian. `TopSpeed` ya está en mph reales. La tabla completa vive en el código de traducción, con tests.
- **Superficies.** `SurfaceType` cubre los 27 materiales de Re-Volt, en el orden de su índice (`from_revolt` es uno a uno). `revvy-physics::surfaces` da el perfil de cada uno con los valores de `COL_MaterialInfo` en metros: fricción, agarre, dureza, baches y velocidad de las cintas (§7.9).

**El port de referencia.** La fase 2.5 portó directo el motor de Re-Volt en `revvy-physics::revolt`: `newcoll.cpp`, `body.cpp`, `car.cpp`, `wheel.cpp`, `control.cpp`, `move.cpp` y `camera.cpp` de `rvsource/Xbox/Src` (ramas `_PC`, modo Simulación; con un solo auto, igual que Arcade). Corre en el espacio de Re-Volt (1 unidad = 5 mm, Y abajo, matrices de tres filas) con sus constantes tal cual y lee datos sin traducir (`LegacyLevel`, `CarInfo`); `revolt::convert` pasa sus resultados a Revvy.

Desde la 2.7 el juego no lo usa. Queda como referencia en `crates/physics/tests/revvy_vs_port.rs`, que corre el Calcure en nhood1 en los dos motores con los mismos mandos: el motor de Revvy queda a 1 mm en el asentamiento, a menos de 0,5 mph en toda la aceleración y a pocos grados en el giro. Donde más se separan es en los golpes contra paredes, que resuelve Rapier. Se puede borrar cuando ya no haga falta comparar.

**Comportamiento de referencia.** Es lo que hace el port, y el manejo al que el vehículo de Revvy se tiene que acercar:

- **Mundo.** Polígonos del `.ncp` (`NEWCOLLPOLY`: plano, planos de borde y caja) en la grilla XZ del archivo. Las instancias del `.fin` entran con `RotTransPlane` y se reparten en las celdas con 70 unidades de margen. Los 27 materiales de `COL_MaterialInfo` dan rugosidad (fricción), agarre, dureza (rebote) y corrugado (baches).
- **Auto.** La piel del cuerpo son las esferas del `.hul`, y sus contactos se resuelven juntos con gradiente conjugado. Cuatro ruedas esfera con suspensión (`Stiffness`, `Damping`, `Restitution`, `MaxPos`) y torque que se apaga cerca de `TopSpeed`. La fricción estática y cinética de cada rueda se multiplica por la del material. Las ruedas derrapan (`WHEEL_SPIN`, `WHEEL_SLIDE`). `DownForceMod` actúa con dos ruedas de un lado en el aire, y la resistencia angular crece sin ruedas en contacto.
- **Frame.** Como `gameloop.cpp`: los mandos corren una vez con el `TimeStep` del frame (tope 10/72 s) y después vienen `1 + TimeStep × 150` pasos de colisión y movimiento. El volante responde en cúbico: ×4 al volver al centro o cambiar de lado, ×0.5 al seguir doblando. El voltaje del motor sigue `EngineRate`. Frenar es voltaje negativo más fricción de eje.
- **Largada.** `STARTPOS` y `STARTROT` del `.inf`, más el slot 0 de `CarGridStarts` para el tipo `STARTGRID`.
- **Enderezar.** `R` corre `MOV_RightCar`: solo si el auto está dado vuelta (`up.y ≤ 0.3`) y toca algo. Lo sube 50 unidades y lo pone derecho mirando para donde miraba.
- **Cámara.** `CAM_FOLLOW_BEHIND`, la del jugador en el juego. El palo mide (0, −150, −460) y se estira hacia su largo, choca con el mundo y se acorta si pierde la vista del auto (`LineOfSight`). El FOV sale de `GeomPers` 512 sobre 640×480: unos 50° en vertical, y el horizontal crece con el aspecto de la ventana.
- **Rarezas.** `SteerMod` se lee pero la versión PC no lo usa. Las ruedas derechas copian la matriz de la izquierda. La inercia se invierte sin el ajuste de ejes paralelos. El port tiene una sola corrección: al ubicar el auto, el `CentrePos` de cada rueda arranca en coordenadas de mundo; el original lo deja relativo durante un paso.
- **Fuera del port.** Marcas de derrape, chispas y polvo, la antena, el env map del chasis, choques entre autos y contra el casco convexo, aceite, speedups y catch-up.

### 1.10 Sonido

El sonido también es del motor de Revvy: Kira como salida y un modelo 3D en el espacio de Revvy (metros), con atenuación por distancia y rango, paneo, Doppler y loops que se cortan fuera de rango. Recibe emisores de sonido y el estado de cada vehículo: giro de las ruedas con tracción, derrape y superficie, roce y golpes. No sabe si el contenido vino de Re-Volt.

- **Auto de Re-Volt, traducido.** La clase (eléctrico o nafta) y el `SFXENGINE` de RVGL pasan a los parámetros de sonido del vehículo: sample de motor y curvas de volumen y tono. `SFXENGINE` es una ruta relativa a la raíz de contenido o un archivo en la carpeta del auto. Sin él, va el sample de su clase: `wavs/moto.wav` para eléctricos (el Calcure) o `wavs/petrol.wav`. Un auto propio declara lo mismo en `car.toml`.
- **Nivel de Re-Volt, traducido.** El banco de `wavs/<banco>` (`SfxLevel`: nhood1, nhood2, stunts y nhood1_battle usan `wavs/hood/`) y los objetos del `.fob` que suenan pasan a emisores, con posición en metros; el rango multiplica la distancia a la que se deja de oír (3 m, las 600 unidades de Re-Volt):
  - `3DSOUND` (tipo 49): en loop o aleatorio cada 10–30 s, con su rango.
  - Regadores (tipo 40): un chorro por cada vaivén del cabezal.

  Las pistas `.glb` todavía no tienen dónde declarar emisores; cuando lo tengan, va en `layout.ron`.
- **Comportamiento de referencia** (Re-Volt PC, `sfx.cpp` sobre Miles, rama `OLD_AUDIO`):
  - volumen 0–127 por distancia: `600 × rango / d − 8/127`;
  - paneo según la X del sonido en pantalla (`GeomPers` 512);
  - Doppler según la velocidad relativa: `1024 / (v + 1024)`;
  - motor según `Revs`, la velocidad de rodadura de las ruedas con tracción: en un eléctrico suben el volumen y el tono; en uno a nafta el volumen es fijo y cambia solo el tono;
  - derrape: `skid_normal` o `skid_rough`, según el material donde derrapan más ruedas. Frenar fuerte suena por este canal, porque frenar es torque inverso y las ruedas patinan;
  - roce del cuerpo o del costado de una rueda: `scrape.wav`. Servo mientras el volante se mueve. Golpe fuerte (`BangMag` > 500): `hit2.wav`.

  El volumen maestro es `sfx_volume` en `config/client.toml`: 90 por defecto, igual que `SFX_DEFAULT_VOL`.
- **Implementación.** `client/src/audio/`: `mixer.rs` (el modelo 3D, en metros), `car.rs` (un juego de sonidos por auto, con el `VehicleSound` del motor) y `level.rs` (los emisores de `TrackSounds`). Las curvas del auto son las de Re-Volt, que cuentan la velocidad en sus unidades de 5 mm por segundo; el factor está en un solo lugar.
- **Streaming.** Un paquete de pista legacy necesita su banco de `wavs/`, y `map-packager` tiene que incluirlo (fase 8).
- **Pendiente.** `basketball.wav` y `roadcone.wav` suenan cuando chocan una pelota o un cono, y esos objetos físicos del nivel todavía no existen. También faltan música (MP3 o CD), bocina y sonidos de armas (fase 5).

---

## 2. Arquitectura general

```
┌─────────────────────┐        QUIC (Quinn)         ┌──────────────────────────┐
│   Cliente (PC/Mob)   │◄────────────────────────────►│   Servidor de Juego      │
│  wgpu+winit+bevy_ecs │   inputs / snapshots /       │  (autoridad de sim,      │
│  rapier3d + kira     │   transferencia de mapas     │   salas, bots, matchmk)  │
│  egui (UI/HUD)       │                              │  rapier3d + bevy_ecs     │
└─────────┬────────────┘                              └───────────┬──────────────┘
          │                                                        │
          │ HTTP/REST (reqwest)                                    │ sqlx / redis
          ▼                                                        ▼
┌─────────────────────┐                              ┌──────────────────────────┐
│   API Server (Axum)  │◄────────────────────────────►│  Postgres (stats/cuentas) │
│  auth, catálogo de   │                              │  Redis (cache/presencia) │
│  mapas, leaderboards │                              │  Almacenamiento mapas    │
└─────────────────────┘                              │  (filesystem + manifest) │
                                                       └──────────────────────────┘
```

- **Servidor de juego** (QUIC): sala, mapas, desconexión, countdown. Si `sim_authority: Server`, además corre la física de Revvy (`revvy-physics`, §1.9): inputs → snapshots. Si `Client`, reenvía `VehicleState` del dueño y el host simula bots.
- **API server** (HTTP): todo lo que no es tiempo real — no comparte proceso obligatoriamente con el servidor de juego, pero en una v1 pueden convivir en el mismo binario con routers separados si preferís simplicidad operativa.
- **Almacenamiento de mapas**: filesystem plano + tabla en Postgres con metadata (hash, tamaño, versión, autor). Si en el futuro escalás horizontalmente, migrás a S3-compatible (MinIO) sin cambiar el resto.

---

## 3. Estructura de carpetas (workspace de Cargo)

```
revvy/
├── Cargo.toml                     # workspace root
├── crates/
│   ├── core/                      # revvy-core
│   │   ├── src/
│   │   │   ├── components/        # componentes ECS compartidos (Transform, Velocity, Health, PowerupSlot...)
│   │   │   ├── systems/           # sistemas compartidos cliente/server (movimiento, pickups, checkpoints)
│   │   │   ├── vehicle/           # modelo de física del auto, parámetros desde .inf/.prm
│   │   │   │   └── param_mod.rs   # aplica ParamMod de pista sobre CarDef
│   │   │   ├── rules/             # GameplayRules (RON/TOML), incl. el vector de turbo custom
│   │   │   ├── powerups/          # PowerupKind, PowerupOdds, resolve + track_locks_host_odds
│   │   │   └── lib.rs
│   │
│   ├── formats/                   # revvy-formats: parsers + capa de traducción (legacy Re-Volt + pistas nuevas); afuera, solo tipos de Revvy
│   │   ├── src/
│   │   │   ├── ncp.rs  ├── prm.rs ├── world.rs (.w) ├── vis.rs
│   │   │   ├── fan.rs  ├── cam.rs ├── fin.rs  ├── fld.rs
│   │   │   ├── fob.rs  ├── inf.rs ├── lit.rs  ├── por.rs
│   │   │   ├── pro.rs  ├── bmp.rs ├── pan.rs ├── taz.rs
│   │   │   ├── glb.rs             # lectura de .glb (crate gltf): nodos, mallas, materiales, texturas
│   │   │   ├── gltf_track.rs      # pista revvy-glb-v1: Visual + Collision (superficie = material) + layout.ron
│   │   │   ├── revvy_car.rs       # auto propio: car.toml + body.glb + collision.glb
│   │   │   ├── revolt_car.rs      # traducción: CAR_INFO + .hul → VehicleParams (SI)
│   │   │   ├── revolt_sounds.rs   # traducción: banco del nivel + objetos del .fob → TrackSounds
│   │   │   ├── vehicle.rs         # VehicleParams, WheelParams, ChassisShape, CarSound
│   │   │   ├── sounds.rs          # TrackSounds: banco y emisores
│   │   │   ├── layout.rs          # TrackLayout (zones, AI, POS, pickups, fld, …) y SurfaceType (27)
│   │   │   └── lib.rs             # load_track, load_car, TrackAsset, CarDef
│   │   ├── revolt/
│   │   │   └── CARINFO.TXT        # defaults de Re-Volt (CAR 0-28), solo para autos de Re-Volt
│   │   └── schema/
│   │       └── layout.ron         # ejemplo canónico del sidecar de navegación
│   │
│   ├── net/                       # revvy-net (protocolo compartido)
│   │   ├── src/
│   │   │   ├── messages/          # Input, Snapshot, MapSync, LobbyEvent (Ready, StartRace, …)
│   │   │   ├── quic.rs            # helpers de conexión Quinn compartidos
│   │   │   ├── codec.rs           # (de)serialización postcard + zstd
│   │   │   └── lib.rs
│   │
│   ├── physics/                   # revvy-physics
│   │   ├── src/
│   │   │   ├── world.rs              # PhysicsWorld: Rapier, la pista por grupos, paso fijo, hooks de superficie
│   │   │   ├── vehicle_controller.rs # vehículo de Revvy: ruedas, suspensión, motor, volante, enderezar
│   │   │   ├── camera.rs             # cámara de persecución (como CAM_FOLLOW_BEHIND) contra la pista
│   │   │   ├── surfaces.rs           # perfil de cada SurfaceType (los 27 de COL_MaterialInfo, en metros)
│   │   │   ├── collision_events.rs   # para stats de colisiones
│   │   │   ├── jump.rs               # componente/sistema de salto (flag OFF por defecto)
│   │   │   ├── force_field.rs        # volúmenes de fuerza / gravedad (desde layout o .fld)
│   │   │   ├── revolt/               # port de Re-Volt PC (§1.9): solo referencia en los tests
│   │   │   │   ├── math.rs           # MAT por filas, cuaterniones, planos, BBOX (Geom.cpp)
│   │   │   │   ├── units.rs          # constantes de units.h, newcoll.h, car.h, wheel.h
│   │   │   │   ├── material.rs       # COL_MaterialInfo (27), corrugado, derrape por material
│   │   │   │   ├── coll.rs           # NEWCOLLPOLY, SphereCollPoly, ModifyShift
│   │   │   │   ├── level.rs          # mundo: .ncp + instancias del .fin + grilla, LineOfSight
│   │   │   │   ├── body.rs           # PARTICLE / NEWBODY, contactos del casco
│   │   │   │   ├── conjgrad.rs       # ConjGrad: los contactos del cuerpo juntos
│   │   │   │   ├── car.rs            # CAR: ruedas, suspensión, CarWheelImpulse2, control, enderezar, frame
│   │   │   │   ├── camera.rs         # cámara de persecución (camera.cpp)
│   │   │   │   └── convert.rs        # espacio de Re-Volt ↔ Revvy, para comparar
│   │   │   └── lib.rs
│   │   └── tests/                    # revvy_vs_port.rs (paridad), revvy_content.rs (convivencia), righting.rs
│   │
│   ├── bots/                      # revvy-bots
│   │   ├── src/
│   │   │   ├── waypoints.rs        # AiPath unificado (desde .fan/.pan o layout.ron)
│   │   │   ├── steering.rs
│   │   │   ├── difficulty.rs
│   │   │   └── lib.rs
│   │
│   ├── stats/                     # revvy-stats (modelo compartido de eventos)
│   │   ├── src/
│   │   │   ├── events.rs           # CollisionEvent, PowerupUsedEvent, LapEvent...
│   │   │   ├── race_summary.rs
│   │   │   ├── session_summary.rs  # agregación de 12-16 carreras
│   │   │   └── lib.rs
│   │
│   └── rules_engine/               # revvy-rules (opcional: wrapper sobre Rhai)
│       └── src/lib.rs
│
├── client/
│   ├── Cargo.toml
│   ├── assets/                     # assets del engine (fuentes, UI, mesh del pickup/rayito) — NO tracks/cars
│   └── src/
│       ├── main.rs
│       ├── app/                    # MainMenu, Lobby, Loading, Countdown, Race, Results, TrackEditor
│       ├── drive.rs                # DriveView: pista + autos (Tab cambia el que se maneja) + cámara + sonido
│       ├── render/                 # wgpu: pipelines, materiales, cámara; mesh desde .prm/.w (legacy) o .glb (nuevo);
│       │                           #   una matriz de modelo por dibujo (chasis y ruedas)
│       ├── input/                  # manejo (flechas/WASD, R), cámara libre (C, WASD, mouse, Q/E, Shift). Gamepad pendiente
│       ├── ui/                     # menú, lobby (Listo/Esperando), HUD, stats; room/pickup_odds.rs
│       ├── audio/                  # Kira + modelo 3D (§1.10), en metros
│       │   ├── mixer.rs            # sonidos 3D como SAMPLE_3D / GetSfxSettings3D, en metros
│       │   ├── car.rs              # por auto, como UpdateCarSfx: motor, derrape, roce, servo, golpes
│       │   └── level.rs            # banco y emisores de TrackSounds (loop, aleatorio, regador)
│       ├── network/                # cliente Quinn, reconciliación, cliente HTTP (reqwest) hacia API
│       ├── assets_pipeline/        # descarga; desktop: disco+reserva; mobile: RAM (ver §4)
│       ├── ecs/                    # systems/plugins específicos de cliente (interpolación visual, cámara)
│       ├── editor/                 # SOLO desktop (feature `track-editor`)
│       │   ├── gizmo.rs            # OBB, esferas, líneas, grid, rayitos, flechas de fuerza
│       │   ├── picking.rs          # raycast mouse → nodo/zona/pickup/campo (rapier)
│       │   ├── modes.rs            # TrackZones | AiNodes | PosNodes | StartGrid
│       │   │                       # | Pickups | ForceFields | Surfaces | CarParams | KillVolumes
│       │   ├── odds.rs             # UI egui: tabla de pesos de poderes (capas 3/2/1)
│       │   ├── param_mods.rs       # UI egui: stack Add / Percent / Set sobre stats de auto
│       │   ├── ghost.rs            # grabar vuelta → generar corredor de IA
│       │   ├── validate.rs         # grafo, zonas, grid, odds (pesos ≥ 0, alguno > 0)
│       │   └── save.rs             # escribe layout.ron
│       └── platform/               # diferencias PC vs Mobile (controles táctiles, resoluciones, packaging)
│
├── server/
│   ├── Cargo.toml
│   ├── migrations/                 # sqlx migrations (Postgres)
│   └── src/
│       ├── main.rs
│       ├── game/                   # simulación autoritativa, salas, matchmaking, ciclo de vida de carrera
│       │   ├── room.rs             # lobby, Listo/Esperando, default track, late_join, cache odds
│       │   ├── race_loop.rs
│       │   └── snapshot.rs
│       ├── network/                # servidor Quinn: sesiones, autenticación de conexión
│       ├── api/                    # Axum: routers de auth, mapas, leaderboards, stats
│       │   ├── maps.rs             # listado, upload (admin), enforcement de 100MB
│       │   ├── stats.rs
│       │   └── auth.rs
│       ├── maps_storage/           # almacenamiento físico + manifest, empaquetado zstd
│       ├── db/                     # queries sqlx, modelos
│       ├── cache/                  # wrappers Redis (presencia, salas, rate-limit)
│       └── bots/                   # instanciación de bots server-side (si la sim es autoritativa)
│
├── tools/
│   ├── map-packager/                # CLI: empaqueta una carpeta de pista (legacy O nueva) → .zst + manifest
│   │                                #      valida 100MB, layout.ron completo, hashes
│   ├── car-packager/                # análogo para autos
│   ├── asset-inspector/             # visor/validador de assets (legacy y .glb)
│   ├── test-content/                # generate.py: contenido propio de prueba (revvy_buggy, revvy_arena)
│   └── track-editor/                # binario desktop: mismo código que client/src/editor/
│                                    #      cargo run -p track-editor -- levels/<id>
│
├── dcc/                             # NO corre en el juego; convenciones para artistas
│   ├── blender/
│   │   └── revvy_export.md          # checklist: nodos Visual/Collision, unidades, presupuesto tris, strip UVs en Collision
│   └── blockbench/
│       └── revvy_export.md          # checklist de export GLB (piezas o pista completa)
│
├── docs/
│   ├── protocol.md                  # spec de mensajes de red
│   ├── formats/                     # notas de reverse-engineering por formato (legacy + glTF)
│   ├── layout-schema.md             # spec de TrackLayout / layout.ron
│   └── gameplay-rules-schema.md
│
├── config/
│   ├── server.toml
│   ├── client.toml                  # ventana, content_root, level, car, sfx_volume
│   └── rules/
│       ├── default.ron              # GameplayRules de sala
│       ├── antigrav_turbo.ron
│       ├── powerups.default.ron     # capa 3: pesos default de cada poder (lista por definir)
│       └── surfaces.default.ron     # efectos default por SurfaceType (hielo, dirt, …)
│
└── content/                         # raíz de contenido (legacy y nuevo), con el layout de la carpeta de Re-Volt
    ├── levels/<id>/                 # pistas: de Re-Volt (nhood1) o revvy-glb-v1 (revvy_arena, de prueba)
    ├── cars/<id>/                   # autos: de Re-Volt (parameters.txt, .prm, .hul, TPAGE) o propios
    │                                #   (car.toml + .glb: revvy_buggy, de prueba)
    ├── wavs/                        # sonidos genéricos del legado: moto, petrol, skid_*, scrape, servo, hit2
    │   └── <banco>/                 # sonidos de nivel (hood/ para nhood1, nhood2, stunts…)
    └── gfx/                         # previews de pista del legado (<id>.bmp)
```

**Raíz de contenido.** `content/` es la raíz de contenido de Revvy. Copia el layout de la carpeta de Re-Volt (`levels/`, `cars/`, `wavs/` y `gfx/` hermanas) para que el contenido legacy entre sin cambios; el contenido nuevo usa las mismas carpetas. Así las rutas de `parameters.txt` (`cars/phim_calcure/body.prm`), el `SFXENGINE` de RVGL y los bancos de sonido resuelven sin cambios. En todo este documento, `levels/`, `cars/` y `cache/` (§4) son relativas a esta raíz. Se configura con `content_root` en `config/client.toml`; en una instalación, la raíz es la carpeta del ejecutable. Los `.wav` genéricos se copiaron de `rvsource/wavs` y se versionan acá, porque `rvsource/` está en `.gitignore`.

---

## 4. Ciclo de vida de la sala y streaming de mapas

Las pistas bajadas del **server** no viven en `levels/` del usuario. `levels/` es solo copias **manuales**; **nunca** se borran.

Dónde vive la copia del server:

- **Desktop:** carpeta `cache/server/` en disco, más una reserva de 100 MB (§4.5).
- **Mobile (Android/iOS):** solo en **RAM** del proceso. No hay `reserve.dat` ni `cache/server/` en el almacenamiento del teléfono.

Hay dos “ready” distintos; no mezclarlos:

| Nombre en UI | En código | Significa |
| --- | --- | --- |
| **Esperando** / Waiting | `LobbyStatus::Waiting` | El jugador todavía no confirma que quiere correr. |
| **Listo** / Ready | `LobbyStatus::Ready` | El jugador tocó Listo en el lobby. **No** implica tener la pista en disco. |
| (interno, loading) | `AssetStatus::Ready` | La pista ya está cargada (manual o caché/descarga). |

### 4.1 Lobby (espera, config, pista)

1. **Host crea sala** → API Axum `room_id`, Quinn abre el canal. Estado `Lobby`.
2. **Pista por defecto:** la **primera** del catálogo (`api/maps.rs`, orden del listado). Si el host no elige otra, se corre esa. Se muestra en el lobby; **aún no se descarga**.
3. En esta espera el host edita la **configuración de sesión** (`GameplayRules`: `sim_authority`, vueltas, turbo, odds de pickups si la pista no lockea, `late_join_mode`, `disconnect_bot_replace`, bots, etc.). Los invitados ven los valores. Código: `client/src/ui/lobby.rs` + `server/src/game/room.rs`.
4. Los jugadores se conectan (en paralelo, en cualquier momento del lobby). Cada uno entra en **Esperando**. Bots no tienen Listo.
5. **Host elige pista** (o deja la primera). `LobbyEvent::SetTrack { id, hash, size, version }` a todos. **No** se emite `MapRequired` todavía: no hay descarga en el lobby.
6. Si el host **cambia pista o reglas**, todos los **Listo vuelven a Esperando** (nadie larga con un setup que no confirmó).
7. Cada jugador (no host) pulsa **Listo** → `LobbyEvent::SetReady`. Puede volver a Esperando mientras el host no haya pulsado Iniciar.
8. El botón **Iniciar** del host está **deshabilitado** hasta que **todos** los humanos de la sala estén Listo (el host no necesita el botón Listo: Iniciar es su confirmación). Singleplayer: un solo humano → puede iniciar cuando quiera.

Tras **fin de carrera** (§4.7) vuelven al lobby **con el mapa todavía renderizado de fondo**: todos en **Esperando**, config otra vez editable, pista la última usada (no vuelve al default salvo que el host cree una sala nueva).

### 4.2 Host da inicio → carga / descarga → cuenta 3-2-1

9. Host pulsa **Iniciar** → estado `Loading`. La config de sesión se **congela** hasta volver al lobby.
10. Server manda `MapRequired { id, hash, size, version }` y arranca el timer de **15 s** (§4.3). Cada cliente, en `assets_pipeline/`:
    - `levels/<id>/` con el mismo hash → carga local, `AssetStatus::Ready` (copia manual, disco).
    - Caché de esta sesión con el mismo hash → `AssetStatus::Ready` (rematch). En desktop es `cache/server/<id>/`; en mobile es el buffer RAM *pinned*.
    - si no: descarga `.zst` por stream QUIC dedicado y descomprime en la caché de sesión (`origin: server`). Desktop usa la reserva de disco (§4.5). Mobile asigna el buffer en RAM; si no hay memoria, la descarga falla.
11. Cada cliente reporta `AssetReady`. UI: barra de descarga / “Cargando…”.
12. Cuando **todos** los que van a correr tienen `AssetReady`:
    - se purga la pista server **anterior** si era distinta (§4.4);
    - estado `Countdown`: **3, 2, 1…** (mismo tick en server, se replica a clientes);
    - primer tick de carrera después del 0.

Rematch del mismo `id`+`hash`: el paso 10 es instantáneo (caché o `levels/`).

### 4.3 Timeout de 15 s: espectador o corredor

El host **no** puede saltarse el Listo del lobby. El timeout es solo de **descarga/carga** después de Iniciar.

- A los **15 s** desde `MapRequired`, si falta alguien con `AssetStatus != Ready`, el host ve **Iniciar de todos modos**.
- Si espera, la cuenta 3-2-1 sale cuando el último termina de bajar.
- `GameplayRules.late_join_mode` (config de sala, lobby):
  - `Spectator` — el rezagado **no** spawnea auto; cámara de espectador. El slot no cuenta como corredor (se puede llenar con bot si la sala lo pide).
  - `Racer` — el rezagado **sí** es corredor: se reserva su slot de grilla. Cuando llega `AssetReady`, spawnea aunque la cuenta o la carrera ya hayan empezado (entra en marcha, sin rewind).
- El server **aplica** el modo; el cliente no elige en el momento. Si mandan `AssetReady` después de que el host forzó y el modo es Spectator, se ignora el spawn.

### 4.4 Cuándo se borra la caché del server

Borrar al finish obliga a re-descargar un rematch. La descarga es al **Iniciar**, no al elegir pista.

| Evento | Caché `origin: server` (disco en desktop, RAM en mobile) |
| --- | --- |
| Termina la carrera | Se **queda**. Rematch → `AssetReady` sin bajar. |
| Host elige pista en el lobby | **No** se descarga ni se borra. |
| Host **Iniciar** en la **misma** pista | Carga desde caché. |
| Host **Iniciar** en pista **distinta** | Se baja *incoming* **sin** soltar *pinned*. Máximo **2** copias. |
| Empieza la cuenta 3-2-1 (carrera nueva distinta) | Se suelta la pinned anterior. Incoming pasa a pinned. En desktop se recrea `reserve.dat`. |
| Sale de la sala / el proceso muere | Se sueltan las copias. Desktop deja `reserve.dat`. Mobile no escribe nada a disco. |

`levels/` no entra en esta tabla.

```
lobby: pista default = catálogo[0]
host deja Harbor, todos Listo, Iniciar → bajan Harbor → 3-2-1 → corren → Harbor sigue en cache
vuelven al lobby (todos Esperando)
host Harbor otra vez, Listo, Iniciar → 0 bytes bajados
host elige Docks, todos Listo, Iniciar → bajan Docks (Harbor sigue en la caché de sesión)
3-2-1 de Docks → se suelta Harbor
```

En mobile, “caché” en ese ejemplo es RAM, no una carpeta.

### 4.5 Dónde cabe la descarga: disco en desktop, RAM en mobile

Toda pista del catálogo pesa **≤ 100 MB** ya comprimida (`.zst`). El tope del paquete no cambia.

**Desktop** (`client/src/assets_pipeline/reserve.rs`): el cliente ocupa **100 MB extra en disco** para que la descarga quepa aunque el usuario esté justo de espacio.

- Al arrancar: `cache/reserve.dat` de exactamente 100 MB (`fallocate`, no sparse).
- Si no se puede crear: no entra a online ni a singleplayer con catálogo. `levels/` a mano sigue.
- Al descargar: se borra `reserve.dat` y se escribe `cache/server/<id>/`.
- Al terminar la descarga o al soltar la pista vieja: se recrea `reserve.dat`.

| Estado (desktop) | Disco extra |
| --- | --- |
| Idle / lobby | 100 MB (`reserve.dat`) |
| Post-carrera, pista en caché | ≤ 100 MB pista + 100 MB reserva |
| Loading de otra pista (pinned + incoming) | ≤ 200 MB en pistas, reserva 0 |
| Tras 3-2-1 en la pista nueva | ≤ 100 MB pista nueva + 100 MB reserva |

**Mobile:** no hay reserva en disco ni carpeta de caché. El `.zst` se recibe en un buffer de RAM, se descomprime en RAM y esa copia es la caché de sesión (*pinned* / *incoming*, mismas reglas de §4.4). El SO puede matar el proceso en background: al reabrir hay que volver a bajar. Si `alloc` del paquete falla, esa descarga no entra (mensaje de memoria insuficiente); no se pide permiso de almacenamiento para mapas del server.

Durante la carrera el cliente igual tiene meshes y texturas en RAM/VRAM. La caché mobile es el paquete descomprimido además de eso, como máximo dos pistas (pinned + incoming) y solo entre Iniciar y el 3-2-1 de la siguiente.

No se bajan autos a este slot. Autos en `cars/` son del usuario y, en mobile, siguen siendo archivos locales si el jugador los copió.

### 4.6 Desconexión

El servidor de **sala** (Quinn) existe en ambos modos de autoridad.

| Quién | Qué pasa |
| --- | --- |
| Jugador que **no** es host | Su auto se **detiene e idle** (vel 0, mismo sitio). El slot no se libera. Si se reconecta a la misma sala (mismo `player_id` / token) **retoma** pose, vueltas, poder y standings. |
| Si `GameplayRules.disconnect_bot_replace: true` | Un bot **toma ese auto** y sigue (usa poderes, suma vueltas). Al reconectar, el humano **reemplaza al bot** y hereda el estado que el bot dejó. |
| **Host** se desconecta | **Se cierra la sala.** Todos al menú. No hay migración de host en v1. |

`reconnect_secs` (default 60): pasado el timeout, el idle queda hasta el fin de carrera (o el bot sigue).

### 4.7 Fin de carrera, contramano, volcar

- **Vueltas:** `GameplayRules.laps`. Al completarlas (zona 0 con el contador lleno) el corredor terminó. Cuando terminaron los corredores humanos (o el host corta), `Results` corto y **lobby con el mapa todavía de fondo** (no se re-descarga).
- **Contramano:** orden inverso de TrackZones/POS → HUD popup **«Wrong Way !»** saltante/pulsante (`client/src/ui/hud/wrong_way.rs`). No respawnea solo por eso.
- **Volcar:** keybind que **endereza** el auto (roll/pitch a 0, se mantiene XZ). Si no hay contacto usable, respawn corto en el mismo punto.

### 4.8 Autoridad: `Client` vs `Server`

Opción de sala, se congela al Iniciar.

| | `sim_authority: Client` (amigos) | `sim_authority: Server` (competitivo) |
| --- | --- | --- |
| Física del auto humano | La corre **el dueño**; manda `VehicleState`. Los demás interpolan. | La corre el **servidor** (`revvy-physics`, §1.9). Clientes mandan `Input`, reciben snapshot. |
| Bots | Los simula el **host** y emite `VehicleState`. | Los simula el servidor. |
| Rayitos / poderes / vueltas | Árbitro: **host**. | Árbitro: **servidor**. |
| Auto custom no oficial | Se confía la física del dueño; los demás ven **placeholder** (§6.6.1). | Hull **default** en server; los demás ven placeholder. |
| Cheat / latencia | Mejor feeling; se puede mentir pose. | Peor feeling; pose validada. |

El dedicated server siempre hace lobby, mapas y “host se fue → cierra”. En modo Client no simula la física de 32 autos.

---

## 5. Decisiones de diseño para tus requerimientos específicos

- **Juego propio, retrocompatible con Re-Volt**: el contenido de Re-Volt entra por una capa de traducción al mismo motor que el contenido nuevo, y autos y pistas de los dos orígenes conviven en una sala. No es un port. §0.
- **Crossplay PC/Mobile**: al usar `winit` + `wgpu` para ambos, el core de render/input es el mismo; solo cambia la capa `platform/` (touch controls overlay en egui, distinto scheme de input). El protocolo de red es agnóstico a plataforma.
- **Hasta 32 bots por carrera / llenar cupos vacíos**: `room.rs` cuenta slots; al iniciar instancia bots vía `revvy-bots`. En `sim_authority: Server` el costo es del server; en `Client`, del host.
- **Stats por carrera y sesión**: `revvy-stats` define eventos; el server los persiste al final de cada carrera y agrega por sesión de 12-16 carreras.
- **Salto implementado pero en desuso**: `physics/jump.rs`, flag `allow_jump: bool` default `false`.
- **Reglas de sala**: `GameplayRules` incluye `sim_authority`, `laps`, `late_join_mode`, `disconnect_bot_replace`, vector de turbo, odds de pickup.
- **Compatibilidad de formatos legacy**: `revvy-formats` parsea los binarios originales sin convertir los archivos y los traduce en memoria a tipos de Revvy. Ejes, color key, gouraud y cielo: §6.5.
- **Manejo como Re-Volt**: lo más parecido posible, no idéntico. Lo da el vehículo de Revvy sobre Rapier, el motor de física de todo el juego, con los parámetros traducidos de `parameters.txt`; el comportamiento de referencia sale de `rvsource`. §1.9.
- **Sonido**: motor de Revvy sobre Kira (atenuación, paneo, Doppler) con el comportamiento de `sfx.cpp`; el auto y los objetos de un nivel de Re-Volt entran traducidos. §1.10.
- **Contenido local**: raíz `content/` con `levels/`, `cars/`, `wavs/` y `gfx/` como hermanas, igual que en la carpeta del juego. §3.
- **Pistas nuevas (Blender/Blockbench)**: nunca se leen `.blend` / `.bbmodel`. Ver secciones 6–8.
- **Visual ≠ colisión**: nodo `Collision` low-poly obligatorio. §6.7.
- **Caché de mapas del server**: desktop en disco con reserva de 100 MB; mobile solo en RAM, sin `reserve.dat`. §4.4–4.5.
- **Lobby**: §4.1–4.3. Fin de carrera con mapa de fondo, contramano, volcar: §4.7. Desconexión: §4.6. Autoridad: §4.8.

---

## 6. Formato de pistas nuevas (Blender / Blockbench)

### 6.1 Decisión: glTF 2.0 binario (`.glb`)

Las pistas hechas en Blender o Blockbench se entregan al engine como **glTF 2.0 en contenedor binario `.glb`** (meshes + materiales PBR + texturas embebidas en un solo archivo).

| Candidato | Por qué no es el canónico |
| --- | --- |
| `.blend` / `.bbmodel` | Formatos de autoría. El runtime no los parsea. Quedan en el repo de arte, nunca en `levels/`. |
| `.gltf` + `.bin` + PNGs sueltos | Válido como working copy del artista. `map-packager` los **hornea a un `.glb`** antes de subir, para no romper el límite de 100MB con cientos de archivos ni complicar el streaming. |
| `.obj` / `.fbx` | Sin extras, sin jerarquía usable, FBX es propietario y doloroso en Rust. |
| Re-exportar a `.w` + `.ncp` + `.prm` | El pipeline legacy es para **consumir** tracks de Re-Volt, no para producir tracks nuevas. Convertir glTF → formatos de 1999 pierde PBR, nombres, y obliga a un exporter que nadie quiere mantener. |
| USD | Overkill; no hay exporter de primera en Blockbench. |

**Por qué `.glb` encaja con este stack**

- Blender y Blockbench exportan glTF 2.0 nativo. El artista usa el DCC que le resulte más cómodo; el engine ve el mismo archivo.
- El crate `gltf` (Khronos) carga directo a buffers que `wgpu` ya consume; no hay motor intermedio.
- Un archivo = un hash = un objeto de streaming (`MapRequired { hash }`). Encaja con `assets_pipeline/` y el tope de 100MB.
- La jerarquía de nodos del glTF sirve para separar **Visual** y **Collision** sin un segundo formato de malla.

El crate `revvy-formats::gltf_track` es el único punto que habla con el crate `gltf`. Cliente (render) y server (colisión) piden un `TrackAsset`; no importan si detrás había un `.w` o un `.glb`.

### 6.2 Qué va en el `.glb` y qué no

El `.glb` es **geometría y materiales**. No es el sitio para el grafo de carrera.

| En el `.glb` | Fuera del `.glb` |
| --- | --- |
| Mesh visible (PBR) | TrackZones |
| Mesh de colisión (no se dibuja) | AI Nodes (corredor, racing line, overtaking) |
| Texturas embebidas | Position Nodes y grilla de largada |
| Luces estáticas opcionales (nodos `KHR_lights_punctual`) | Metadata de pista (nombre, vueltas, autor) |
| Props estáticos mergeados o como nodos instanciables | Pickups + odds, campos de fuerza, efectos de piso |

**Por qué no meter zones/nodos como `extras` del glTF**

1. Cada re-export desde Blender/Blockbench **pisaría** los extras si el artista no usa el mismo addon.
2. El grafo se itera **manejando el auto** (ghost lap), no mirando el viewport del DCC.
3. RON se diffea en git; un JSON embebido en un binario no.

### 6.3 Convención de escena (obligatoria al exportar)

Unidades: **1 unidad = 1 metro**. Eje **Y-up**, diestro (spec glTF). Blender ya exporta así si “+Y Up” está activo. Blockbench: export GLB con Y-up.

Nombres de nodos (case-sensitive):

```
Scene
├── Visual          # todo lo que wgpu dibuja (hijos meshes)
├── Collision       # trimesh para rapier; NO se renderiza
│                   # material name = superficie: Road, Dirt, Ice, Grass, Metal, Wood, Sand, … (los 27 de §7.9)
└── Props           # opcional: meshes instanciables (cajas, conos). Si no existe, todo está en Visual.
```

Reglas:

- Tiene que existir al menos `Visual` y `Collision`. Si `Collision` falta, `gltf_track.rs` **rechaza** la pista (no se usa la mesh visible como collider: es cara y no lleva superficies).
- `Collision` es una mesh **distinta y más simple** que `Visual` (equivalente a `.w` vs `.ncp` de Re-Volt). Detalle, presupuestos y tradeoff de tamaño: sección 6.7.
- El material de cada primitiva de `Collision` mapea a un enum `SurfaceType` en `revvy-physics`. Nombre desconocido → `Road`.
- Origin del mundo = origen de la pista. El largada **no** se infiere del origen; se define en `layout.ron`.
- El engine **no** lee custom properties de Blender ni outliner de Blockbench más allá de estos nombres.

Blockbench: si el artista modela **piezas** (módulo de recta, curva, etc.), las ensambla en Blockbench en una escena o las importa a Blender y exporta **un** `.glb` de pista. Revvy no ensambla módulos en runtime en v1.

### 6.4 Carpeta de una pista nueva en `levels/`

```
levels/<track_id>/
├── track.toml          # metadata: name, author, laps, env, preview
├── visual.glb          # escena glTF (Visual + Collision + Props). Nombre fijo.
├── preview.png         # thumbnail del selector (reemplaza el bmp de gfx/ de Re-Volt)
└── layout.ron          # layout de carrera (lo escribe el editor; ver §7)
```

`track.toml` mínimo:

```toml
id = "harbor_night"
name = "Harbor Night"
author = "..."
laps = 3
format = "revvy-glb-v1"    # discrimina de pistas legacy (que no tienen este campo)
env = "night"              # preset de cielo/exposición, no geometría
```

Una pista **legacy** sigue siendo la carpeta Re-Volt de siempre (`.w`, `.ncp`, `.fan`, `.pan`, `.taz`, `.inf`, …) **sin** `track.toml` con `format = "revvy-glb-v1"`. El loader en `revvy-formats` decide:

1. Si existe `track.toml` con `format = "revvy-glb-v1"` → pipeline glTF + `layout.ron`.
2. Si no, busca `.inf` + `.w` + `.ncp` → pipeline legacy.

No se mezclan en la misma carpeta.

Hoy `gltf_track.rs` lee `track.toml` (la escena es `visual.glb`, o la que diga `visual`), dibuja `Visual` y `Props`, choca con `Collision` (la superficie es el nombre del material) y de `layout.ron` toma por ahora solo `start_grid`. `extras.revvy.collider` todavía no se lee: toda la colisión es malla de triángulos. La pista de prueba es `content/levels/revvy_arena`.

### 6.5 Coordenadas y traducción legacy

- Re-Volt: diestro, **Y hacia abajo**, +X derecha, +Z adelante (`DownVec`, `LookVec`).
- glTF / Revvy interno: diestro, **Y hacia arriba**, +Z adelante.
- Una unidad de Re-Volt mide 5 mm (`REVOLT_TO_METERS = 0.005`, §1.9). Hasta la fase 2 se usó 1 cm, y todo salía al doble de tamaño.

Negar solo Y deja el mundo zurdo y el mapa espejado (en nhood1 la primera curva sale a la izquierda). La conversión niega **X e Y**: es un giro, la derecha sigue siendo la derecha. `axes::position` y `axes::direction` hacen esa cuenta. Las matrices del `.fin` se aplican en espacio de archivo con la misma multiplicación que el `.prm` (`mul_rows`) y después pasan por `position`. El yaw de `STARTROT` queda `-turns · τ`: Re-Volt ubica el auto con `RotMatrixY(-turns)` y el giro de ejes no cambia el ángulo (`axes::yaw_from_turns`).

La vista de manejo (`client/src/drive.rs`, `DriveView`) carga la pista y los autos y los pone en el motor de Revvy; solo usa tipos de Revvy. La conversión de esta sección la hace la capa de traducción al cargar. Los autos van en los puestos de la grilla: primero los de los jugadores de la sala, después `extra_cars` de `config/client.toml`. `cargo run -p revvy-client -- <pista> <auto> [más autos…]` arranca directo en la carrera; Esc vuelve a la sala del menú.

Teclas:

- **↑/W** acelera, **↓/S** frena y da reversa, **←/A** y **→/D** doblan.
- **R** endereza el auto, solo si está dado vuelta (como `MOV_RightCar`).
- **Tab** cambia el auto que se maneja; los demás quedan quietos.
- **C** alterna entre la cámara de persecución y la libre. En la libre, **WASD** mueve, **Q/E** bajan y suben, el mouse gira y **Shift** acelera; las flechas siguen manejando.

#### Colisión de instancias

`BuildInstanceCollPolys` no tiene una lista de props. Cada nombre del `.fin` busca `<nombre>.ncp` en la carpeta del nivel. El `.fin` guarda 8 caracteres: `WHITEPOS` es `whitepost`, `BARRIERP` es `barrierpole`. Si no hay `.ncp` (casas, aros, asientos), esa instancia no choca, igual que en el juego. Un `.ncp` vacío no aborta la pista.

#### Dibujo del mapa legacy

Solo `TrackAsset.visual`. El `.ncp` no se dibuja.

- **Color key** (`texture.cpp`, clave RGB 0): en las texturas de pista, un texel negro queda con alpha 0 y el shader lo descarta. El resto de la cara se dibuja. No aplica a pistas `.glb` ni a autos.
- **Gouraud negro no es color key.** El techo del túnel de nhood1 tiene vértices en `0,0,0` y textura con color. Re-Volt lo modula a negro y lo dibuja. Revvy también: si se omite la cara, el túnel queda abierto.
- **Luz global:** `DrawCubePolys` pinta `textura × color de vértice`. No hay sol ni hemisferio encima. En nhood1 `WORLDRGBPER` es 100, así que el gouraud del archivo entra tal cual. `.lit` sigue sin usarse.
- **Cielo:** `RenderSkybox` pega `sky_ft`, `sky_rt`, `sky_bk`, `sky_lt`, `sky_tp`, `sky_bt` en +Z, −X, −Z, +X, arriba y abajo del archivo. Tras el giro de ejes, el cubemap es +X `sky_rt`, −X `sky_lt`, +Y `sky_tp`, −Y `sky_bt`, +Z `sky_ft`, −Z `sky_bk`.

#### Autos legacy

`load_car` lee `parameters.txt`. Las claves que faltan salen del bloque `CAR 0-28` de `CARINFO.TXT` (`merge_stock_defaults`). Es el archivo de Re-Volt, versionado en `crates/formats/revolt/` y compilado dentro de `revvy-formats`. Esos defaults son solo para autos de Re-Volt: una carpeta con `car.toml` es un auto propio y `load_car` la rechaza (§6.6). `WHEEL 0 - 3` se expande como `ReadNumberList`. `Inertia` sigue en las dos líneas de abajo, igual que en `ReadMat`. Las líneas `;)` de RVGL se leen (§1.5).

`load_car` devuelve:

- `vehicle`: los parámetros del vehículo de Revvy, traducidos según su dimensión (§1.9). Las esferas del `.hul` pasan a la piel del chasis y sus cascos convexos, a los choques entre autos.
- `sound`: la clase y el `SFXENGINE`.
- El chasis.
- Una malla por rueda, según el `ModelNum` de cada `WHEEL`.
- La `TPAGE`, que se dibuja sin color key.
- `revolt`: el `CAR_INFO` crudo y las esferas del `.hul` en unidades de Re-Volt. Solo los usa el port de referencia.

Cómo se dibuja, igual que `DrawCar`:

- El chasis va en el centro de masa + `body_offset`, con la rotación del cuerpo.
- Cada rueda va en su anclaje más el recorrido de suspensión, con `cuerpo · RotY(volante) · RotX(giro)`. Así gira con la velocidad y dobla con el volante.
- Las ruedas derechas copian la orientación de la izquierda de su eje, como en el original.

### 6.6 Autos y props desde Blockbench

Blockbench es el DCC natural para **autos y props** (low-poly, UV, animaciones simples). Mismo contrato: export `.glb`. Un auto nuevo:

```
cars/<car_id>/
├── car.toml            # name, body, collision, [sound], [vehicle] (VehicleParams en SI)
├── body.glb            # chasis + nodos WheelFL, WheelFR, WheelBL, WheelBR (centrados en el buje)
└── collision.glb       # nodos Sphere* (tocan el mundo) + cascos convexos (contra otros autos)
```

Un auto propio trae en `car.toml`, en la tabla `[vehicle]`, los parámetros del vehículo de Revvy (§1.9). Es el mismo tipo que sale de traducir un `parameters.txt`, así que los dos corren en el mismo motor y en la misma carrera. Están pensados para manejarse lo más parecido posible a Re-Volt, sin código de Re-Volt, y nunca usan `CARINFO.TXT` ni otros datos de Re-Volt, tampoco como default: con `car.toml` en la carpeta, `load_car` no mira un `parameters.txt`. La forma de choque sale de `[vehicle.chassis]`, si no de `collision.glb`, y si no del casco del chasis visible (peor). `[sound]` elige el tipo de motor (`electric` o `petrol`) y un sample propio opcional. La escala es la de los autos de Re-Volt, que son de radiocontrol (el Calcure mide 68 cm), para que convivan en la misma pista. El auto de prueba es `content/cars/revvy_buggy`, que genera `tools/test-content/generate.py`. El legado `.prm` + `parameters.txt` no se toca.

#### 6.6.1 Autos custom vs catálogo oficial

El catálogo oficial es el set de `car_id` publicado en el server (`api` / `cars/` del pack del juego). Un jugador puede elegir un auto de su `cars/` local que **no** esté en ese set.

- **Dueño:** ve y siente su auto (mesh + `car.toml` / `.inf` propios).
- **Todos los demás** (lobby y carrera): mesh `client/assets/cars/placeholder.glb`. A la **derecha del nombre**, tanto en la lista de la sala como en el nametag 3D sobre el auto: **`! Custom Car`**.
- Física remota:
  - `sim_authority: Client` — se acepta el `VehicleState` del dueño (el custom se siente).
  - `sim_authority: Server` — el server usa hull **default** (`cars/_default/`), no el `.inf` custom (anti-hitbox). El dueño sigue viendo su mesh; el resto, placeholder.

No se descarga el `.glb` custom a los demás en v1.

### 6.7 Mesh visual vs mesh de colisión (equivalente a `.w` / `.ncp`)

Re-Volt no conduce sobre lo que se ve: `.w` es el aspecto, `.ncp` es una geometría más simple (poliedros / planos) pensada para física. Revvy hace **lo mismo** en pistas nuevas. No es un extra opcional: es el contrato de `gltf_track.rs`.

| Re-Volt | Revvy (pista nueva) | Quién lo consume |
| --- | --- | --- |
| `.w` (mundo visible) | nodo `Visual` dentro de `visual.glb` | cliente wgpu |
| `.ncp` (colisión simple + superficies) | nodo `Collision` dentro del **mismo** `visual.glb` | cliente y server, `rapier3d` TriMesh / colliders primitivos |
| superficies en el `.ncp` (asfalto, hielo, …) | nombre del material en primitivas de `Collision` → `SurfaceType` | `revvy-physics` |

El cliente **nunca** dibuja `Collision`. El server **nunca** construye buffers wgpu de `Visual`: `gltf_track.rs` expone `TrackAsset { visual: Option<…>, collision: CollisionMesh }`; el binario server pide solo `collision`.

Una pista de Re-Volt llega al mismo lugar por la capa de traducción: su `.ncp` pasa a un `TriMesh` de Rapier y cada material a una superficie de Revvy (§1.9). Desde ahí, el motor no distingue una pista de la otra.

#### Por qué no usar la mesh visible como collider

Una pista “bonita” tiene molduras, vallas con barrotes, árboles, bordes biselados, miles de tris de adorno. Meter eso en Rapier con 32 autos (server autoritativo) multiplica el costo de *narrow phase* cada tick. Además la mesh visible no lleva `SurfaceType` por primitiva de forma fiable (un atlas PBR no es “Ice” vs “Road”). Por eso, si `Collision` falta, la pista se rechaza: no hay fallback silencioso a `Visual`.

#### Cómo se modela `Collision` (punto de equilibrio)

No es un *decimate* genérico de `Visual`. El decimate destroza saltos, peraltes y el plano de conducción. Se modela **a propósito**, en el DCC, con esta prioridad:

1. **Superficie de manejo** — la única parte que puede acercarse en detalle a lo visual. Conservar curvatura de jumps, banks y whoops. Acá un collider demasiado grosero se *siente* (ruedas flotan, el auto “sube escalones”).
2. **Muros / límites de pista** — planos, cajas o un extrude de baja densidad. No los ladrillos ni las rejas del render.
3. **Edificios, público, vegetación, props de adorno** — **fuera** de `Collision`, o un cubo/convex por prop si el auto puede chocarlos. El árbol de 4k tris visual es un cilindro.
4. **Detalles más chicos que una rueda** (tornillos, ranuras, basura) — no existen en colisión.

Formas Rapier, en este orden de preferencia (más barato → más caro):

- `Cuboid` / `Cuboid` orientado para muros y props.
- `ConvexHull` para obstáculos irregulares simples.
- `TriMesh` **estático** solo para el asfalto / terreno manejable.

`gltf_track.rs` decide el collider según extras del nodo hijo de `Collision`:

- `extras.revvy.collider = "trimesh" | "cuboid" | "convex"` (default `trimesh` en la raíz si no hay hijos).
- Hijos con cuboid: el engine toma el AABB de la mesh (o `half_extents` en extras) y **no** construye TriMesh.

En Blender: colección `Collision` con meshes low-poly. En Blockbench: una layer/group `Collision` aparte, cubos para muros, plano/subdiv baja para el piso; exportar todo en el mismo GLB.

#### ¿Duele el tamaño de descarga?

Sí hay dos geometrías en el paquete, **pero el costo extra es pequeño si `Collision` está bien hecha**. Lo que hincha el `.glb` (y el `.zst`) son las **texturas embebidas** de `Visual`, no los índices de un collider.

Orden de magnitud (orientativo, una pista típica):

| Contenido | Peso relativo en el `.glb` sin comprimir | Después de `zstd` en `map-packager` |
| --- | --- | --- |
| Texturas PBR de `Visual` | ~70–90 % | sigue dominando |
| Geometría `Visual` (pos + normal + uv + tangents) | ~10–25 % | baja bien (meshopt) |
| Geometría `Collision` (solo `POSITION` + índices + material/superficie) | **~0.5–5 %** si se cumple el presupuesto de abajo | casi ruido |
| `layout.ron` | despreciable | despreciable |

Si el artista duplica `Visual` entero como `Collision` (mismos tris, con UVs y normales), el extra de geometría **sí** se nota: puede acercarse a otro 10–25 % del GLB. Eso es un error de autoría, no el diseño. `map-packager` lo trata como fallo (ver presupuesto).

Separar en `visual.glb` + `collision.glb` **no reduce** la descarga del cliente: el jugador necesita ambos. Solo ahorraría parseo/RAM en el **server** (que ya no construye `Visual`). En v1 un solo `visual.glb` con dos nodos es más simple para el hash/streaming; el server igual ignora el nodo `Visual` al cargar. No vale la pena un segundo archivo solo por tamaño.

#### Presupuesto (el punto equilibrado, enforceable)

Objetivo: **misma técnica que Re-Volt, sin pagar un segundo mapa a precio de geometría hi-poly**.

| Métrica | Objetivo | `map-packager` |
| --- | --- | --- |
| Tris de `Collision` vs tris de `Visual` | **≤ 15 %** | warn 15 %, **error** si ≥ 50 % (casi seguro copiaron el visual) |
| Atributos en primitivas `Collision` | solo `POSITION` (+ material/`COLOR_0` si hace falta superficie) | warn si hay `TEXCOORD_*`, `NORMAL`, `TANGENT`, `JOINTS_*` |
| Payload `Collision` vs tamaño del `.glb` | **≤ 5 %** | warn |
| Paquete total `.zst` | **≤ 100 MB** (regla ya existente) | error |
| Superficie manejable | TriMesh; no convex único de toda la pista | error si la raíz `Collision` es un solo convex (el auto se desliza mal) |

Números de trabajo para el artista (no hardcode de engine, guía en `dcc/*/revvy_export.md`):

- Pista media: `Visual` 80k–250k tris, `Collision` 8k–25k tris.
- Muros: decenas de cajas, no miles de tris.
- El piso puede ser *más* denso que los muros; el adorno visual no paga física.

En el DCC, exportar `Collision` **sin** UVs/normales (Blender: desactivar “UVs” / “Normals” en el exporter para esa colección, o un segundo pass de glTF que strippee atributos). `gltf_track.rs` ignora esos atributos si vinieran igual; `map-packager` avisa para no inflar el archivo.

Opcional en `map-packager` (Visual only, no Collision): meshopt + quantize (`KHR_mesh_quantization`) sobre `Visual`. **No** cuantizar `Collision`: Rapier necesita `f32` estable en el plano de manejo.

#### Desventajas reales (no el peso)

Más importantes que el download:

- **Desajuste visual/físico** — valla visible que no choca, o muro invisible. Mitigación: en `track-editor`, overlay wireframe de `Collision` sobre `Visual` (toggle, default on). El artista ve el hueco antes de publicar.
- **Superficies mal asignadas** — material con nombre libre → todo `Road`. Mitigación: enum cerrado (`Road`, `Dirt`, `Ice`, `Grass`, `Metal`, `Wood`, `Sand`); `map-packager` lista primitivas sin match.
- **Costo de autoría** — hay que modelar dos veces el “volumen” de la pista. Es el mismo costo que en Re-Volt; Blockbench lo baja si los muros son cubos.
- **No hay ahorro de descarga mágico** — dos meshes pesan más que una. El equilibrio es **no copiar el hi-poly** y **strippear atributos**, no eliminar `Collision`.

#### Qué no se hace

- No generar `Collision` automático por decimate en el engine (v1). Puede existir más adelante un botón en `track-editor` “crear piso desde Visual + voxel walls” como *punto de partida*, nunca como asset final sin revisar.
- No mezclar `Visual` y `Collision` en el mismo nodo.
- No mandar `Collision` por un stream distinto al cliente en v1: el `.zst` ya va comprimido y el extra es mínimo.

---

## 7. Editor in-game: layout de carrera

Zonas, nodos, **pickups**, **campos de fuerza**, **efectos de piso** y **mods de parámetros de auto** se editan aquí, no en Blender/Blockbench.

### 7.1 Decisión: editor in-engine (desktop), no plugin de Blender ni de Blockbench

La herramienta canónica es un **modo editor del cliente desktop**, el equivalente a MAKEITGOOD de Re-Volt. Vive en `client/src/editor/` y se lanza también como binario `tools/track-editor`.

**No** se implementa como plugin de Blender. **No** se implementa como plugin de Blockbench. **No** se shippea en Android/iOS.

| Opción | Veredicto | Motivo |
| --- | --- | --- |
| Plugin Blockbench | Descartado | Blockbench es modelador de piezas/personajes, no editor de mundo a escala de pista. No hay gizmos decentes para un grafo de cientos de nodos, OBB orientados, racing line vs overtaking line, ni física del auto. |
| Plugin Blender | Descartado como herramienta canónica | Blender *sí* puede colocar empties y curvas, pero no corre la física del juego, no genera la racing line desde una vuelta fantasma, y deja fuera al artista que trabaja solo en Blockbench. Cada re-export del `.glb` pelearía con datos guardados como extras. Mantener un addon Python contra versiones de Blender es un segundo producto. |
| Editor in-game en el cliente de jugador (mobile incluido) | Descartado | Infla el binario mobile, mete UI de artista en el loop de carrera, y no se puede usar con el dedo lo que en Re-Volt ya era denso con mouse. |
| **Modo editor desktop (`client/src/editor/` + `tools/track-editor`)** | **Canónico** | Misma wgpu, misma colisión, mismo auto. El artista *maneja* la pista para validar. Un `.glb` sale igual de Blender o de Blockbench; el editor no pregunta de dónde vino. Feature `track-editor` / `cfg(not(mobile))`. |

Esto copia el flujo que ya funciona en Re-Volt (POS → Track Zones → AI Nodes → objetos pickup / fields, colocados *sobre* la pista jugable), no el flujo de un DCC.

### 7.2 Dónde se implementa (crates y binarios)

```
client/src/editor/          # código del editor (lib interna, no se compila en mobile)
  gizmo.rs                  # OBB, esferas L/R, racing/overtaking, grid, rayito, flecha de fuerza
  picking.rs                # ray desde cámara contra gizmos
  modes.rs                  # … | Surfaces | CarParams | KillVolumes
  odds.rs                   # panel egui de PowerupOdds (capas 3 / 2 / 1)
  param_mods.rs             # stack Add / Percent / Set (global o por auto)
  ghost.rs                  # Time Trial → resample a nodos
  save.rs                   # serializa TrackLayout → levels/<id>/layout.ron
  validate.rs               # grafo, zonas, grid, pesos de odds, campos, param_mods

tools/track-editor/         # cargo run -p track-editor -- --track levels/harbor_night

crates/formats/src/layout.rs
                            # TrackLayout { …, param_mods: [ParamMod] }

crates/core/src/powerups/   # PowerupKind, PowerupOdds, resolve_odds, track_locks_host_odds
crates/core/src/vehicle/param_mod.rs
                            # apply_param_mods(car, layout) → CarDef efectivo
crates/core/src/systems/    # RaceProgress + PickupSystem
crates/physics/src/surfaces.rs
crates/physics/src/force_field.rs
crates/bots/src/waypoints.rs
```

Activación:

- Menú desktop del cliente: **Editor de pista** (solo si `cfg(feature = "track-editor")`).
- O CLI: `track-editor --track levels/<id>` abre directo en el modo editor, sin lobby ni net.
- Mobile: el módulo `client/src/editor/` **no se compila**. Feature `track-editor` en `client/Cargo.toml`, activada por defecto en el binario desktop y **ausente** en los builds de `cargo-mobile2`. El código va detrás de `#[cfg(feature = "track-editor")]`.

El servidor de juego **nunca** incluye el editor. Solo consume `layout.ron` ya escrito (igual que consume `.fan` en legacy).

### 7.3 Cómo se usa (flujo del artista)

Orden obligatorio, igual que MAKEITGOOD. El editor bloquea el modo siguiente si el actual no valida.

1. Exportar `visual.glb` desde Blender o Blockbench a `levels/<id>/` y crear `track.toml`.
2. Abrir `track-editor --track levels/<id>`. Carga el `.glb` con el mismo renderer/física que una carrera.
3. **StartGrid**: colocar N transforms (posición + yaw) en la línea de largada. Mínimo 1, pensado para 32. Se guardan como `start_grid: [Transform]`.
4. **PosNodes**: colocar esferas a lo largo del *racing path* (no el corredor). Cada nodo tiene hasta N links prev/next (sin el tope rígido de 4 de `.pan`; default práctico 8). Al guardar, `save.rs` calcula `distance` acumulada y `total_distance`. El `start_node` es el nodo de meta/largada.
5. **TrackZones**: cajas OBB (centro, quat, half-extents) con `id` secuencial. La zona `0` es meta. Cubren el asfalto jugable; el auto *debe* poder atravesarlas en orden para completar una vuelta. Sirven para posición de carrera, wrong-way, y respawn (se asocia al PosNode más cercano).
6. **AiNodes**: cada segmento es un par **verde (izquierda) / rojo (derecha)** que define el corredor. Al conectar dos segmentos aparece la **racing line** (offset `t ∈ [0,1]` entre L y R) y la **overtaking line** (otro `t`). Propiedades por segmento: `Racing`, `Slowdown`, `SpeedLimit(f32)`, `PickupRoute` (la IA se desvía hacia rayitos), `Careful`, `WallLeft`, `WallRight`. Atajos = ramas extra en `next[]`.
7. **Pickups (rayitos)**: colocar **posiciones** (clic en el piso). Mesh global `client/assets/pickups/bolt.glb`. En el panel egui de este modo se editan las **probabilidades de poderes** (capas 3 → 2 → 1, ver §7.7). No bloquea los modos anteriores.
8. **ForceFields**: OBB con un vector / escala de gravedad, equivalente a `.fld`. Ver §7.8.
9. **Surfaces**: (a) tabla de efectos por tipo de piso de toda la pista; (b) volúmenes opcionales que fuerzan un tipo (p.ej. un parche de hielo) sin re-exportar el `.glb`. Ver §7.9.
10. **CarParams**: stack de modificadores sobre stats de auto. Ver §7.10.
11. **KillVolumes**: OBB que al **tocarlos** respawnean (pozos, vacío). Ver §7.12. El “área jugable” son las TrackZones: salir de todas ellas también respawnea.
12. Opcional: **Ghost**. Time Trial, `G` resamplea AiNodes.
13. Guardar (`Ctrl+S`) → `layout.ron`. `validate.rs` debe pasar o no se escribe.
14. `map-packager` vuelve a validar al generar el `.zst`.

Cámara del editor: flycam (WASD + mouse) **y** cámara de auto. Colocar nodos “a pie” y validarlos manejando. Raypick con LMB; gizmos de traslación/escala/rotación en wgpu (no egui-gizmo sobre la escena: egui solo paneles laterales de propiedades).

### 7.4 Semántica (mapeo 1:1 con Re-Volt, formato propio)

No se escriben `.taz` / `.pan` / `.fan` para pistas nuevas. Se escribe **un** sidecar con la misma semántica, sin los límites hardcoded de 1999.

| Concepto Re-Volt | Archivo legado | En Revvy (pistas nuevas) | Lo usa |
| --- | --- | --- | --- |
| Track Zones (cajas, orden de vuelta) | `.taz` | `layout.ron` → `zones: [TrackZone]` | `core` (vueltas, posición, wrong-way, respawn) |
| POS Nodes (path + distancia a meta) | `.pan` | `layout.ron` → `pos_nodes: [PosNode]` | `core` (standings, largo de pista) |
| AI Nodes (verde/rojo, racing + overtaking) | `.fan` | `layout.ron` → `ai_nodes: [AiNode]` | `revvy-bots` |
| Grilla de largada | `.inf` / start pos | `layout.ron` → `start_grid` | `server/game/room.rs` al spawnear |
| Pickup / rayito (posición en pista) | `.fob` (objeto pickup) | `layout.ron` → `pickups` + `pickup_odds` | `core/powerups` (spawn, sorteo, poder) |
| Campo de fuerza / viento / gravedad | `.fld` | `layout.ron` → `force_fields` | `physics/force_field.rs` |
| Efecto de tipo de piso (hielo, dirt, …) | superficie en `.ncp` | `layout.ron` → `surface_effects` + `surface_volumes` | `physics/surfaces.rs` |
| Mods de stats de auto en esta pista | (a veces `.inf` de pista) | `layout.ron` → `param_mods` | `core/vehicle/param_mod.rs` |
| Kill / fuera de pista (respawn) | triggers / caerse | `layout.ron` → `kill_volumes` + TrackZones + world AABB | `core` respawn |

Esquema de `layout.ron` (el tipo Rust vive en `crates/formats/src/layout.rs`):

```ron
TrackLayout(
    version: 1,
    start_node: 0,
    total_distance: 1240.5,           // metros, calculado al guardar
    start_grid: [
        (pos: (x: 0.0, y: 0.5, z: 0.0), yaw: 0.0),
        // ... hasta 32
    ],
    zones: [
        TrackZone(
            id: 0,
            center: (x: 0.0, y: 1.0, z: 0.0),
            rotation: (x: 0.0, y: 0.0, z: 0.0, w: 1.0),  // quat
            half_extents: (x: 8.0, y: 4.0, z: 6.0),
        ),
        // id 1, 2, ... recorren la pista; el último conecta de vuelta a 0
    ],
    pos_nodes: [
        PosNode(
            id: 0,
            position: (x: 0.0, y: 0.5, z: 0.0),
            distance: 0.0,
            prev: [3],
            next: [1],
        ),
        // ...
    ],
    ai_nodes: [
        AiNode(
            id: 0,
            left:  (x: -4.0, y: 0.5, z: 0.0),   // esfera verde
            right: (x:  4.0, y: 0.5, z: 0.0),   // esfera roja
            racing_t: 0.5,                      // 0 = left, 1 = right
            overtaking_t: 0.72,
            prev: [12],
            next: [1],
            flags: Racing,                      // o Slowdown, PickupRoute, Careful, ...
            speed_limit: None,                  // Some(m/s) si flags incluye SpeedLimit
        ),
        // ...
    ],
    pickups: [
        PickupSpawn(
            pos: (x: 12.0, y: 0.6, z: -4.0),
            respawn_secs: 5.0,
            odds: None,                         // None = usa capa 2 o 3. Some(...) = capa 1
        ),
        PickupSpawn(
            pos: (x: 40.0, y: 0.6, z: 8.0),
            respawn_secs: 5.0,
            odds: Some(PowerupOdds(             // capa 1: solo ESTE rayito
                weights: [(kind: Fireball, w: 80), (kind: Battery, w: 20)],
            )),
        ),
    ],
    pickup_odds: None,                          // None = capa 3 (engine). Some(...) = capa 2, toda la pista
    force_fields: [
        ForceField(
            id: 0,
            center: (x: 0.0, y: 2.0, z: 80.0),
            rotation: (x: 0.0, y: 0.0, z: 0.0, w: 1.0),
            half_extents: (x: 12.0, y: 8.0, z: 12.0),
            kind: GravityScale(0.35),           // o Wind((x,y,z)) | ConstantForce((x,y,z))
        ),
    ],
    surface_effects: Some({                     // None = defaults de config/rules/surfaces.default.ron
        Ice: SurfaceEffect(
            friction: 0.12,
            lateral_grip: 0.22,
            rolling_resist: 0.01,
            speed_factor: 1.05,
        ),
        // tipos omitidos = default del engine
    }),
    surface_volumes: [
        SurfaceVolume(                          // opcional: este OBB se comporta como Ice
            center: (x: 5.0, y: 0.5, z: 20.0),
            rotation: (x: 0.0, y: 0.0, z: 0.0, w: 1.0),
            half_extents: (x: 6.0, y: 2.0, z: 10.0),
            surface: Ice,
        ),
    ],
    param_mods: [
        ParamMod(target: All, op: Add, stat: Engine, surface: None, value: -0.5),
        ParamMod(target: All, op: Percent, stat: Engine, surface: None, value: -5.0),
        ParamMod(target: All, op: Set, stat: Engine, surface: None, value: 50.0),
        ParamMod(target: Car("calcure"), op: Add, stat: Engine, surface: None, value: -0.5),
        ParamMod(target: All, op: Percent, stat: Grip, surface: Some(Ice), value: -5.0),
    ],
    kill_volumes: [
        KillVolume(
            center: (x: 0.0, y: -20.0, z: 0.0),
            rotation: (x: 0.0, y: 0.0, z: 0.0, w: 1.0),
            half_extents: (x: 200.0, y: 5.0, z: 200.0),  // pozo / vacío
        ),
    ],
)
```

Límites: sin cap de 1024 nodos. `map-packager` avisa si `visual.glb` + `layout.ron` superan 100MB (el RON es despreciable; el tope lo manda el GLB).

### 7.5 Runtime: cómo se lee

Al cargar una pista `revvy-glb-v1`:

1. `gltf_track.rs` construye meshes wgpu (nodo `Visual`) y `rapier` TriMesh (nodo `Collision` + `SurfaceType`).
2. `layout.rs` deserializa `layout.ron`. Si falta o `validate` falla, la pista **no entra a carrera** (el editor sí puede abrirla a medio hacer).
3. `core` registra:
   - cada `TrackZone` como collider sensor (query) para saber en qué sector está el auto;
   - el grafo `pos_nodes` como `RacePath` (posición relativa / vueltas);
   - `start_grid[i]` como spawn del slot `i`;
   - cada `PickupSpawn` como entidad con sensor (mesh `client/assets/pickups/bolt.glb`). Si el auto tiene el **slot vacío**: `resolve_odds` → llena el slot, se oculta el rayito, respawn a `respawn_secs`. Si el slot **está lleno**: no hay overlap de pickup (el auto **traspasa** el rayito, no cambia el poder). Ver §7.7.2;
   - `kill_volumes` como sensors: on enter → respawn (§7.12);
   - cada `ForceField` como sensor de volumen; `physics/force_field.rs` aplica la fuerza/gravedad a los autos que solapan;
   - `surface_effects` pisa los defaults de `surfaces.default.ron`; el contacto rueda usa `SurfaceType` del mesh, salvo que un `surface_volumes` contenga el punto de contacto.
   - al spawnear cada auto, `apply_param_mods(car_def, layout.param_mods)` produce el `CarDef` efectivo (§7.10). Los mods con `surface: Some(...)` no se hornean: se aplican en el tick si el contacto coincide.
4. `revvy-bots` construye `AiPath` interpolando `left`/`right`/`racing_t`/`overtaking_t` y las `flags` por segmento. Steering no conoce RON ni glTF. La flag `PickupRoute` solo sesga la línea hacia rayitos; **no** coloca pickups.

En pistas legacy, `fan.rs` / `pan.rs` / `taz.rs` / `fob.rs` / `fld.rs` **adaptan** al mismo `TrackLayout` en memoria (pickups del `.fob` → `pickups`; `.fld` → `force_fields`; superficies del `.ncp` → `SurfaceType` en el collider). A partir de ahí el código de carrera es uno solo.

### 7.6 Lo que no hace el editor en v1

- No edita la mesh de la pista (no es un modelador). Si hay que mover una pared, se vuelve a Blender/Blockbench, se re-exporta el `.glb`, se reabre el editor; `layout.ron` se conserva.
- No modela el rayito: el mesh es único y vive en `client/assets/pickups/`. El editor instancia posiciones y edita **odds**.
- Overlay wireframe de `Collision` sobre `Visual` (toggle) para cazar desajustes visual/físico; no genera la mesh de colisión.
- No reemplaza `map-packager` (upload / zstd / límite 100MB / presupuesto Visual vs Collision).
- No corre en el server ni en mobile.
- Visiboxes y cámaras de replay: mismo `layout.ron` más adelante; no están en v1.

### 7.7 Probabilidades de poderes (3 capas de pista + host)

Al recoger un rayito se sortea un `PowerupKind`. La lista de poderes y los pesos **default están por definir**; el tipo es un enum cerrado en `crates/core/src/powerups/kind.rs` (Battery, Shockwave, Fireball, Oil, … — nombres finales TBD). Los pesos viven en datos, no en `match` hardcodeados.

**Prioridad de autoría de pista (el número más bajo gana):**

| Capa | Alcance | Quién la escribe | Dónde vive | En el editor de pista |
| --- | --- | --- | --- | --- |
| **3** (más débil) | Global: **todos** los rayitos | Engine | `config/rules/powerups.default.ron` | Panel de Pickups, **solo lectura** (“defaults”). Botón *Usar como base* copia a capa 2. |
| **2** | Global de **esta pista**: todos los rayitos que no tengan capa 1 | Autor de la pista | `layout.ron` → `pickup_odds: Option<PowerupOdds>` | Panel de Pickups, tabla editable. Vacío (`None`) = se usa capa 3 (o el host, §7.7.1). |
| **1** (más fuerte) | **Un** rayito concreto | Autor de la pista | `PickupSpawn.odds: Option<PowerupOdds>` | Seleccionar el rayito → checkbox *Tabla propia* → sliders. `None` = cae a capa 2 o 3. |

El **host** (sala multiplayer o la sesión singleplayer) no es una cuarta capa que pise al autor. O bien la pista es “vanilla” y el host puede sustituir la capa 3, o bien la pista trajo capa 1 o 2 y el host **no puede tocar nada**. Detalle en §7.7.1.

Resolución en runtime (`crates/core/src/powerups/odds.rs`), al momento del contacto:

```
locked = track_locks_host_odds(layout)
       = layout.pickup_odds.is_some()
         || pickups.iter().any(|p| p.odds.is_some())

odds = spawn.odds                         // capa 1
    ?? layout.pickup_odds                 // capa 2
    ?? (if locked { None } else { room.pickup_odds })  // host, solo si la pista no lockea
    ?? DefaultPowerupOdds                 // capa 3
kind = weighted_sample(odds.weights)
```

Reglas:

- Se usan **pesos relativos** (`w: u32`), no porcentajes. Al sortear se normalizan (`p_i = w_i / sum(w)`).
- `w = 0` → ese poder no sale en esa capa.
- Un poder ausente de una tabla custom vale **0** (reemplazo completo, no merge). La UI de capas 2 y 1 **siempre muestra todos** los `PowerupKind` y arranca copiando la capa inferior.
- `validate.rs`: todos los `w >= 0` y `sum(w) > 0`.
- Capa 2 vacía + capa 1 vacía en todos los spawns = pista vanilla → el host puede editar.
- El sorteo lo hace el **server** (sim autoritativa). El cliente solo muestra el resultado. Si un host manda `SetPickupOdds` sobre una pista locked, el server **rechaza** el mensaje (no se confía en la UI).

UI del **editor de pista** (`client/src/editor/odds.rs`, modo Pickups) — no cambia: capas 3 / 2 / 1 para el autor.

#### 7.7.1 Host / singleplayer: panel de sala y bloqueo por pista

Misma pantalla en **lobby multiplayer** y en **singleplayer**. Solo el host la edita; los invitados la ven de solo lectura o ni la abren.

- Código: `client/src/ui/room/pickup_odds.rs`.
- Estado de sala (dura toda la sesión): `GameplayRules.pickup_odds: Option<PowerupOdds>` en `server/src/game/room.rs`. `None` = capa 3 (defaults). `Some` = lo que el host aplicó.
- Red: `LobbyEvent::SetPickupOdds { odds: Option<PowerupOdds> }` (solo host). `None` = Reset to Default.

**Valor guardado ≠ valor mostrado.** Cambiar de pista **nunca** escribe ni borra `room.pickup_odds`. El lock solo decide si los sliders se mueven y qué tabla se dibuja.

**Pista vanilla** (`track_locks_host_odds == false`):

1. Sliders **ON**. Muestran `room.pickup_odds` si es `Some`; si no, capa 3.
2. Botón **Reset to Default** (*Restablecer defaults*): habilitado si `pickup_odds.is_some()`. Pone `None` y los sliders vuelven a capa 3. No toca el layout de la pista.
3. Sorteo: `room.pickup_odds ?? defaults`.

**Pista bloqueada** (capa 2 y/o algún rayito con capa 1):

1. Banner: *«No se puede modificar las probabilidades en esta pista»*.
2. Sliders **inmóviles**. Muestran la tabla **de la pista**, no la de sala.
3. **Reset to Default** también **OFF** (no se muta la sala desde una pista locked).
4. `room.pickup_odds` **no se toca**. El sorteo de esta carrera usa el layout.

**Volver a vanilla:** sliders ON con **el mismo `Some(host)` de antes del lock**, no los defaults. Solo Reset escribe `None`.

```
sala: pickup_odds = None
host pista A (libre)      → aplica sus odds → pickup_odds = Some(host)
host pista B (bloqueada)  → sliders OFF; se sortea B; Some(host) SIGUE en sala
host pista C (libre)      → sliders ON otra vez con Some(host)  // no se pierde
host Reset to Default     → pickup_odds = None
```

Si resetea en C, pasa por B y vuelve a C: ve defaults, porque el reset **sí** guardó `None`. El lock de B no es un reset.

Pistas legacy: locked solo si el `.fob`/metadata mapea tablas custom. El resto es vanilla. El server rechaza `SetPickupOdds` (incluido reset) si la pista actual está locked.

#### 7.7.2 Slot de poder (uno, hasta que se usa)

- **Un** slot por auto (`PowerupSlot: Empty | Occupied { kind, charges, extra }`).
- El poder **no tiene timer de inventario**: dura hasta que el jugador (o el bot) lo **usa**. Excepción: **Bomba** (papa caliente), que cuenta 10 s en el slot.
- Recoger: solo si `Empty`. Si `Occupied`, el rayito **no se recolecta**; el auto lo atraviesa (sensor apagado para ese auto). El rayito sigue ahí para quien tenga slot vacío.
- Bots recogen y usan con la misma máquina de estados (`revvy-bots` + `PickupSystem`).
- Árbitro: host si `Client`, server si `Server`.

Lista de `PowerupKind`, comportamiento y VFX: §7.11.

---

### 7.8 Campos de fuerza

Equivalente a `.fld` de Re-Volt. Se **colocan en el editor** como OBB (mismo gizmo que TrackZones), no se modelan en el `.glb`.

`ForceField.kind`:

- `GravityScale(f32)` — 1.0 = normal, `0.35` = luna, `0.0` = caída libre, negativo = invertido.
- `Wind(Vec3)` — fuerza continua en world-space (túnel, ventilador).
- `ConstantForce(Vec3)` — empujón arbitrario (rampa invisible, conveyor).

Runtime: `physics/force_field.rs` cada tick consulta overlap auto↔volumen y suma la aceleración al `vehicle_controller`, sobre la gravedad base del motor (§1.9). El visual del campo en carrera es opcional (partículas); en el editor siempre se ve el OBB + flecha del vector.

Legacy: `fld.rs` rellena `force_fields`. Pistas nuevas no escriben `.fld`.

### 7.9 Efectos de tipo de piso

Hay dos piezas, las dos se tocan en el editor. El **tipo** de un triángulo de colisión sigue viniendo del material de `Collision` en el `.glb` (`Ice`, `Road`, …). Lo que el editor define es **qué hace** ese tipo, y opcionalmente **parches** que lo fuerzan.

**A. Tabla de efectos (pista entera)** — modo **Surfaces**, panel egui, sin gizmo.

- Defaults: `config/rules/surfaces.default.ron` (hielo resbala, dirt corta grip, etc.; números TBD). Las pistas de Re-Volt traducen sus 27 materiales a este mismo modelo (§1.9). `SurfaceType` los cubre y el perfil físico de cada uno ya da el agarre de `COL_MaterialInfo` (§1.9), así que los defaults de esta tabla tienen que ser neutros (1.0) para que una pista legacy se sienta como en el juego.
- Override de pista: `layout.ron` → `surface_effects: Option<Map<SurfaceType, SurfaceEffect>>`. Solo se listan los tipos que el autor cambió; el resto cae al default.
- `SurfaceEffect`: `friction`, `lateral_grip`, `rolling_resist`, `speed_factor` (extensible: sfx, partículas).
- `vehicle_controller` lee el efecto del `SurfaceType` bajo las ruedas **después** de resolver el tipo (punto B).

**B. Volúmenes de superficie** — gizmos OBB en el mismo modo.

- `surface_volumes: [SurfaceVolume { obb, surface }]`.
- En el punto de contacto: si el contacto está dentro de un volumen, ese `surface` **pisa** el material del mesh. Así se marca un charco de hielo sin volver a Blender.
- Prioridad del **tipo** en un contacto: `SurfaceVolume` que contiene el punto → material de `Collision` → `Road`.
- Varios volúmenes solapados: gana el de id más alto (el último colocado), visible en el panel.

El editor pinta el piso según tipo (tint wireframe: cian = Ice, marrón = Dirt, …) para verificar mesh + volúmenes juntos.

No se pintan texturas de hielo desde el editor: si el visual tiene que *verse* helado, eso es el `.glb`. El volumen solo cambia la física.

Si lo que se quiere es “los autos agarran peor en hielo” (sobre todo **un** auto), eso es `param_mods` con `surface: Some(Ice)` (§7.10), no esta tabla. Las dos se **componen**: `grip_efectivo = car.grip × mod_de_auto_en_hielo × surface_effects[Ice].lateral_grip`. El editor avisa si hay un `ParamMod` All+Ice+Grip **y** un override de `Ice.lateral_grip` a la vez (redundante, fácil de doble-nerfear).

### 7.10 Mods de parámetros de auto (por pista)

Una pista puede cambiar cómo se conducen los autos **en esa pista**, sin tocar `cars/<id>/car.toml`. Se edita en el modo **CarParams** (panel egui, sin gizmos 3D). Se guarda en `layout.ron` → `param_mods: [ParamMod]`.

La idea de autoría es exactamente esta:

| Intención | No se escribe así en disco (ver sintaxis abajo) |
| --- | --- |
| Todos −0.5 de engine (puntos / unidades del stat) | target All, op Add, Engine, −0.5 |
| Todos −5 % de engine | target All, op Percent, Engine, −5 |
| Todos el engine **fijado** a 50 | target All, op Set, Engine, 50 |
| Solo `calcure` −0.5 de engine | target Car("calcure"), op Add, Engine, −0.5 |
| Todos −5 % de grip **en hielo** | target All, op Percent, Grip, surface Ice, −5 |

`calcure` es el `car_id` (nombre de carpeta en `cars/`), igual que en Re-Volt.

#### Por qué no la sintaxis `.global (` / `.calcure (` / `-0.5%`)

Se entiende, y el editor puede *mostrar* algo parecido. En disco **no** se usa ese texto:

1. El resto del layout ya es **RON**. Un mini-lenguaje aparte es un segundo parser, con errores peores (`-0.5%` vs `−5%`).
2. `-0.5` (puntos) y `-0.5%` (porcentaje) se leen como si `0.5` significara dos cosas. “5 % menos” es **−5** con op `Percent`, no `−0.5%`.
3. `set` como keyword al lado de `.global` no encaja con serde; `op: Set` es un enum explícito.
4. El artista no edita el RON: edita filas en el panel. El archivo es detalle de implementación.

#### Modelo

```ron
ParamMod(
    target: All,              // o Car("calcure")
    op: Add,                  // Add | Percent | Set
    stat: Engine,             // enum cerrado: Engine, Grip, Mass, Steer, … (lista TBD, = campos de car.toml)
    surface: None,            // None = siempre. Some(Ice) = solo con las ruedas en ese SurfaceType
    value: -0.5,
)
```

Operaciones sobre el valor actual `x` del stat:

- `Add`: `x + value` (unidades del stat, las mismas que en `car.toml`).
- `Percent`: `x * (1 + value/100)` → `value: -5.0` es “5 % menos”.
- `Set`: `x = value` (absoluto; ignora el valor previo de esa pasada).

#### Orden de aplicación

Al iniciar la carrera, por cada auto:

1. Cargar `CarDef` base (`car.toml` o `.inf` legado).
2. Aplicar, **en el orden de la lista** (arriba → abajo en el editor):
   - primero las filas `target: All`;
   - después las filas `target: Car(este_id)`.
3. Dentro de cada grupo, el orden de la lista es la verdad. Un `Set` posterior pisa un `Add` anterior. Un `Add` posterior a un `Set` suma sobre el valor ya fijado.
4. Clamp por stat (p.ej. `Engine >= 0`). `validate.rs` rechaza stats desconocidos y `Percent` con `value <= -100` (multiplicar por 0 o negativo).

Los mods con `surface: Some(S)` **no** se hornean en el `CarDef` de spawn. Quedan en el componente `SurfaceStatMods` y `vehicle_controller` los aplica solo mientras el contacto resuelto (§7.9 B) sea `S`.

Un `Car("no_existe")` si ese auto no corre es un no-op. El editor lista los `car_id` de `cars/` local; se puede tipear un id a mano (pack que el autor no tiene).

#### Editor (`client/src/editor/param_mods.rs`)

Tabla de filas: Target (All | combo de autos) · Stat · Op (Δ / % / =) · Valor · Superficie (cualquiera | Ice | Dirt | …).

- Preview: elegir un auto de `cars/` y ver stats **antes / después** (incluye All + ese Car). Si hay mods con superficie, una segunda columna “en hielo”.
- Arrastrar filas para cambiar el orden (el orden *es* la semántica).
- No hay gizmos: no es espacial.

Runtime: `crates/core/src/vehicle/param_mod.rs`, **árbitro** (server o host según §4.8). El cliente usa el mismo `CarDef` efectivo.

### 7.11 Lista de poderes

Enum en `crates/core/src/powerups/kind.rs`. Pesos default TBD en `powerups.default.ron`. Uso = tecla/botón de poder. Vectores de empuje: constantes en `config/rules/powerups.default.ron` (magnitud TBD).

| Kind (código) | UI | Cómo se usa | Efecto |
| --- | --- | --- | --- |
| `WaterBalloon` | Bombucha | Hasta **3** disparos (charges). | Proyectil; al pegar a un auto aplica vector de empuje. Al **pared** explota (se consume ese disparo). |
| `HomingRocket3` | Cohete ×3 | Hasta **3** disparos. | Como bombucha, pero **busca** al rival más relevante (línea de visión / más cercano adelante) y lo sigue. |
| `HomingRocket1` | Cohete ×1 | **1** disparo. | Igual que el ×3. |
| `OilSlick` | Aceite | 1 uso: se **derrama** en el piso. | Volumen/decal en el suelo. Otro auto que lo pise **pierde el control** (grip ~0) mientras solapa. El autor no resbala en su propio aceite un breve grace (TBD, ~0.5 s). |
| `Electric` | Electricidad | 1 uso: pulso en radio R. | Autos en el radio (no el usuario) quedan **electrificados**: no pueden acelerar **4 s**. |
| `Shockwave` | Shockwave | 1 disparo. | Como bombucha, **hitbox más grande**, el empuje es **siempre hacia arriba**. Al pared **rebota** (no explota). |
| `Battery` | Batería | 1 uso desde el slot. | +aceleración y +velocidad tope **10 s**. |
| `FakeBolt` | Rayito | 1 uso: deja un rayo **falso** en el mapa (mismo mesh que el pickup). | Si **otro** auto lo toca: VFX de explosión + vector hacia **arriba**. No otorga poder. El que lo tiró no lo dispara. |
| `HotPotato` | Bomba | Al recoger entra al slot y **arranca 10 s**. No se “dispara”: se **pasa**. | Papa caliente. Al llegar a 0: explosión (VFX + empuje) sobre quien la tiene. Para pasarla hay que **tocar** a otro auto; el receptor la hereda con el tiempo **que quedaba**. Quien la acaba de pasar tiene **3 s de invulnerabilidad** (no se la pueden devolver). |
| `Star` | Estrella | 1 uso. | Aplica el efecto **Electric** a **todos** los demás (sin radio). |
| `HeavyBall` | Bola gigante | 1 uso: se **suelta** con la velocidad del auto (no queda quieta). | Cuerpo Rapier dinámico (como la bola cromada de Re-Volt), mucha masa. Estorba. Colisiona con paredes; si la velocidad es alta, **rebota**. |

Invulnerabilidad de la bomba no bloquea otros poderes ni el daño de aceite, solo el **pase** de HotPotato.

### 7.12 Respawn (estilo Re-Volt)

Tres disparadores automáticos + uno manual. Destino: último **PosNode** válido asociado a la TrackZone más reciente (auto derecho, pequeño i-frames).

| Disparador | Qué es | Dónde se edita |
| --- | --- | --- |
| **Kill volume** | OBB: si el auto **entra**, respawn. Pozos, lava, vacío. | Editor modo `KillVolumes` → `kill_volumes` |
| **Fuera de TrackZones** | Las TrackZones **son** el volumen jugable (cajas a lo largo de la pista). Si el auto queda **fuera de todas** más de `off_track_secs` (default ~1.5 s), respawn. | Las mismas cajas de §7.3 paso 5 |
| **Fuera del mundo** | Sale del AABB de la pista (`Collision` + margen). Respawn inmediato. | Automático, sin gizmo |
| **Manual** | Keybind / botón HUD. El jugador respawnea cuando quiera (atado, trabado, etc.). | Input map |

Las kill volumes son el caso “tocar el bloque malo”. Las TrackZones son el caso “bloques gigantes = zona jugable, si te salís volvés”. Los dos coexisten.

Legacy: triggers de reposition de Re-Volt → `kill_volumes` si el tipo es kill; si no, solo TrackZones + world AABB.

---

## 8. Pipeline artista → `levels/` → runtime

```
  Blender ──┐
            ├─ export GLB (nodos Visual + Collision) ──► levels/<id>/visual.glb
  Blockbench┘                                            levels/<id>/track.toml   (a mano o plantilla)

                 tools/track-editor
                    gizmos + ghost + rayitos/odds + campos + superficies + car params + kill volumes
                         │
                         ▼
                 levels/<id>/layout.ron

                         │
                         ▼
              tools/map-packager
              (valida layout + 100MB,
               hornea .gltf suelto → .glb si hace falta)
                         │
                         ▼
              paquete .zst  →  maps_storage / catálogo Axum
                         │
          ┌──────────────┴──────────────┐
          ▼                             ▼
   cliente wgpu                    server rapier
   (Visual)                        (Collision + layout)
```

Copia manual: si el artista deja `levels/<id>/` completa (`track.toml` + `visual.glb` + `layout.ron`) en el disco del jugador, `assets_pipeline/` la marca `Ready` por hash igual que una track Re-Volt copiada a mano. El `.blend` / `.bbmodel` **no** viaja.

---

## 9. Consideraciones adicionales (no pediste esto explícitamente, pero conviene resolverlo pronto)

- **Auth**: algo liviano tipo JWT (`jsonwebtoken` crate) o session tokens en Redis; no hace falta un sistema complejo para arrancar.
- **Anti-cheat**: en `sim_authority: Server`, validar inputs y rangos. En `Client` (amigos) se asume confianza; no es modo ranked.
- **Escalabilidad de salas**: si más adelante corrés múltiples instancias del servidor de juego, Redis pub/sub sirve para coordinar matchmaking entre instancias.
- **Empaquetado mobile**: `cargo-mobile2` para no reescribir nada de lógica; `platform/` es el overlay táctil. Las pistas del server viven en RAM (§4.5), no en el almacenamiento de la app.
