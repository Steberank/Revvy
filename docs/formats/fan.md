# `.fan`

AI nodes. Lo lee `LoadAiNodes` (`rvsource/Xbox/Src/ainode.cpp`); el struct es `FILE_AINODE` de `editai.h`. Little-endian en PC.

- Cabecera de 4 bytes: `i16` nodos del archivo (`AiSingleNodeNum`) e `i16` nodos link (`AiLinkNodeNum`). Los link los arma el juego en memoria y no tienen datos en el archivo. El editor viejo guardaba un `i32` con la cantidad: se lee igual.
- Nodos de 76 bytes.
- Cola: `i32` nodo de inicio de la IA y `f32` distancia total. RVGL agrega un `i32` más, el nodo final de las pistas sprint (`ENDPOS`). Las arenas de batalla traen solo la cabecera en cero.

Nodo, en bytes:

| offset | campo |
| --- | --- |
| 0 | `Priority` (`AIN_TYPE`), `StartNode` |
| 2 | `flags[2]`, iguales: `0x01` pared izquierda, `0x02` pared derecha |
| 4 | `f32` racing line, distancia a la meta, overtaking line, relleno |
| 20 | `i32` velocidades de racing line y del centro |
| 28 | `i32` `Prev[2]`, después `Next[2]` (`-1` no conecta) |
| 44 | dos extremos, cada uno `i32` velocidad y posición |

El juego invierte los extremos al cargar: el segundo del archivo es el verde, a la izquierda del sentido de carrera, y el primero es el rojo, a la derecha. Racing y overtaking line van de verde (0) a rojo (1). `Next` sigue el sentido de carrera y la distancia a la meta baja.

`nhood1.fan`: 214 nodos y 10 link. Siguiendo `Next[0]` desde el nodo 0, al lado de la largada, se pasa por las zonas 0 a 19 en orden y la vuelta mide 742 m (distancia total del archivo: 740 m).

## Prioridad → `AiFlags`

Según lo que hace la IA en `ai_car.cpp`. El límite va en m/s.

| `AIN_TYPE` | `AiFlags` | `speed_limit` |
| --- | --- | --- |
| `RACINGLINE`, `TURBOLINE`, `SHORTCUT` | racing | — |
| `SLOWDOWN_15`, `_20`, `_25`, `_30` (frena) | racing + slowdown | 15, 20, 25, 30 mph |
| `TITLESCR_SLOWDOWN` (frena por encima del 25 % de la máxima) | racing + slowdown | — |
| `OFFTHROTTLE`, `OFFTHROTTLEPETROL` (suelta el acelerador; `PETROL` solo en autos glow) | racing + slowdown | 20 mph |
| `PICKUP`, `LONGPICKUP` | pickup_route | — |
| `BUMPY`, `SOFTSUSPENSION`, `STAIRS`, `JUMPWALL`, `LONGCUT`, `WILDERNESS`, `BARRELBLOCK` | careful | — |

Los valores desconocidos cuentan como racing line, igual que el `default` de `ai_car.cpp`.

## Archivos raros

En todo `REVOLT/levels` (384 `.fan`):

- Una carpeta `reversed/` normalmente tiene el verde a la izquierda de su propio sentido de carrera, igual que la pista normal: así están 102 de las 108, incluida la de nhood1.
- Las otras seis (`aquavoltredux`, `jailhouse`, `ragarden`, `skires`, `spaceship2020`, `spavolt2`) son los nodos de la pista normal con `Prev` y `Next` intercambiados. Los extremos quedaron en su lugar, así que el verde cae a la derecha del nuevo sentido.
- `ProcessNearWallAvoidance` (`ai_car.cpp`) supone el verde a la izquierda: cerca del borde verde dobla a la derecha. Con el verde a la derecha, la distancia al borde verde da negativa en todo el nodo, y según ese código la IA doblaría a fondo y frenaría. RVGL es de código cerrado y no se puede ver si lo corrige.
- Las arenas `lms_*` traen 2 a 4 nodos sin un sentido claro.
- Dos pistas custom tienen un `Prev` a un índice que no está en el archivo. Ese link se descarta.
