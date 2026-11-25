# MAP.md

## Goal
Reescribir `~/bin/shimeji-gui-rs` (PySide6/Qt6, 1982 líneas) como app de escritorio Rust con `egui/eframe`, manteniendo compatibilidad funcional y de formatos/configuración.

## Project layout
- Repo: `~/git/shimeji-gui-rs`
- Cargo crate: `shimeji-gui-rs`
- Binary (coexistencia): `shimeji-gui-rs`
- Binary (cutover, cuando se confirme): `shimeji-gui-rs`
- Original reference: `original_gui.py`

## Reference
- Behavior map: `rust/research/original-behavior.md`
- GUI toolkit research: `rust/research/gui-toolkit.md`
- Decisions: `rust/DECISIONS.md`
- Glossary: `rust/GLOSSARY.md`

## ADR index
- ADR-001: forma desktop Rust, `shimejictl` externo — accepted
- ADR-002: toolkit `egui/eframe` + `ksni`/`rfd` — accepted
- ADR-003: alcance completo — accepted
- ADR-004: compatibilidad rutas/formatos — accepted
- ADR-005: equivalencia funcional — accepted
- ADR-006: target Linux Wayland/KDE Plasma — accepted
- ADR-007: coexistencia + checkpoints + cutover confirmado — accepted
- ADR-008: portado adaptado a GUI (lógica pura testeada + UI egui) — accepted
- ADR-009: naming/layout Q7a:A — accepted
- ADR-010: UI bilingüe es/en, default es — accepted

## Unit plan (draft for implementation)
| Unit | State | Notes |
|---|---|---|
| `core::config` - paths XDG, daemon conf, plugins_location, ie_mode, session detection | in-progress | Implementado con tests (13 passing total en lib); falta integrar con UI/app |
| `core::wlshm` - lectura/escritura WLPK+tar+manifest, rename/import seguro | in-progress | Implementado con tests de formato/rename/import; falta conectar con shimejictl/prototype store |
| `core::shimejictl` - modelo de comandos, parser de salida, PTY async runner/cancel/timeout | in-progress | Runner PTY + `CtlCommand` tipado (incl. EnvironmentClose/Stop); falta fixtures reales y validación de flags con shimejictl |
| `core::parsing/domain` - prototipos/entornos/behaviors/config parsing | in-progress | Parsers puros implementados con tests; falta integrar con runner real |
| `ui::app` - shell eframe, estado, navegación, logs, async tasks | in-progress | Esqueleto eframe+i18n+puente async CTL+panel log/prototipos; faltan tabs/dialogs completos |
| `ui::main_window` - library/summon/dismiss/behavior/restore/environments | in-progress | Summon env combo, dismiss screen/all confirm, stop/restart, env close, inspector, search hint, preview; falta validación manual Wayland/KDE y paridad fina restante |
| `ui::dialogs` - converter/settings, file dialogs, confirmaciones | in-progress | Settings/Converter/confirm delete + restart prompt + plugins dir path; falta paridad fina y validación UX |
| `ui::tray` - StatusNotifierItem/kSNI, acciones rápidas | in-progress | ksni blocking tray con labels i18n actualizables, close-to-tray; falta validar icono/nombres en KDE |
| `packaging` - desktop/autostart/iconos, docs de instalación/cutover | in-progress | `packaging/shimeji-gui-rs.desktop` y README de coexistencia/cutover añadidos; falta validación real en KDE/Wayland |

## Checkpoints
1. Config/wlshm/parsing + tests
2. Runner shimejictl + modelo comandos + tests
3. UI principal (main window) con flujos core
4. Dialogs/settings/tray/autostart + i18n
5. Paridad final, checklist manual Wayland/KDE, cutover

## Open questions
- Ninguna bloqueante para iniciar bloque 1; confirmar MSRV/toolchain solo si hay restricción (default: stable reciente).

## Out of scope
- Reescribir `shimejictl`, `wl_shimeji` o plugins compositor.
- Paridad visual pixel-perfect con Qt.
- X11/otros desktops como target primario inicial.
