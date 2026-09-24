# `.w`

Mundo visible. Confirmado contra el addon de RVGL (`rvstruct.World`) y `content/levels/nhood1/nhood1.w` (77 meshes, el archivo se consume entero).

Cabecera de meshes, luego bigcubes, animaciones de textura y una lista de colores de entorno (un color por polígono con el bit `0x800`). Algunos niveles terminan justo después de los bigcubes; si no hay más bytes, las animaciones se toman como vacías.

Cada mesh trae esfera, AABB, polígonos y vértices. El vértice es posición + normal, sin UV: el UV vive en el polígono.
