# `.ncp`

Colisión. `u16` de poliedros y, en cada uno, tipo, material, 5 planos y un AABB. La grilla de lookup que sigue no se guarda: es una aceleración del juego viejo.

El bit 0 del tipo es quad. Los vértices salen de la intersección de planos, igual que el addon (distancia negada). El material de Re-Volt se mapea a `SurfaceType`; un id desconocido queda en `Road`. `nhood1.ncp` tiene 1383 poliedros.
