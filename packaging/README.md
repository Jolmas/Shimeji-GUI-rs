# Packaging

## Instalación
- Binario: `shimeji-gui-rs`
- Desktop: `packaging/shimeji-gui-rs.desktop` -> `~/.local/share/applications/shimeji-gui-rs.desktop`
- Autostart (opcional, desde Settings): `~/.config/autostart/shimeji-gui-rs.desktop` con `Exec=shimeji-gui-rs`

## Build
```bash
cargo build --release
install -Dm755 target/release/shimeji-gui-rs ~/.local/bin/shimeji-gui-rs
install -Dm644 packaging/shimeji-gui-rs.desktop ~/.local/share/applications/shimeji-gui-rs.desktop
```
