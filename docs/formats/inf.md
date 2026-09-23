# `.inf` y `parameters.txt`

El `.inf` de pista es texto. `NAME`, `STARTPOS` y `STARTROT` (vueltas de 0 a 1) arman el slot 0 de la grilla. El resto de claves se conserva.

El auto de stock usa `parameters.txt` con la misma sintaxis; un `.inf` en la carpeta del auto también sirve. `EngineRate`, `Mass`, `Grip` y `SteerRate` mapean al enum `CarStat`. Cualquier otra clave se guarda y se avisa; no se descarta.
