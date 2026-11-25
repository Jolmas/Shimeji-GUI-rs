# Original behavior map: `original_gui.py`

All line citations refer to `/home/luis/git/shimeji-gui-rs-rust/original_gui.py` (1982 lines). This is a factual reading of the Python original; no code was changed.

## 1. Application entry point and global state

- The file is an executable Python 3 script (`#!/usr/bin/env python3`) described as a PySide6/Qt6 GUI for `wl_shimeji` on Wayland/KDE Plasma, communicating with `shimejictl` (`original_gui.py:1-7`).
- Imports include `pty`, `select`, `subprocess`, `threading`, `tarfile`, `json`, `tempfile`, `shutil`, `PIL.Image`, and PySide6 widgets (`original_gui.py:9-34`).
- Constants:
  - `APP = 'wl_shimeji GUI'`, `APP_ID = 'shimeji-gui-rs'` (`original_gui.py:36-37`).
  - `HOME = Path.home()` (`original_gui.py:38`).
  - `DATA = ~/.local/share/wl_shimeji` (`original_gui.py:39`).
  - `CONFIG = ~/.config/wl_shimeji` (`original_gui.py:40`).
  - `AUTOSTART = ~/.config/autostart/shimeji-gui-rs.desktop` (`original_gui.py:41`).
  - `DESKTOP = ~/.local/share/applications/shimeji-gui-rs.desktop` (`original_gui.py:42`).
- Global pixmap cache `_PIXMAP_CACHE` avoids repeated image decoding (`original_gui.py:44-45`).
- Plugin configuration root `PLUGIN_CONFIG_DIR`: if `XDG_CONFIG_HOME` is non-empty and starts with `/`, it is used; otherwise `~/.config/wl_shimeji` (`original_gui.py:50-51`).
- Daemon config `DAEMON_CONF = DATA / 'shimeji-overlayd.conf'` (`original_gui.py:52`).
- Supported plugin sessions:
  - `labwc` → label `Labwc`, plugin dir `~/.local/lib/wl_shimeji_labwc` (`original_gui.py:54-55`).
  - `umbriel` → label `Umbriel`, plugin dir `~/.local/lib/wl_shimeji_umbriel` (`original_gui.py:56-58`).
  - Plugin config files: `<PLUGIN_CONFIG_DIR>/labwc-support.conf` and `<PLUGIN_CONFIG_DIR>/umbriel-support.conf`; `UMBRIEL_IE_CONF` is a compatibility alias for the Umbriel file (`original_gui.py:59-63`).
- IE modes presented to the user (`IE_MODES`): `active` = “Ventana activa (recomendado)”, `focused` = “Ventana enfocada del workspace”, `largest` = “Ventana visible más grande”, `none` = “Ninguna (desactivar interacción con ventanas)” (`original_gui.py:65-70`).
- IE mode aliases (`IE_MODE_ALIASES`): `active`/`seat` → `active`; `focused`/`workspace` → `focused`; `largest`/`biggest`/`max` → `largest`; `none`/`off`/`disabled` → `none` (`original_gui.py:71-76`).
- `normalize_ie_mode()` lowercases/strips input and maps through aliases; returns `None` for empty/unknown values (`original_gui.py:79-83`).
- Main entry:
  - If `shimejictl` is not found on `PATH`, print `Error: no se encontró el ejecutable shimejictl en PATH.` to stderr and return 1 (`original_gui.py:1969-1972`).
  - Otherwise create `QApplication`, set application name to `APP`, set `setQuitOnLastWindowClosed(False)`, show `MainWindow`, enter `app.exec()` (`original_gui.py:1973-1978`).
  - `if __name__ == '__main__': raise SystemExit(main())` (`original_gui.py:1981-1982`).

## 2. Config files, environment variables, and session/plugin logic

### 2.1 Generic and daemon config

- `read_config_value(path, key)` reads UTF-8 text, strips `#` comments from each line, splits on the first `=`, and returns the stripped value for a case-insensitive key match; returns `None` on `OSError` or no match (`original_gui.py:170-183`).
- `_same_path(a, b)` compares `os.path.realpath(os.path.expanduser(...))` of both arguments; returns `False` on `OSError`/`TypeError` (`original_gui.py:186-190`).
- `detect_session()` joins `XDG_CURRENT_DESKTOP` and `XDG_SESSION_DESKTOP`, lowercases: contains `umbriel` → `umbriel`; contains `labwc` or `wlroots` → `labwc`; else `None` (`original_gui.py:193-203`).
- `effective_session()` reads `plugins_location` from `DAEMON_CONF`; if it matches a known plugin directory via `_same_path`, returns that session key; otherwise `detect_session() or 'labwc'` (`original_gui.py:206-212`).
- `save_plugins_location(session)`:
  - Invalid session falls back to `labwc`; target is the corresponding plugin directory (`original_gui.py:215-219`).
  - Creates parent directory, reads existing lines or starts empty on `OSError` (`original_gui.py:220-224`).
  - For each line, strips comments; if key is `plugins_location`, writes `plugins_location=<absolute target>` for the first occurrence and drops all other occurrences; all other lines/comments are preserved verbatim (`original_gui.py:226-237`).
  - If missing, appends `plugins_location=<target>` (`original_gui.py:239-240`).
  - Writes UTF-8 text joined with `\n` plus a trailing newline (`original_gui.py:241`).

### 2.2 Plugin IE-mode config and env vars

- `read_ie_mode(session)` looks up `PLUGIN_CONFS[session]`; returns `None` for unknown session (`original_gui.py:86-90`).
  - Reads UTF-8, strips `#` comments, requires `=`, accepts key `ie_mode` or `<session>_ie_mode` case-insensitively, normalizes the value, returns the first valid mode (`original_gui.py:91-104`).
  - `read_umbriel_ie_mode()` is a compatibility wrapper for `read_ie_mode('umbriel')` (`original_gui.py:107-109`).
- `effective_ie_mode(session='umbriel')` checks env var `<SESSION>_IE_MODE` (i.e. `LABWC_IE_MODE` or `UMBRIEL_IE_MODE`) first; then the plugin config file; then defaults to `active` (`original_gui.py:112-119`).
- `save_plugin_ie_mode(session, mode)`:
  - Invalid session falls back to `labwc`; invalid/unknown mode falls back to `active` (`original_gui.py:129-131`).
  - Path is `PLUGIN_CONFS[session]`; parent created with `mkdir(parents=True, exist_ok=True)`; existing lines read or empty on `OSError` (`original_gui.py:132-137`).
  - For existing `ie_mode` / `<session>_ie_mode` keys, first occurrence is replaced with `ie_mode=<mode>` and subsequent occurrences dropped; other lines/comments preserved (`original_gui.py:139-150`).
  - If no key existed and the file was empty/missing, adds two comment lines: `# <Label>Support: ventana en la que las mascotas pueden pararse.` and `# active | focused | largest | none`, then appends `ie_mode=<mode>` (`original_gui.py:152-159`).
  - Writes UTF-8 with `\n` and trailing newline, then exports `os.environ[f'{session.upper()}_IE_MODE'] = mode` so overlays launched by this GUI see it (`original_gui.py:161-162`).
- `save_umbriel_ie_mode(mode)` is a compatibility wrapper for `save_plugin_ie_mode('umbriel', mode)` (`original_gui.py:165-167`).
- Environment variables read/written:
  - `XDG_CONFIG_HOME` read for plugin config root (`original_gui.py:50-51`).
  - `LABWC_IE_MODE` / `UMBRIEL_IE_MODE` read (`original_gui.py:117-119`) and written (`original_gui.py:162`).
  - `XDG_CURRENT_DESKTOP`, `XDG_SESSION_DESKTOP` read for session detection (`original_gui.py:195-198`).
  - `PATH` indirectly via `shutil.which('shimejictl')` (`original_gui.py:1970`).

### 2.3 Autostart .desktop file

- Autostart checkbox is initialized from `AUTOSTART.exists()` (`original_gui.py:1011-1012`).
- On save, parent directory is created (`original_gui.py:1163`).
  - If enabled and `DESKTOP` exists, it is copied with `shutil.copy2(DESKTOP, AUTOSTART)` (`original_gui.py:1164-1166`).
  - If enabled and `DESKTOP` does not exist, exact generated content (`original_gui.py:1168-1176`):
    ```ini
    [Desktop Entry]
    Type=Application
    Name=wl_shimeji GUI
    Exec=shimeji-gui-rs
    Icon=applications-games
    X-KDE-autostart-after=panel
    ```
  - If unchecked and `AUTOSTART` exists, it is unlinked (`original_gui.py:1177-1178`).
  - Any exception adds error `Error al configurar el inicio automático: <e>` (`original_gui.py:1179-1180`).

## 3. Command execution: PTY, sync, async, threads

### 3.1 `run_ctl_sync(*args, input_str=None, timeout=12)`

- Executes `shimejictl` through a pseudo-terminal specifically to avoid `os.get_terminal_size()` failures when no TTY is attached (`original_gui.py:244-249`).
- Opens `pty.openpty()`, sets master non-blocking, spawns `['shimejictl', *args]` with stdin/stdout/stderr all attached to the slave and `close_fds=True`, then closes the slave (`original_gui.py:250-260`).
- If `input_str` is provided, encodes UTF-8 and writes once to master; `OSError` is ignored (`original_gui.py:261-265`).
- Reads with `select.select(..., 0.1)` and `os.read(master, 4096)`; on process exit drains remaining bytes with `select(..., 0.02)` until no data or `OSError` (`original_gui.py:267-296`).
- Error handling during reads: `EAGAIN`/`EWOULDBLOCK` ignored; `EIO`/`EBADF` ends loop; other `OSError` is raised (`original_gui.py:276-282`).
- Waits with `p.wait(timeout=timeout)` (default 12s), decodes bytes as UTF-8 with `replace`, normalizes `\r\n` and `\r` to `\n`, returns `(p.returncode, output)` (`original_gui.py:298-301`).
- Always closes master in `finally`, ignoring `OSError` (`original_gui.py:302-306`).

### 3.2 `CtlAsyncWorker(QThread)`

- Non-blocking worker for commands that stream output or wait for interactive on-screen clicks, e.g. `summon -s`, `dismiss --select`, `convert` (`original_gui.py:309-313`).
- Signals: `output_received = Signal(str)`, `command_finished = Signal(int, str)` (`original_gui.py:314-315`).
- Constructor stores `args`, optional `input_str`, `timeout=120`, `_proc`, `_master`, `_cancelled=False` (`original_gui.py:317-324`).
- `cancel()` sets `_cancelled=True` and calls `self._proc.terminate()` if still running, ignoring `OSError` (`original_gui.py:326-332`).
- `run()` mirrors `run_ctl_sync`: PTY, non-blocking master, same `shimejictl` Popen, optional UTF-8 input, 0.1s select loop, 4096-byte reads, 0.02s drain, CRLF normalization, per-chunk `output_received.emit(text)` (`original_gui.py:334-387`).
- On cancellation: does not wait, returns `-1`, emits `\n[Operación cancelada por el usuario]\n` (`original_gui.py:389-394`).
- On normal completion: waits with worker timeout, emits `(returncode, total_output)`; on exception emits `\n[Error de ejecución: {e}]\n` and `command_finished.emit(1, str(e))` (`original_gui.py:396-400`).
- Finally closes master, clears `_master` and `_proc` (`original_gui.py:401-407`).

### 3.3 Other execution paths

- ZIP import in `add_role()` uses a plain `subprocess.run(['shimejictl', 'convert', f, '-O', td, '-f'], input='A\n', text=True, capture_output=True)` (no PTY), then logs stdout+stderr (`original_gui.py:1758-1761`).
- Batch summon with count > 1 spawns a daemon `threading.Thread` that loops `run_ctl_sync(*base_args)` `count` times and uses `QTimer.singleShot(0, ...)` to update status/refresh environments on the GUI thread (`original_gui.py:1671-1683`).
- `restart_daemon()` spawns a daemon `threading.Thread`: `run_ctl_sync('stop')`, `time.sleep(1.0)`, `run_ctl_sync('environment', 'list')`, then `QTimer.singleShot(0, ...)` to set status and call `refresh()`/`refresh_environments()` (`original_gui.py:1894-1909`). Log text says `$ shimejictl stop && restart`, but no `restart` command is actually executed (`original_gui.py:1896`, compare `1899-1902`).
- `MainWindow.run_async()` refuses a new command if `current_worker.isRunning()`, warns with Spanish text, sets status, enables cancel, logs `$ shimejictl ...`, creates `CtlAsyncWorker(args, parent=self)`, connects output to log, and on completion disables cancel, clears worker, sets success/error status, logs `[Comando finalizado]` if output empty, invokes optional callback, then calls `refresh_environments()` (`original_gui.py:1490-1517`).
- `cancel_current_command()` calls worker `cancel()`, clears `current_worker`, disables cancel button, sets status `Se ha solicitado interrumpir el comando actual` (`original_gui.py:1519-1524`).

## 4. Parsers and helper behavior

- `load_pixmap(file_path)`: returns `None` for empty/nonexistent path; caches by `str(file_path.resolve())`; uses `PIL.Image.open(...).convert('RGBA')` → raw bytes → `QImage(..., Format_RGBA8888)` → `QPixmap.fromImage`; falls back to `QPixmap(str(file_path))`; caches non-null results (`original_gui.py:410-442`).
- `find_preview(name)`: needles are lowercased `name` and `shimeji.<name>`; searches `DATA/shimejis`, `DATA/prototypes`, `DATA` in that order, then `~/.config/wl_shimeji`, for `*.qoi`, `*.png`, `*.webp` with `rglob`, substring-matching lowercased paths (`original_gui.py:445-478`). Candidate scoring prefers `shime1.qoi`, `shime1.png`, `idle_01.qoi`, `idle_01.png`, `stand_01.qoi`, `stand_01.png`, then shortest path (`original_gui.py:480-489`).
- `prototype_names(text)`: strips leading box/numbering markers such as `|`, `` ` ``, `-`, `+`, `*`, `>`, digits with `.)` separators and whitespace; skips lines containing `prototype`, `available`, `object id`, `end`, `id:`, `author:`, `version:`; deduplicates preserving order (`original_gui.py:492-504`).
- `parse_section(text, heading)`: activates after exact `<heading>:`, stops at a capitalized `Key:`-style line, `Object id:`, or `[END]`; collects `- item` / `* item` lines; deduplicates (`original_gui.py:507-523`).
- `filter_behaviors(name, raw_behaviors)`: reads the first existing `behaviors.json` among `DATA/shimejis/Shimeji.<name>`, `DATA/shimejis/<name>`, `DATA/prototypes/Shimeji.<name>`, `DATA/prototypes/<name>`; loads `definitions` array into `name → definition`; drops behaviors starting with `_`, containing `condition` case-insensitively, equal to `leavewall`, or definitions flagged `hidden`/`is_conditioner` (`original_gui.py:526-562`).
- `proto_info(name)`: runs `prototypes info <name>`; if rc==0 parses `Behaviors` and `Actions` sections and filters behaviors; returns `(behaviors, actions, raw_output)` (`original_gui.py:565-571`).
- `prototype_store_path(info)`: extracts first multiline match of `^\s*Prototype store path:\s*(.+?)\s*$` or empty string (`original_gui.py:574-577`).
- `existing_prototype_names()`: runs `prototypes list -d`; on rc==0 collects `^\s*Name:\s*(.+?)\s*$` names; also adds directory names under `DATA/shimejis` and `DATA/prototypes` (`original_gui.py:663-675`).
- `unique_prototype_name(base, used)`: strips base, strips previous numeric suffix, starts at 1, returns base at 1 else `<stem>_<n>`, increments until unused (`original_gui.py:678-688`).
- `renamed_display_name(original, new_internal, old_internal)`: keeps the same numeric suffix on the visible name as the new internal name; if no suffix, returns new internal name (`original_gui.py:691-703`).
- `parse_environments(text)`: regex parses `Environment(id=N, name=..., description=..., pos=(x,y), size=(w,h), scale=f, with N mascots)`; returns dicts with `id` int, stripped name/description, `pos` as `(x, y)`, `size` as `w x h`, `scale` as 2-decimal string (default `1.00`), `mascots` int (default 0) (`original_gui.py:751-773`).

## 5. `.wlshm` format and collision-safe import

- A `.wlshm` is documented as a 512-byte `WLPK` header followed by an uncompressed tar; the header contains the internal prototype name used by the daemon as storage path, while `manifest.json` contains the visible name (`original_gui.py:580-589`).
- Constants: `WLPK_MAGIC = b'WLPK'`, `WLPK_HEADER_SIZE = 512` (`original_gui.py:591-592`).
- `read_wlshm(path)`:
  - Reads all bytes; returns `None` if file missing/short or magic mismatch (`original_gui.py:595-602`).
  - Header layout: byte 0-3 `WLPK`; byte 4 `name_len`; next `name_len` bytes UTF-8 name; next byte `version_len`; next `version_len` bytes UTF-8 version; `IndexError` returns `None` (`original_gui.py:604-612`).
  - Opens remaining bytes as tar `r:` and tries to load `manifest.json` as JSON (UTF-8 replace); manifest errors are swallowed and manifest defaults to `{}` (`original_gui.py:614-621`).
  - Returns `{'raw': raw, 'name': name, 'version': version, 'manifest': manifest}` (`original_gui.py:623`).
- `write_renamed_wlshm(source, dest, new_name, display_name)`:
  - Calls `read_wlshm`; invalid package raises `ValueError('el archivo no es un paquete .wlshm válido')` (`original_gui.py:626-630`).
  - Encodes new name/version; rejects empty or >255-byte internal name with `ValueError('el nombre interno debe tener entre 1 y 255 bytes')`; rejects header >512 bytes (`original_gui.py:632-639`).
  - Header is `WLPK + len(name) byte + name + len(version) byte + version`, zero-padded to 512 bytes (`original_gui.py:636-640`).
  - Manifest copy: sets `manifest['name'] = new_name`; if `display_name` provided sets `manifest['display_name']`; serializes with `json.dumps(..., ensure_ascii=False)` UTF-8 (`original_gui.py:642-646`).
  - Rebuilds tar in `USTAR_FORMAT`, copies all members, replaces `manifest.json` payload and adjusts its `size`, preserves other members (`original_gui.py:648-658`).
  - Writes `header + tar bytes` to `dest` (`original_gui.py:660`).
- `prepare_import_files(paths, workdir)`:
  - Calls `existing_prototype_names()`; files without valid internal name are passed through unchanged (`original_gui.py:706-721`).
  - If internal name already used, computes `unique_prototype_name`, adjusted display name, sanitizes filename with `re.sub(r'[^\w.\-]+', '_', new_name).strip('._') or 'shimeji'`, writes `<workdir>/<safe>.wlshm` (adding `_2`, `_3`, ... if taken), renames package (`original_gui.py:723-738`).
  - On `OSError`, `ValueError`, `tarfile.TarError`, appends notice `No se pudo renombrar «<old>»: <exc>` and keeps original file (`original_gui.py:739-742`).
  - On success adds new name to `used` and notice `«<old>» ya existía: se importará como «<new>».` (`original_gui.py:744-746`).
  - Returns `(prepared_paths, notices)`; purpose is to avoid `--force` overwriting an existing character (`original_gui.py:706-712`).

## 6. Windows and dialogs

### 6.1 `ConverterDialog` (`original_gui.py:776-904`)

- Window title `Convertidor de Shimeji-EE a wl_shimeji`, size 600x420 (`original_gui.py:778-782`).
- Controls (`original_gui.py:784-826`):
  - Source `QLineEdit`; buttons `Seleccionar archivo ZIP`, `Seleccionar directorio descomprimido`; row label `Archivo/directorio de origen:` (`original_gui.py:787-794`).
  - Output `QLineEdit` default `~/Downloads`; button `Examinar...`; row label `Directorio de salida:` (`original_gui.py:796-801`).
  - Checkbox `Sobrescribir archivos existentes (-f / --force)`, checked by default (`original_gui.py:803-805`).
  - Checkbox `Importar automáticamente a la biblioteca al terminar`, checked by default (`original_gui.py:807-809`).
  - Read-only log `QTextEdit` with placeholder `Registro de conversión...` (`original_gui.py:813-816`).
  - Buttons `Iniciar conversión`, `Cancelar`, `Cerrar`; cancel initially disabled (`original_gui.py:818-834`).
- File dialogs:
  - ZIP: `Seleccionar archivo ZIP de Shimeji-EE`, starts at `HOME`, filter `Archivos ZIP (*.zip)` (`original_gui.py:836-839`).
  - Source dir: `Seleccionar directorio descomprimido de Shimeji-EE`, starts at `HOME` (`original_gui.py:841-844`).
  - Output dir: `Seleccionar directorio de salida`, starts at current output text or `HOME` (`original_gui.py:846-849`).
- `start_conversion()` (`original_gui.py:851-874`):
  - Validates source exists and output non-empty; warnings `Selecciona un archivo ZIP o directorio de origen válido.` / `Especifica un directorio de salida.` (`original_gui.py:854-859`).
  - Creates output dir with `os.makedirs(out, exist_ok=True)` (`original_gui.py:860`).
  - Builds `convert <src> -O <out>` and appends `-f` if overwrite checked (`original_gui.py:862-864`).
  - Logs `$ shimejictl <args>`, disables start, enables cancel (`original_gui.py:866-868`).
  - Starts `CtlAsyncWorker(args, input_str='A\n')` (timeout default 120s) so all prototypes are converted; streams output into log (`original_gui.py:870-874`).
- `cancel_conversion()` calls worker `cancel()` (`original_gui.py:876-878`).
- `on_conversion_finished(rc, out)` (`original_gui.py:880-904`):
  - Re-enables start, disables cancel (`original_gui.py:881-882`).
  - rc==0: counts sorted `*.wlshm` in output dir, logs `¡Conversión completada! El directorio de salida contiene N archivos .wlshm.` (`original_gui.py:883-886`).
  - If auto-import and files exist: creates `TemporaryDirectory(prefix='shimeji-gui-rs-import-')`, calls `prepare_import_files`, runs sync `prototypes import <prepared...>`, logs `$ shimejictl prototypes import ...\n<output>`, appends notices (`original_gui.py:887-893`).
  - Import success: info `Se han convertido e importado correctamente N personajes.` plus notices; calls parent `refresh()` if available (`original_gui.py:894-899`).
  - Import failure: warning `La conversión se realizó correctamente, pero se produjo un error al importar los personajes:\n<first 1000 chars>` (`original_gui.py:900`).
  - Auto-import disabled/no files: info `Conversión completada.` (`original_gui.py:901-902`).
  - rc!=0: warning `La conversión devolvió el código de error <rc>. Consulta el registro.` (`original_gui.py:903-904`).
- `btn_close` calls `accept()` (`original_gui.py:833`).

### 6.2 `SettingsDialog` (`original_gui.py:907-1197`)

- Title `Centro de ajustes - wl_shimeji`, size 620x520; four tabs (`original_gui.py:917-924`).
- Docstring lists covered config keys: `BREEDING`, `DRAGGING`, `CURSOR_POSITION`, `WINDOW_INTERACTIONS`, `WINDOW_THROWING`, `WINDOW_THROW_POLICY`, `AFFORDANCES`, `DISMISS_ANIMATIONS`, multimonitor keys, visual/scale keys, limits/input, raw view (`original_gui.py:907-916`).

Tab 1 `Interacción y comportamiento` (`original_gui.py:926-957`):
  - Checkboxes: `Permitir que los personajes se multipliquen (Multiplication / BREEDING)`, `Permitir arrastrar a los personajes (Dragging / DRAGGING)`, `Seguir la posición del cursor (Cursor Position / CURSOR_POSITION)`, `Activar interacciones específicas del personaje (Affordances / AFFORDANCES)`, `Activar animaciones de desaparición (Dismiss Animations / DISMISS_ANIMATIONS)`, `Permitir la interacción con ventanas de aplicaciones (Window Interactions / WINDOW_INTERACTIONS)`, `Permitir agarrar y lanzar ventanas (Window Throwing / WINDOW_THROWING)` (`original_gui.py:929-935`).
  - Combo `ie_mode` with labels from `IE_MODES`; tooltip says value is saved in selected session plugin config (`labwc-support.conf` or `umbriel-support.conf`) and IE actions also require `WINDOW_THROWING=true` in wl_shimeji 0.0.2 (`original_gui.py:936-944`).
  - Combo `throw_policy` with `looping`, `clamp`, `bounce`, `ignore` (`original_gui.py:945-946`).
  - Row label `Mascotas sobre ventanas (plugin de la sesión):`; row label `Política al lanzar ventanas fuera de la pantalla:` (`original_gui.py:955-956`).

Tab 2 `Multimonitor y renderizado` (`original_gui.py:959-990`):
  - Checkboxes: `Permitir lanzar ventanas entre monitores (Allow Throwing Multihead)`, `Permitir arrastrar entre monitores (Allow Dragging Multihead)`, `Unificar el espacio de todas las salidas (Unified Outputs)` (`original_gui.py:962-964`).
  - `MASCOT_SCALE` double spin range -1.0..5.0, step 0.25, special text `Seguir la escala del sistema (-1.0)` (`original_gui.py:966-969`).
  - `OPACITY` double spin range -1.0..1.0, step 0.1, special text `Completamente opaco (-1.0)` (`original_gui.py:971-974`).
  - `WLR_SHELL_LAYER` combo `overlay`, `top` (`original_gui.py:976-977`).
  - `INTERPOLATION_FRAMERATE` spin range -1..1000, special text `Seguir la tasa de refresco (-1)`; row label notes `0=desactivado, -1=seguir refresco` (`original_gui.py:979-989`).

Tab 3 `Sistema y entrada` (`original_gui.py:992-1020`):
  - Session combo with `Labwc`/`Umbriel` items; tooltip explains saving adjusts `plugins_location` in `~/.local/share/wl_shimeji/shimeji-overlayd.conf` so only the matching plugin loads (`original_gui.py:995-1002`).
  - `session_path` word-wrapped label updated on combo change via `update_session_path()` (`original_gui.py:1003-1005`, `1046-1049`).
  - `MASCOT_LIMIT` spin range 1..2048, default 512; checkbox `Activar entrada de tableta de dibujo (se recomienda desactivarla en KWin)`; checkbox `Iniciar la interfaz automáticamente al iniciar sesión en KDE Plasma` initialized from `AUTOSTART.exists()` (`original_gui.py:1007-1012`).
  - Row labels `Sesión / compositor activo:`, `Directorio de plugins:`, `Número máximo de personajes (MASCOT_LIMIT):` (`original_gui.py:1014-1016`).

Tab 4 `Configuración completa`: read-only `QTextEdit` (`original_gui.py:1022-1028`).

Buttons: `Volver a leer` → `load_config`, `Aplicar y guardar` → `apply_config`, `Cerrar` → `accept`; `load_config()` called in constructor (`original_gui.py:1030-1044`).

`load_config()` (`original_gui.py:1051-1117`):
  - Runs sync `config list`, puts raw output into tab 4 (`original_gui.py:1052-1053`).
  - Parses lines with regex `\(([^)]+)\)\s*=\s*(.+)$` into key→value (`original_gui.py:1055-1060`).
  - `parse_bool` treats `1`, `true`, `yes`, `on` case-insensitively as true (`original_gui.py:1062-1063`).
  - Defaults on missing values: breeding/dragging/cursor/affordances/dismiss animations true; window interactions/window throwing false; multihead/unified outputs true; scale -1.0; opacity -1.0; layer overlay; interpolation 0; mascot limit 512; tablets false (`original_gui.py:1065-1117`).
  - Session/IE mode: `effective_session()` selects combo; `effective_ie_mode(session)` selects IE combo (`original_gui.py:1073-1081`).
  - Bad numeric values fall back to defaults (`original_gui.py:1092-1115`).
  - `load_config()` does not touch the `autostart` checkbox; it is only initialized once in the constructor from `AUTOSTART.exists()` (`original_gui.py:1011-1012`).

`apply_config()` (`original_gui.py:1119-1197`):
  - Builds `settings_map` with values formatted as lowercase Python booleans (`true`/`false`), current text for policy/layer, `f"{value:.2f}"` for scale/opacity, decimal int strings for interpolation/limit (`original_gui.py:1120-1138`).
  - For each key runs sync `config set <key> <value>`; nonzero rc appends `<key>: <stripped output>` to errors (`original_gui.py:1140-1144`).
  - Writes `plugins_location` via `save_plugins_location(session)`, errors appended as `Sesión / plugins_location: <e>` (`original_gui.py:1146-1153`).
  - Writes IE mode via `save_plugin_ie_mode(session, ...)`, errors appended as `Modo de ventanas (<session>): <e>` (`original_gui.py:1155-1159`).
  - Handles autostart as described in section 2.3 (`original_gui.py:1161-1180`).
  - Calls `load_config()` again (`original_gui.py:1182`).
  - No errors: message `La configuración se ha aplicado correctamente.\nLa selección de sesión y el modo de ventanas se aplican al reiniciar el servicio.`; if parent has callable `restart_daemon`, asks `¿Reiniciar el servicio ahora?` with Yes/No, default Yes, and calls it on Yes; else shows info (`original_gui.py:1183-1195`).
  - Errors: warning `No se pudieron guardar algunas opciones:\n<errors>` (`original_gui.py:1196-1197`).

### 6.3 `MainWindow` (`original_gui.py:1200-1966`)

- Window title `wl_shimeji GUI`, size 920x680; tracks `names`, `environments`, `current_worker` (`original_gui.py:1201-1208`).
- Horizontal splitter: left library (stretch 1), right tabs (stretch 2) (`original_gui.py:1210-1215`, `1414-1416`).

Left panel (`original_gui.py:1217-1251`):
  - Search field with placeholder `Buscar personajes...` and clear button (`original_gui.py:1222-1227`).
  - `QListWidget` prototype list, icon size 64x64, min width 260 (`original_gui.py:1229-1232`).
  - Buttons: `Actualizar` (refresh), `Recargar personajes` (tooltip `Envía una señal de recarga al daemon (prototypes reload-all)`), `Añadir personaje`, `Exportar`, `Eliminar personaje` (`original_gui.py:1234-1249`).

Right tab 1 `Personajes y control` (`original_gui.py:1261-1378`):
  - Preview label initial text `Selecciona un personaje`, centered, min 180x160, dashed border; role title initial `Ningún personaje seleccionado`, bold 14pt; role meta grey; behavior combo row label `Comportamiento inicial:` (`original_gui.py:1265-1291`).
  - Group `Invocar personajes (Summon)`: quantity spin 1..50 default 1; target screen combo with item `Aleatoria / todos los entornos`; X spin -1..9999 default -1 special `Aleatoria (-1)`; Y spin -1..9999 default -1 special `Predeterminada (-1)`; buttons `Invocar ahora` (bold) and `Elegir posición en pantalla (-s)` (`original_gui.py:1293-1333`).
  - Group `Eliminar y limpiar (Dismiss)`: buttons `Elegir y eliminar uno (-s)`, `Elegir y eliminar los de su tipo (-fs -s)`, `Elegir y eliminar los demás (-o -s)`, `Limpiar la pantalla seleccionada`, `Eliminar todos los personajes (--all)` (red/bold); ID input placeholder `p. ej.: 1`, button `Eliminar por ID` (`original_gui.py:1335-1364`).
  - Group `Cambio de comportamiento y rescate de ventanas`: button `Elegir un personaje y cambiar a este comportamiento`; button `Restaurar ventanas capturadas (Restore Windows)` with blue style and tooltip explaining captured/launched windows return to normal desktop view (`original_gui.py:1366-1376`).

Right tab 2 `Entornos de pantalla (Environments)` (`original_gui.py:1380-1404`):
  - Hint label `wl_shimeji ejecuta un entorno de pantalla (Environment) independiente en cada salida de Wayland.` (`original_gui.py:1384-1386`).
  - 6-column table headers `ID del entorno`, `Resolución (Ancho x Alto)`, `Coordenadas`, `Escala`, `Personajes actuales`, `Descripción`; stretched columns, row selection (`original_gui.py:1388-1392`).
  - Buttons `Actualizar lista de pantallas`, `Cerrar la capa de la pantalla seleccionada (-i ID)`, `Elegir pantalla para cerrar la capa` (`original_gui.py:1394-1402`).

Right tab 3 `Detalles del personaje (Inspector)`: read-only `QTextEdit` (`original_gui.py:1406-1412`).

Bottom area (`original_gui.py:1418-1448`):
  - Label `Registro de ejecución (Command Output):`, buttons `Interrumpir comando actual` (initially disabled) and `Borrar registro`; monospace 11px read-only log console max height 130 (`original_gui.py:1419-1433`).
  - Status label initial `Listo`; bottom buttons `Convertidor de Shimeji-EE`, `Detener servicio en segundo plano`, `Reiniciar servicio en segundo plano`, `Ajustes...` (`original_gui.py:1435-1447`).

Signal connections (`original_gui.py:1450-1485`):
  - Search filtering, selection changes, library buttons, summon/dismiss buttons, extra actions, environment buttons, cancel/clear log, converter/settings dialogs (both `exec()`), daemon stop/restart, tray setup, initial `refresh()` and `refresh_environments()`.

## 7. User actions → exact `shimejictl` commands

| User action / code path | Command | Citation |
| --- | --- | --- |
| Initial/refresh library (`refresh`, `Actualizar`, after import/reload) | `shimejictl prototypes list` | `original_gui.py:1453`, `1530-1532`, `1485` |
| Select prototype → details/inspector | `shimejictl prototypes info <name>` | `original_gui.py:1576-1591`, `565-571` |
| Collision-safe import name discovery | `shimejictl prototypes list -d` | `original_gui.py:663-668` |
| Add `.wlshm` | `shimejictl prototypes import <prepared>` (async) | `original_gui.py:1735-1754` |
| Add ZIP | `shimejictl convert <zip> -O <temp> -f` with input `A\n`, then `shimejictl prototypes import <prepared...>` | `original_gui.py:1757-1768` |
| Converter dialog start | `shimejictl convert <src> -O <out> [-f]` with input `A\n`; optional `shimejictl prototypes import <prepared...>` | `original_gui.py:862-871`, `887-891` |
| Export selected character | `shimejictl prototypes export -i <name> -o <dest> -f` | `original_gui.py:1778-1787` |
| Delete character (native support) | `shimejictl prototypes --help`, then `shimejictl prototypes remove|delete <name>` | `original_gui.py:1796-1801` |
| Delete character (fallback) | no ctl delete; remove local directory, then `shimejictl prototypes reload-all` | `original_gui.py:1803-1838` |
| Reload all (`Recargar personajes`) | `shimejictl prototypes reload-all` | `original_gui.py:1236-1237`, `1720-1721` |
| Settings load / raw view | `shimejictl config list` | `original_gui.py:1051-1053` |
| Settings save | `shimejictl config set <KEY> <value>` for each key | `original_gui.py:1119-1144` |
| Summon direct/batch | `shimejictl mascot summon [-b <behavior>] [-e <env>] [--position x,y | -x x | -y y] <name>`; batch repeats synchronously | `original_gui.py:1638-1683` |
| Summon with `-s` | `shimejictl mascot summon -s [-b ...] [-e ...] <name>` | `original_gui.py:1650-1669` |
| Tray quick summon | `shimejictl mascot summon <name>` | `original_gui.py:1927-1930` |
| Dismiss one interactive (button/tray) | `shimejictl mascot dismiss -s` | `original_gui.py:1461`, `1936-1937` |
| Dismiss same type interactive (button/tray) | `shimejictl mascot dismiss -fs -s` | `original_gui.py:1462`, `1939-1940` |
| Dismiss others interactive | `shimejictl mascot dismiss -o -s` | `original_gui.py:1463` |
| Dismiss all | `shimejictl mascot dismiss --all` | `original_gui.py:1685-1687`, `1942-1943` |
| Dismiss by ID | `shimejictl mascot dismiss -i <digits>` | `original_gui.py:1689-1694` |
| Dismiss by environment | `shimejictl mascot dismiss -e <env> --all` | `original_gui.py:1696-1708` |
| Change behavior on clicked mascot | `shimejictl mascot set-behavior <behavior>` | `original_gui.py:1710-1715` |
| Restore windows | `shimejictl restore-windows` | `original_gui.py:1717-1718`, `1933-1934` |
| Refresh environments | `shimejictl environment list` | `original_gui.py:1842-1844`, `1902` |
| Close selected environment layer | `shimejictl environment close -i <id>` | `original_gui.py:1873-1888` |
| Interactive environment close | `shimejictl environment close` | `original_gui.py:1473` |
| Stop daemon | `shimejictl stop` | `original_gui.py:1890-1892`, `1949-1950` |
| Restart daemon | `shimejictl stop`, sleep 1s, `shimejictl environment list` (log misleadingly says `stop && restart`) | `original_gui.py:1894-1909` |

## 8. Main window method behavior details

- `log(text)` appends `text.rstrip()` to log console (`original_gui.py:1487-1488`).
- `current_prototype_name()` returns selected item's `Qt.UserRole` or `''` (`original_gui.py:1526-1528`).
- `refresh()` clears list, runs `prototypes list`; on rc!=0 sets status `No se pudo conectar ni leer la lista de personajes; comprueba que shimejictl esté instalado.` and logs `No se pudieron leer los personajes:\n<out>` (`original_gui.py:1530-1539`). On success parses names, creates items with preview icons or fallback theme icon `applications-games`, applies current search filter by hiding nonmatches, selects first visible row, sets status `Se han cargado N prototipos de personajes`, rebuilds tray (`original_gui.py:1541-1567`).
- `filter_prototypes(text)` hides items whose internal name does not contain lowercased trimmed text (`original_gui.py:1569-1574`).
- `on_proto_selected(item)` resets behavior combo to `(Comportamiento predeterminado)` with data `''`; no item resets title/meta/preview/inspector (`original_gui.py:1576-1586`). With item: calls `proto_info`, populates behavior combo, extracts `Author:`/`Version:` regexes, sets meta `Autor: ... | Versión: ... | Acciones: N | Comportamientos: N`, scales preview to 200x180 with `Qt.KeepAspectRatio`/`Qt.SmoothTransformation` or shows `(No se encontró vista previa local)`, fills inspector with prototype header, author, version, storage path, actions/behaviors lists and raw `prototypes info` output (`original_gui.py:1588-1636`).
- `summon(select=False)` warns `Selecciona primero un personaje en el panel izquierdo.` if no selection; builds args as described; if `select` or count==1 calls `run_async` with status `Haz clic en la pantalla en el lugar donde quieres invocar al personaje...` or `Invocando al personaje...`; otherwise logs batch and threads (`original_gui.py:1638-1683`).
- `dismiss_all()` confirmation: `¿Seguro que quieres eliminar todos los personajes de la pantalla?` (`original_gui.py:1685-1687`).
- `dismiss_by_id()` warns `Introduce un ID de personaje que contenga solo números.` when input empty/non-digit (`original_gui.py:1689-1694`).
- `dismiss_by_env()` takes env from combo or selected environment table row; warns `Selecciona primero una pantalla concreta en el desplegable «Pantalla de destino» o en la pestaña «Entornos de pantalla».` when missing (`original_gui.py:1696-1707`).
- `change_behavior_select()` warns `Selecciona primero un comportamiento concreto en el desplegable «Comportamiento inicial».` when default/missing (`original_gui.py:1710-1715`).
- `add_role()` (`original_gui.py:1723-1776`):
  - Blocks if another async worker running: `Hay otro comando en curso; espera a que termine antes de añadir un personaje.` (`original_gui.py:1724-1726`).
  - File dialog title `Añadir personaje`, starts at `HOME`, filters `Personajes Shimeji (*.wlshm *.zip);;Paquetes de personajes wl_shimeji (*.wlshm);;ZIP de Shimeji-EE (*.zip);;Todos los archivos (*)` (`original_gui.py:1728-1731`).
  - `.wlshm`: creates temp dir prefix `shimeji-gui-rs-import-`, prepares collision-safe copy, async import; callback removes temp dir, logs notices, refreshes, shows info with notices if any (`original_gui.py:1735-1755`).
  - ZIP: temp dir prefix `shimeji-gui-rs-`, direct `subprocess.run` conversion with `input='A\n'`, logs command/output; failure warning `La conversión ha fallado o no se han encontrado recursos válidos de Shimeji-EE en el ZIP.\n\n<last 2000>`; success import and info `¡Se han convertido e importado correctamente N prototipos de personajes!` or warning `La conversión se completó, pero se produjo un error durante la importación:\n<imp_out>` (`original_gui.py:1757-1776`).
- `export_role()` warns `Selecciona primero en el panel izquierdo el personaje que quieres exportar.`; save dialog default `~/Shimeji.<name>.wlshm`, filter `Paquetes de personajes wl_shimeji (*.wlshm)` (`original_gui.py:1778-1787`).
- `delete_role()` (`original_gui.py:1789-1840`):
  - Confirmation includes note that original ZIP/.wlshm files are unaffected (`original_gui.py:1793`).
  - Fallback cannot parse storage path warning `No se pudo interpretar la ruta de almacenamiento del prototipo de este personaje.`; not found warning includes `No se encontró la carpeta de almacenamiento del personaje en el directorio local de datos:\n<rel>` (`original_gui.py:1804-1820`).
  - Security check `target.relative_to(DATA.resolve())`; failure critical `Restricción de seguridad` / `Eliminación denegada: la ruta no está dentro del directorio exclusivo de wl_shimeji.` (`original_gui.py:1822-1827`).
  - Success logs `Se ha limpiado el directorio de almacenamiento: <path>`, reloads, refreshes, info `El personaje «<name>» se ha eliminado correctamente.`; `OSError` critical `Error en la operación del sistema de archivos: <e>` (`original_gui.py:1829-1840`).
- `refresh_environments()` (`original_gui.py:1842-1871`): on `environment list` rc!=0 silently returns; fills table with id, size, pos, scale, mascots, name/description/`-`; rebuilds summon combo with first empty item and labels `Pantalla #<id> (<size>, <mascots> personajes)`, preserving previous selection.
- `close_selected_environment()` warns `Selecciona en la tabla la pantalla cuya capa quieres cerrar.`; confirmation explains closing equals disconnecting that output and requires daemon restart to recover (`original_gui.py:1873-1888`).
- `stop_daemon()` confirmation warns all characters disappear; callback refreshes environments (`original_gui.py:1890-1892`).

## 9. Tray behavior

- `setup_tray()` returns immediately if system tray unavailable; otherwise creates `QSystemTrayIcon` with theme icon `applications-games`, connects activation on `QSystemTrayIcon.Trigger` to `showNormal()`, shows it, and rebuilds menu (`original_gui.py:1911-1917`).
- `rebuild_tray()` returns if no tray; constructs fresh `QMenu` each time (`original_gui.py:1919-1922`).
  - `Abrir el gestor` → `showNormal()` (`original_gui.py:1923-1924`).
  - Separator; submenu `Invocación rápida` with one action per prototype name → async `mascot summon <name>` with status `<name> invocado` (`original_gui.py:1926-1930`).
  - Separator; `Restaurar ventanas capturadas` → `restore_windows()` (`original_gui.py:1932-1934`).
  - `Elegir y eliminar uno` → async `mascot dismiss -s`; `Elegir y eliminar los de su tipo` → async `mascot dismiss -fs -s`; `Eliminar todos los personajes` → `dismiss_all()` (`original_gui.py:1936-1943`).
  - Separator; `Reiniciar el servicio de personajes` → `restart_daemon()`; `Detener el servicio de personajes` → `stop_daemon()` (`original_gui.py:1945-1950`).
  - Separator; `Ajustes...` opens `SettingsDialog(self).exec()`; `Salir de la interfaz` calls `QApplication.instance().quit()` (`original_gui.py:1952-1957`).
  - Sets menu with `self.tray.setContextMenu(menu)` (`original_gui.py:1959`).
- `rebuild_tray()` is called from `refresh()` so quick-summon entries follow the current prototype list (`original_gui.py:1567`).
- `closeEvent()`: if tray exists and is visible, hides window and ignores close event; otherwise normal close (`original_gui.py:1961-1966`). Combined with `setQuitOnLastWindowClosed(False)`, closing the manager keeps it in the tray (`original_gui.py:1975`).

## 10. Converter flow end-to-end

1. Converter dialog opened from bottom button `Convertidor de Shimeji-EE` (`original_gui.py:1439`, `1478`).
2. User picks ZIP or unpacked Shimeji-EE directory and output dir; `-f` and auto-import are on by default (`original_gui.py:787-809`, `836-849`).
3. Validation and output dir creation; command `shimejictl convert <src> -O <out> [-f]` (`original_gui.py:851-864`).
4. Async PTY worker sends `A\n` to answer “convert all prototypes” prompt; output streams to log (`original_gui.py:866-874`).
5. User can cancel; worker terminates process and emits cancellation line (`original_gui.py:876-878`, `326-332`, `389-394`).
6. On success, counts `*.wlshm` in output dir and optionally imports: temp dir, collision check/rename of internal names, `shimejictl prototypes import ...`, notices and messages (`original_gui.py:880-904`, `706-748`).
7. Add-character flow has a separate synchronous ZIP conversion path with the same `A\n` input (`original_gui.py:1757-1776`).

## 11. Settings flow end-to-end

1. Settings dialog opened from bottom button or tray (`original_gui.py:1442`, `1481`, `1953-1954`).
2. Constructor builds tabs and calls `load_config()` (`original_gui.py:1044`).
3. `config list` output populates raw tab and all widget values with documented defaults (`original_gui.py:1051-1117`).
4. Save applies 17 `config set` commands sequentially, collecting per-key errors (`original_gui.py:1120-1144`).
5. Session selection edits daemon config file directly; IE mode edits the selected session plugin config and exports env var (`original_gui.py:1146-1159`, `215-241`, `122-162`).
6. Autostart checkbox creates/copies/removes the `.desktop` file (`original_gui.py:1161-1180`).
7. Reloads config display, then either offers immediate daemon restart or reports errors (`original_gui.py:1182-1197`).

## 12. Error paths and important Spanish strings

- Command wrapper errors:
  - Async worker emits `[Error de ejecución: <e>]` on exception (`original_gui.py:398-400`).
  - Cancellation emits `[Operación cancelada por el usuario]` (`original_gui.py:394`).
  - `run_async` busy warning: `El comando anterior todavía está en curso; espera a que termine o pulsa «Interrumpir comando actual».` (`original_gui.py:1492-1494`).
  - Empty async output logs `[Comando finalizado]`; nonzero rc status `Error al ejecutar el comando (código de retorno: <rc>)`, success status `Comando completado (código de retorno: 0)` (`original_gui.py:1506-1511`).
- Library:
  - `No se pudo conectar ni leer la lista de personajes; comprueba que shimejictl esté instalado.` (`original_gui.py:1537`).
  - `No se pudieron leer los personajes:\n<out>` (`original_gui.py:1538`).
  - Search placeholder `Buscar personajes...` (`original_gui.py:1224`).
  - Selection-empty default labels `Selecciona un personaje` / `Ningún personaje seleccionado` (`original_gui.py:1267`, `1274`, `1580-1585`).
- Add/export/delete/convert warnings are listed in sections 6.1, 6.3, 8, and 10; key ones:
  - `No se pudo preparar la importación:\n<exc>` (`original_gui.py:1740-1743`).
  - `La conversión ha fallado o no se han encontrado recursos válidos de Shimeji-EE en el ZIP.` (`original_gui.py:1764`).
  - `Error de importación` with `La conversión se completó, pero se produjo un error durante la importación:\n<imp_out>` (`original_gui.py:1776`).
  - Security critical `Restricción de seguridad` (`original_gui.py:1826`).
- Settings:
  - Success/restart question text in `original_gui.py:1184-1193`; aggregated failure `No se pudieron guardar algunas opciones:\n<errors>` (`original_gui.py:1197`).
- Tray/window:
  - `Salir de la interfaz` quits app (`original_gui.py:1956-1957`).
  - Closing window hides to tray when available (`original_gui.py:1961-1966`).
- Internal package/rename errors are Spanish: invalid `.wlshm`, name length, header too long (`original_gui.py:630`, `635`, `639`), collision notice text (`original_gui.py:740`, `746`).
