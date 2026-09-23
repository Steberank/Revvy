# `.pan`

POS nodes. `i32` cantidad, `i32` nodo de largada, `f32` distancia total y nodos de 48 bytes: posición, distancia a la meta, 4 links `prev` y 4 `next`. `-1` no es conexión. El tipo nuevo guarda solo los links válidos y no impone el tope de 4. `nhood1.pan`: 44 nodos, largada en el 41.
