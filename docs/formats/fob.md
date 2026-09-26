# `.fob`

Objetos del nivel. Un `i32` con la cantidad y registros `FILE_OBJECT` de 56 bytes: tipo (`OBJECT_TYPE`), cuatro flags enteros, posición y los ejes `Up` y `Look` (la matriz es `[Up × Look, Up, Look]`). Lo que significa cada flag depende del tipo.

- **Rayitos** (tipo 30): en `nhood1` son el grupo que cae más cerca de los POS nodes (27 instancias) → `pickups`. No hay tabla de odds en el binario; `PickupSpawn.odds` queda en `None`.
- **Sonidos** (tipo 49, `3DSOUND`, y 40, regadores) → emisores de `TrackSounds`.
- **Objetos físicos** → `TrackObjects` (`revolt_objects.rs`), con la física de su `Init*` y el modelo de `models/`:

  | Tipo | Objeto | Modelo | Arranca dormido |
  | --- | --- | --- | --- |
  | 1 | pelota de playa | `beachball` | no |
  | 15 | pelota de fútbol | `football` | no |
  | 43 | pelota de básquet | `basketball` | no |
  | 57 | botella | `bottle` | si `flags[0]` ≠ 0 |
  | 58 | balde | `bucket` | sí |
  | 59 | cono | `trafficcone` | no |
  | 66 | caja | `packet` | sí |
  | 67 | cubo ABC | `abcblock` | sí |

- **Lanzador** (tipo 42, `OBJECT_THROWER`): `flags` = id, tipo de objeto, velocidad y reuso. Cuando un auto entra en el trigger `.tri` de tipo 6 con ese id, aparece el objeto en el lanzador a `Speed × 50` unidades/s por su eje `Look`, una vez por carrera.
- **Puerta corrediza** (tipo 56, `OBJECT_TYPE_SLIDER`): `models/slider.m`, que choca con `slider.ncp`. `flags[0]` es el id de la hoja. Cada hoja va y viene 400 unidades por su eje `R` (`Up × Look`) en 3 s; la de id 0, para el otro lado → objeto con camino (`ObjectMotion::Slide`). En market2 son dos.
- **Chango** (tipo 7, `OBJECT_TYPE_TROLLEY`): el auto `cars/trolley`, sin conductor → `CarSpawn`, con el yaw de su `Look`. Hay dos en market1 y dos en market2.
- **Cielo** (tipo 55, `SKYBOX`): prende el cielo de la carpeta del nivel, que ya carga `load_sky`.
- **Objeto animado** (tipo 76, RVGL): `flags[0]` es el `Slot` de su animación en `custom_animations.txt`, `flags[1]` el id de su trigger, `flags[2]` la espera antes de arrancar en décimas de segundo y `flags[3]` si se ve en contrarreloj → `TrackAnimations` (`custom_animations.md`). En wildland son 19: 17 mástiles con bandera y 2 sogas con banderines.

Los demás tipos (la lata, la colchoneta, las estrellas, las luces…) se leen y no se traducen, porque no están sus modelos; el log los lista.
