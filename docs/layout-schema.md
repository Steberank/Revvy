# TrackLayout / `layout.ron`

El ejemplo canónico está en `crates/formats/schema/layout.ron`.

El tipo Rust (`crates/formats/src/layout.rs`) y la validación del editor llegan en fases posteriores. Hasta entonces este archivo describe la forma: grilla, zonas, POS, AI, pickups, campos de fuerza, superficies, `param_mods`, kill volumes y objetos.

Hoy una pista `.glb` lee de su `layout.ron` `start_grid`, lo que usa la carrera (`zones`, `pos_nodes`, `start_node`, `total_distance` y `kill_volumes`, con la misma semántica que `.taz`, `.pan` y los triggers de Re-Volt) y `objects`. Las zonas llevan los ids en el orden de la vuelta, desde la meta; cada POS node dice cuánto falta hasta la meta (0 en `start_node`); la grilla va en la última zona, detrás de la línea. Cada objeto nombra una carpeta de `objects/` de la pista; `spawn` es `Start` (por defecto) o un `Trigger` con su caja y un `rearm` opcional en segundos, y `motion` le da un camino (`Slide` con `offset` y `period`). Si la carpeta tiene `car.toml`, es un auto sin conductor y solo usa `pos` y `yaw` (ver §6.4 de la arquitectura).
