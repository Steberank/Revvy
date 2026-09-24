//! Constantes de `TypeDefs.h`, `units.h`, `newcoll.h`, `body.h`, `car.h` y `wheel.h`
//! (versión PC). La unidad de largo es la de Re-Volt (5 mm) y el tiempo en segundos.

pub const SMALL_REAL: f32 = 0.00001;
pub const SIMILAR_REAL: f32 = 0.0001;
pub const LARGEDIST: f32 = 1_000_000.0;

/// `OGU2MPH_SPEED`: velocidad de Re-Volt a millas por hora.
pub const OGU2MPH_SPEED: f32 = 0.01118;
/// `MPH2OGU_SPEED`: `ReadInit` multiplica `TopSpeed` por esto al leer el auto.
pub const MPH2OGU_SPEED: f32 = 1.0 / OGU2MPH_SPEED;

/// `FRICTION_TIME_SCALE` en PC.
pub const FRICTION_TIME_SCALE: f32 = 120.0;

/// Espesor de la piel de colisión (`COLL_EPSILON`).
pub const COLL_EPSILON: f32 = 2.0;
/// `SMALL_IMPULSE_COMPONENT`: componentes de impulso de rueda por debajo se descartan.
pub const SMALL_IMPULSE_COMPONENT: f32 = 0.005;

/// `MAX_COLLS_BODY`, `MAX_COLLS_PER_BODY` y `MAX_COLLS_WHEEL` de PC.
pub const MAX_COLLS_BODY: usize = 700;
pub const MAX_COLLS_PER_BODY: usize = 32;
pub const MAX_COLLS_WHEEL: usize = 500;

/// `MAX_SCRAPE_TIME`: el material de roce se olvida después de esto.
pub const MAX_SCRAPE_TIME: f32 = 0.05;
/// `MIN_SPARK_VEL`: por debajo no hay chispas ni roce.
pub const MIN_SPARK_VEL: f32 = 100.0;
/// `OILY_WHEEL_TIME`.
pub const OILY_WHEEL_TIME: f32 = 2.5;
/// `SKID_RAISE`: el punto de contacto se levanta esto sobre el plano.
pub const SKID_RAISE: f32 = 2.0;

/// `FLD_Gravity`: aceleración del campo de gravedad global, hacia +Y (abajo).
pub const FLD_GRAVITY: f32 = 2200.0;

/// `MAX_TIMESTEP` de `gameloop.h`: 1 / paso máximo de física.
pub const MAX_TIMESTEP: f32 = 150.0;

/// `CTRL_RANGE_MAX` de `control.h`.
pub const CTRL_RANGE_MAX: f32 = 127.0;

/// `COLLGRID_EXPAND`: margen al asignar polígonos de instancia a una celda.
pub const COLLGRID_EXPAND: f32 = 70.0;
