# `.fan`

AI nodes. Cabecera de 12 bytes y nodos de 76. El count es el `u16` bajo (versión chica, p. ej. 10 en `nhood1`) o un `i32` cuando la versión en el segundo entero es 256 (`pici`). Si sobran 4 bytes, es el nodo final de una pista sprint.

En cada nodo, confirmado por las coordenadas de `nhood1`: `racing_t` en el float +4, links next en +20/+24 y prev en +28/+32 (`-1` vacío, dos slots por sentido), esfera izquierda en +40 y derecha en +56, `overtaking_t` en +72. El byte +68 y el byte +70 son propiedades viejas: 0 carrera, 1 ruta de pickup, 2 careful, 5 o propiedad 3 slowdown. `WallLeft` / `WallRight` no están en este archivo.
