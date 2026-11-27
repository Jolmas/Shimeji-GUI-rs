# shimeji-gui-rs

Desktop GUI written in Rust for managing [wl_shimeji](https://github.com/CluelessCatBurger/wl_shimeji) characters via `shimejictl`.  
Built with `eframe/egui`, targeted at Linux Wayland/KDE Plasma.

![image](image.png)

## Features

### Characters
- Prototype list with search, thumbnail, and preview.
- **Summon**: count, X/Y position, initial behavior, and target environment.
- **Dismiss**: a character, the same type, others, the current screen, or all (with confirmation).
- **Behavior and rescue**: change the behavior of the selected character and restore windows.
- **Management**: add/import `.wlshm` files, export a character to `.zip` (choose the destination folder), delete a character (with confirmation).

### Converter
- Detects the input schema: original Shimeji with **Japanese XML** (`動作リスト`/`行動リスト`), **Shimeji-EE** instance (`conf/` + `img/`) or bundle (`manifest.json` + `animation.json` + `sprites/*.webp`), from ZIP or directory.
- Japanese XML is translated in Rust using the `shimeji-jap-to-wlshm.py` table, then prepared as a single-prototype EE instance for `shimejictl convert`. Use the **Prototype name** field when `img/<name>` must be selected; otherwise it is deduced from the ZIP/directory name.
- Bundles are converted in Rust by reusing a base prototype's logic (`--base`) and resizing its 46 sprites to 128×128 QOI. If the configured base is not installed, the first **compatible** prototype (logic referencing exactly `shime1.qoi`…`shime46.qoi`) is used with a warning; mapped sprites missing from short bundles fall back to the nearest existing sprite.
- Shimeji-EE instances are delegated to `shimejictl convert`.
- Idempotent, selectable fixes (`--fixes`): `refs` (verify sprite references and restore compatible logic), `scale` (configurable target height), `anchor`, `fall`, `embedded` and `cover`; disable all with **No fixes**.
- **Fix existing .wlshm** applies the fixes to an already converted package without converting it again.
- Failed `shimejictl` conversions are detected even when it exits with code 0: empty/invalid `.wlshm` files are discarded and the failure is reported instead of being imported.
- Lets you choose the output directory (or a `.wlshm` file as destination) and import the result when finished.

### Environments / Inspector
- Displays detected environments and their data (ID, name, size, pets).
- Allows selecting the target environment for actions.

### Settings
- **Language**: Español / English.
- **Theme**: Light / Dark (saved and restored on startup).
- Session/plugin configuration and window interaction (IE mode), depending on `wl_shimeji` support.
- Autostart and service options.

### System
- Tray icon with quick actions.
- Closing the window with the "X" (or Alt+F4) actually terminates the GUI process, so its icon disappears from the dock/taskbar; the tray host process stays alive.
- **Show / Settings** in the tray relaunches the window; **Exit/Quit** terminates the application.
- Works with any compositor (Wayland or X11): it does not rely on proprietary shortcuts or minimized windows.
- Log of command output.
- File dialogs with native support on KDE (`kdialog`/`zenity`) and a fallback.

## Requirements

- Linux (tested on Wayland/KDE Plasma).
- `shimejictl` available in `PATH`.
- Stable Rust to build (`edition 2024`).

## Build and run

```bash
cargo build --release
cargo run
```

Release binary:

```bash
./target/release/shimeji-gui-rs
```

## Compatibility

The app maintains compatibility with existing `wl_shimeji` paths and formats:

- `~/.config/wl_shimeji`
- `~/.local/share/wl_shimeji/shimeji-overlayd.conf`
- `*-support.conf` files
- `ie_mode`, autostart/desktop, `shimejictl`, `.wlshm` packages
