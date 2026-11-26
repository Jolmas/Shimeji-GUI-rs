# Packaging

## Instalación
- Binario: `shimeji-gui-rs`
- Desktop: `packaging/shimeji-gui-rs.desktop` -> `~/.local/share/applications/shimeji-gui-rs.desktop`
- Icono: `packaging/shimeji-gui-rs.svg` -> `~/.local/share/icons/hicolor/scalable/apps/shimeji-gui-rs.svg`
- Autostart (opcional, desde Settings): `~/.config/autostart/shimeji-gui-rs.desktop` con `Exec=shimeji-gui-rs`

El icono es un hongo sonriente (sombrero rojo con puntos, ojitos y boca). Si tu dock o panel no
muestra SVG, instala también una versión PNG:

```bash
inkscape packaging/shimeji-gui-rs.svg \
  --export-type=png --export-filename=shimeji-gui-rs.png -w 256 -h 256
install -Dm644 shimeji-gui-rs.png \
  ~/.local/share/icons/hicolor/256x256/apps/shimeji-gui-rs.png
gtk-update-icon-cache -f ~/.local/share/icons/hicolor 2>/dev/null || true
```

## Build
```bash
cargo build --release
install -Dm755 target/release/shimeji-gui-rs ~/.local/bin/shimeji-gui-rs
install -Dm644 packaging/shimeji-gui-rs.desktop ~/.local/share/applications/shimeji-gui-rs.desktop
install -Dm644 packaging/shimeji-gui-rs.svg ~/.local/share/icons/hicolor/scalable/apps/shimeji-gui-rs.svg
```
