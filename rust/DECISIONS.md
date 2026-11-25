# DECISIONS.md

## ADR-001: Forma de app de escritorio Rust
Status: accepted (2026-11-25)
Context: El original es una GUI PySide6/Qt6 de ~1982 líneas (`original_gui.py`) que orquesta `shimejictl` por PTY/subprocess y gestiona mascotas/prototipos/entornos/configuración en Wayland/KDE Plasma.
Decision: Reescribir como aplicación de escritorio nativa en Rust, manteniendo los flujos de usuario. `shimejictl` permanece como backend externo.
Consequences: La UI se reimplementa en Rust; la lógica de orquestación/config/parsing se puede portar y verificar por unidades.

## ADR-002: Toolkit GUI = egui/eframe
Status: accepted (2026-11-25)
Context: El usuario eligió egui. La investigación primaria (`rust/research/gui-toolkit.md`) recomienda gtk4-rs/libadwaita para mejor ajuste nativo; egui/eframe tiene buen soporte Wayland vía winit pero no widgets nativos KDE, sin tray ni diálogos nativos integrados.
Decision: Usar `egui/eframe`. Integrar tray con `ksni` (preferido en KDE/SNI) o `tray-icon` solo si hace falta; diálogos de archivo con `rfd`.
Consequences: Mayor trabajo de integración de shell de escritorio; look no nativo. Se validará en Wayland/KDE Plasma.

## ADR-003: Alcance completo
Status: accepted (2026-11-25)
Context: El original incluye gestión de mascotas/prototipos/entornos, conversión/import/export `.wlshm`, ajustes, tray, autostart, logs y operaciones async/cancelables.
Decision: Portar todas las funciones visibles; excluir solo funcionalidad muerta/deprecada si se identifica y confirma.
Consequences: El plan cubre Converter/Settings/MainWindow/tray/PTY async/parsing `.wlshm`/config/entornos/manejo de archivos.

## ADR-004: Compatibilidad de rutas y formatos
Status: accepted (2026-11-25)
Context: La app actual lee/escribe `~/.config/wl_shimeji`, `~/.local/share/wl_shimeji/shimeji-overlayd.conf`, `*-support.conf`, `ie_mode`, desktop/autostart e interactúa con `shimejictl` y formatos `.wlshm`/tar/zip.
Decision: Mantener compatibilidad con rutas, claves, formatos y semántica observable existentes. Sustitución sin migración manual.
Consequences: Parsers/escritores preservan formato/comentarios donde el original lo hace; rutas XDG/nombres se mantienen.

## ADR-005: Barra de equivalencia funcional
Status: accepted (2026-11-25)
Context: El usuario pidió paridad funcional, no pixel-perfect.
Decision: Equivalencia funcional/observable: mismos efectos, comandos, archivos, errores relevantes y flujos. UI puede adaptarse a egui.
Consequences: Pruebas centradas en comportamiento/efectos secundarios; diferencias visuales aceptables si no rompen flujos.

## ADR-006: Target/plataforma
Status: accepted (2026-11-25)
Context: El original apunta a Wayland/KDE Plasma y usa plugins Labwc/Umbriel.
Decision: Objetivo principal Linux Wayland (KDE Plasma). No priorizar X11/otros desktops en la primera entrega.
Consequences: Integraciones tray/portales validadas primero en Wayland/KDE.

## ADR-007: Cutover y checkpoints
Status: accepted (2026-11-25)
Context: Se necesita validar paridad antes de reemplazar.
Decision: Coexistencia durante desarrollo/pruebas; cutover final cuando el usuario lo confirme. Checkpoints de revisión por bloque grande: 1) config/wlshm/parsing, 2) runner shimejictl/modelo comandos, 3) UI principal, 4) dialogs/settings/tray/autostart, 5) paridad final/cutover.
Consequences: Binario de convivencia durante desarrollo; no se reemplaza el script original hasta aprobación.

## ADR-008: Estrategia de portado adaptada a GUI
Status: accepted (2026-11-25)
Context: `rust-anything` soporta bien lógica pura; una GUI egui no se graba/reproduce igual que CLI/server.
Decision: Extraer/portar lógica pura (config/ie_mode, parsing wlshm/tar/manifest, parseo de salida shimejictl, modelo de comandos) como módulos Rust con tests; reimplementar UI egui encima; verificar con tests de caracterización y checklist/manual de paridad.
Consequences: Loop record/diff/gate aplicado donde haya comportamiento determinista; UI validada por integración/manual.

## ADR-009: Nombre/layout del proyecto
Status: accepted (2026-11-25)
Context: Usuario eligió Q7a:A.
Decision: Repo `~/git/shimeji-gui-rs`, crate Cargo `shimeji-gui-rs`, binario `shimeji-gui-rs` durante coexistencia. Al cutover final, instalar/publicar también como `shimeji-gui-rs` (nombre compatible con el original) según confirmación.
Consequences: Cargo.toml, desktop file y autostart usan `shimeji-gui-rs` en desarrollo; se planifica alias/nombre final para cutover.

## ADR-010: Idioma de UI
Status: accepted (2026-11-25)
Context: Usuario eligió Q10a:A (inglés/español, selector, por defecto español).
Decision: UI bilingüe con selector en ajustes, idioma por defecto español. Textos de paridad con original en español; equivalentes en inglés para toda la UI.
Consequences: Se necesita capa simple de i18n/catálogo de strings; selector persistido en config de la app; pruebas de cobertura de claves.
