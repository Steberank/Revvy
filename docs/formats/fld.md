# `.fld`

Campos de fuerza. Count y registros de 100 bytes (a veces 104). Tipo 0: dirección en Y de Re-Volt, guardada como `GravityScale` (Y positiva del archivo es hacia abajo). Otro tipo: `ConstantForce` con esa dirección. `nhood1.fld` está vacío. El tamaño que no cierra se ignora con un warning, sin panic.
