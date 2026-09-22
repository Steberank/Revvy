# GameplayRules

Los presets viven en `config/rules/`. El recurso ECS que los carga llega en la fase 4.

Campos de sala:

- `laps`
- `allow_jump` (default `false`)
- `sim_authority`: `Client` o `Server`
- `late_join_mode`: `Spectator` o `Racer`
- `disconnect_bot_replace`
- `reconnect_secs` (default 60)
- `turbo`: vector
- `pickups_enabled`
- `pickup_odds`: `Option` (capa del host; `None` usa los defaults)

`config/rules/antigrav_turbo.ron` es el mismo set con otro vector de turbo.
`powerups.default.ron` es la capa 3 de pesos. `surfaces.default.ron` es la tabla de `SurfaceEffect`.
