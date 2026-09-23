# `.prm` / `.m`

Mesh de instancia o de auto. Mismo polígono que el `.w`, sin esfera ni AABB: `u16` polígonos, `u16` vértices, polígonos, vértices.

Un polígono son 60 bytes: flags, página de textura, 4 índices, 4 colores BGRA (el alfa del archivo se invierte) y 4 UV. El bit 0 marca quad. El cuerpo de `lib16d_maverick` entra por este parser (198 polígonos).
