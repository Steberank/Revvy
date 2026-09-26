# `custom_animations.txt` (RVGL)

Objetos animados de las pistas de RVGL (desde la versión 19.1230a). Es texto: comentarios con `;`, bloques `NOMBRE { … }` (la llave puede ir en la línea de abajo) y claves con sus valores en la misma línea. Lo lee `animations.rs`, y los objetos que lo usan son los de tipo 76 del `.fob`.

- `MODEL id "nombre"` (0 a 63): un `.m` con su `.ncp` opcional, en `custom/` o en `models/`. `MODELRGBPER` del `.inf` escala su color, como el de los otros modelos del nivel.
- `SFX id "nombre"`: los sonidos de los keyframes. Se leen y todavía no suenan.
- `ANIMATION { … }`: `Slot`, `Name`, `Mode` (0 loop, 1 una vez, 2 ida y vuelta), `NeedsTrigger`, `TriggerOnce`, `PreCountdown`, sus `BONE` y sus `KEYFRAME`.
- `BONE { … }` de la animación (hasta 16): `BoneID`, `ModelID` (−1 es sin modelo), `Parent` (un id menor; el 0 no tiene) y la pose de reposo respecto del padre: `OffsetTranslation`, `OffsetRotationAxis` y `OffsetRotationAmount` (grados).
- `KEYFRAME { … }` (hasta 256): `FrameNr`, `Time` (segundos desde el keyframe anterior), `Type` (0 lineal, 1 arranca suave, 2 termina suave, 3 las dos, 4 se pasa y vuelve) y un `BONE` por hueso que mueve, con `BoneID`, `Visible`, `Translation`, `RotationAxis`, `RotationAmount` (grados) y los bloques `SFX`, `SPARK` y `LIGHT`.

Lo que se confirmó con pistas de RVGL:

- **Los keyframes suman.** El ascensor de fair2 tiene +4600, 0, −4600 y 0 en Y: sube, espera, baja y espera. Con valores absolutos bajaría por debajo del piso.
- **Los giros van al revés que la regla de la mano derecha**, como las matrices de Re-Volt, que multiplican vectores fila. La soga de wildland baja a medida que avanza en Z y sus banderines están girados +8.6° sobre X: con la regla de la mano derecha subirían contra la soga. En Revvy el ángulo va negado.
- **Ida y vuelta deshace lo hecho.** Las banderas de wildland giran 20°, 10°, 12° y 8° por vuelta y vuelven al reposo en la vuelta de regreso.

Lo que se eligió sin poder confirmarlo:

- Un loop sigue desde donde terminó la vuelta anterior, como si cada keyframe sumara a la pose de ese momento. Los loops de wildland y fair2 suman 0 por vuelta, así que ahí no hay diferencia.
- `Visible` cambia cuando empieza el keyframe.
- El reloj es el de la carrera, y `PreCountdown` no cambia nada, porque Revvy no tiene cuenta regresiva.

Según la documentación de RVGL, `Visible` también saca la colisión del hueso. En Revvy chocan los huesos con `.ncp` que nunca se mueven ni se esconden, como los mástiles de wildland, y van con la colisión de la pista. Los huesos se dibujan como objetos y ocupan lugares de `MAX_OBJECTS`.

Todavía no: los triggers (con `NeedsTrigger`, la animación queda quieta), los sonidos, chispas y luces de los keyframes, y los huesos que se mueven y chocan.
