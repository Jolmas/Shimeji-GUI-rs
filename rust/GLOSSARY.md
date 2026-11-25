# GLOSSARY.md

| Original term | Meaning | Rust name (planned) |
|---|---|---|
| `shimejictl` | CLI externo que controla el daemon/overlay wl_shimeji | `shimejictl` / `CtlCommand` runner |
| prototype / prototipo | Mascota disponible/instalada | `Prototype` |
| environment / entorno | Entorno/overlay gestionado por shimejictl | `Environment` |
| summon / dismiss | Invocar / despedir mascota | `summon`, `dismiss` commands |
| `ie_mode` | Modo de interacción con ventanas del plugin | `IeMode` enum (`active/focused/largest/none`) |
| `plugins_location` | Ruta de plugins en `shimeji-overlayd.conf` | `plugins_location` config key |
| `.wlshm` | Paquete WLPK (header 512B + tar) con manifest/assets | `WlshmPackage` |
| `DAEMON_CONF` | `~/.local/share/wl_shimeji/shimeji-overlayd.conf` | `daemon_conf_path()` |
