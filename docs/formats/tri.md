# `.tri`

Triggers: cajas orientadas que hacen algo cuando entra un auto. Un `i32` con la cantidad y registros `FILE_TRIGGER` de 68 bytes: tipo, flag, centro, matriz de tres filas y medio tamaño en cada eje (`tri.rs`). El tipo es el índice en `TriggerInfo` de `trigger.cpp` (el de `rvsource/Xbox/Src`, con la rama `_PC`) y el flag es su parámetro:

| Tipo | En Re-Volt | En Revvy |
| --- | --- | --- |
| 0 | `TriggerPiano` | se lee |
| 1 | `TriggerSplit` (tiempos parciales) | se lee |
| 2 | `TriggerTrackDir` (las flechas de hacia dónde sigue la pista) | se lee |
| 3 | `TriggerCamera` | se lee |
| 4 | `CAI_TriggerAiHome` | se lee |
| 5 | `TriggerCameraShorten` | se lee |
| 6 | `TriggerObjectThrower`: el lanzador del `.fob` con `ID == flag` tira su objeto | aparición con trigger (`ObjectSpawn`) |
| 7 | `TriggerGapCamera` | se lee |
| 8 | `TriggerRepositionCar`: el auto quedó afuera y vuelve a la pista | `kill_volumes` |

Hasta la fase 2.8 los kill volumes salían del tipo 2, que son las flechas: nhood1 no tiene triggers de tipo 8.
