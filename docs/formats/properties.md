# `properties.txt` (RVGL)

Lo que una pista de RVGL redefine del juego. Es texto: comentarios con `;` y secciones `NOMBRE { … }` con una clave por línea. `ID` dice qué se reemplaza, y una clave que falta deja el valor de Re-Volt (`COL_MaterialInfo`, `COL_CorrugationInfo`). Lo lee `rvgl_properties.rs`.

- `MATERIAL` (ID 0 a 26, los materiales de Re-Volt): de acá se usan `Roughness`, `Grip`, `Hardness`, `Corrugated`, `CorrugationType`, `Moves` y `Velocity` → `SurfaceTuning` en `Collision.surfaces`. El motor usa esos números en esa pista (`surfaces::tuned`). `Name`, el derrape, las chispas y el polvo se leen y no se usan.
- `CORRUGATION` (ID 0 a 7): `Amplitude` y `Wavelength` (x, z) en unidades. Un tipo redefinido llega a todos los materiales que lo usan, también a los que no redefine la pista.
- `DUST`, `SPARK`, `TRAIL`, `WIND`, `GRAVITY` y `PICKUPS` se leen y todavía no se usan.

Wildland redefine la tierra (ID 18: Roughness 0.73, Grip 0.3425, Hardness 0.2 y los baches de la grava) y la arena (ID 4), además de polvo, chispas y viento.
