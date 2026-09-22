# Protocolo Revvy

Mensajes de juego sobre Quinn. La spec se cierra en la fase 7.

Previstos: `Input`, `Snapshot` / `VehicleState`, `MapSync` (`MapRequired`, `AssetReady`), `LobbyEvent` (`SetTrack`, `SetReady`, `SetRules`, `SetPickupOdds`, `StartRace`, `ForceStart`, `PlayerJoined`, `PlayerLeft`).

Codec: postcard, con zstd opcional en payloads grandes. ALPN del canal de juego: `revvy`.
