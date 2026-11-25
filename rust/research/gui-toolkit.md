# Rust GUI toolkit comparison for replacing PySide6/Qt6 on Wayland/KDE Plasma

**Date:** 2026-11-25  
**Scope:** Primary-source comparison of `gtk4-rs` (+ libadwaita where relevant), `iced`, `egui/eframe`, and `slint` for a ~2000-line Linux utility GUI currently implemented with PySide6/Qt6. The target app calls an external CLI (`shimejictl`) via PTY and presents tables/lists/forms/dialogs.

## Recommendation (short)

**Recommend `gtk4-rs` + `libadwaita`, with `ksni`/`tray-icon` for system-tray integration.** For this specific app shape, GTK4 gives the closest fit to Qt-style desktop widgets, mature native dialogs, strong Wayland/Linux desktop integration, and straightforward mapping for tables/lists/forms/dialogs. The main tradeoff is GTK/libadwaita visual conventions versus QtWidgets fidelity; for a utility control panel, that is usually acceptable.

**Second choice: Slint with the winit backend** if you want a smaller declarative UI layer and can accept more custom work for desktop-native behaviors (tray, some dialog/workflow integration) and less direct Qt-widget parity. **Iced and egui/eframe** are viable for custom or immediate-mode UIs, but they are weaker default fits for a conventional KDE/Wayland utility with tables/forms/dialogs and desktop tray expectations.

## Requirements matrix

| Requirement | gtk4-rs / libadwaita | iced | egui/eframe | slint |
|---|---|---|---|---|
| Wayland/KDE Plasma support | Strong. GTK4 is a native Wayland toolkit; GDK has Wayland backend types. | Good via winit backend, but desktop integration is less "native KDE widget" oriented. | Good via winit/eframe; immediate-mode rendering, not native KDE widgets. | Good via winit backend; also has Qt/LinuxKMS backends. |
| System tray | No built-in GTK4 tray API in scope; use `ksni` (StatusNotifierItem) or `tray-icon`. | No built-in tray in iced core; use `tray-icon`/`ksni` with event-loop integration care. | No built-in tray in eframe core; use `tray-icon`/`ksni` with event-loop integration care. | No obvious built-in Rust tray API in Slint crate; use external tray crate/integration. |
| Native file dialogs | Strong. `GtkFileDialog` is async native dialog API. | No obvious built-in file dialog widget; use `rfd` or platform integration. | Common pattern is `rfd`; eframe ecosystem uses it, not a built-in native dialog widget. | No obvious built-in native file dialog in Slint core; use `rfd`/platform integration. |
| Maturity/maintenance (2026) | Mature GTK ecosystem; gtk-rs book/docs active; GTK4/libadwaita stable desktop stack. | Active, 0.14.0 on docs.rs (2026-10-27). API still evolving. | Active, eframe 0.37.0 on docs.rs (2026-10-15); mature ecosystem but immediate-mode paradigm. | Active, 1.19.1 on docs.rs (2026-11-24); commercial + OSS development. |
| Fit for ~2k-line utility GUI with PTY CLI, tables/lists/forms/dialogs | Best fit. Natural model for forms/dialogs/lists; async/dialog patterns available; desktop conventions align. | Moderate. Good Elm-style architecture, but tables/forms/dialogs/tray require more assembly. | Moderate/low for conventional desktop CRUD. Fast to prototype, weaker for complex tables/forms/native desktop affordances. | Moderate/high if UI can be declarative; strong UI language, but desktop shell features need extra integration. |

## Detailed findings

### 1) gtk4-rs (+ libadwaita)

**What it is.** `gtk4-rs` is the Rust binding for GTK4; libadwaita provides GNOME/libadwaita widgets and patterns. The gtk-rs book is the official Rust GTK4 book. GTK4 is a mature native Linux toolkit with Wayland support.

**Wayland/KDE Plasma.** GTK4 is designed for modern Linux display systems including Wayland. GDK4 exposes Wayland-specific backend types under `gdk4::wayland`, indicating first-class Wayland backend support. On KDE Plasma/Wayland, GTK apps run well, though visual style follows GTK/libadwaita conventions rather than Qt/KDE native widgets.

**System tray.** GTK4 does not provide a modern StatusNotifier/AppIndicator tray abstraction as a primary widget in the same way old X11 tray icons worked. For KDE Plasma Wayland, the practical primary-source path is the freedesktop StatusNotifierItem protocol via `ksni`, or `tray-icon` which supports Linux/BSD via AppIndicator or KSNI and documents platform/event-loop caveats. This is an integration task, not a built-in GTK widget.

**Native file dialogs.** `GtkFileDialog` is the GTK4 async file dialog API. GTK docs describe it as a dialog for opening/saving files, and the Rust binding exposes it in `gtk4::FileDialog`. This is the strongest native-dialog story among the evaluated Rust UI options.

**Maturity/maintenance.** GTK4/libadwaita are mature upstream projects; gtk-rs provides active Rust bindings and book documentation. This is the least risky choice for long-lived Linux desktop utility behavior.

**Suitability for this app.** Very good. Tables/lists/forms/dialogs map naturally to GTK widgets and patterns. A PTY/CLI subprocess can be driven from Rust async/threaded code with UI updates marshalled to the GTK main loop. The main risk is not capability but effort: GTK-rs has boilerplate and ownership/main-loop discipline.

**Primary sources:**
- gtk-rs book: https://gtk-rs.org/gtk4-rs/stable/latest/book/index.html
- gtk4 `FileDialog` (Rust): https://docs.rs/gtk4/latest/gtk4/struct.FileDialog.html
- GTK4 `FileDialog` (C API docs): https://docs.gtk.org/gtk4/class.FileDialog.html
- GDK4 Wayland backend: https://docs.gtk.org/gdk4/wayland.html
- libadwaita Rust docs: https://docs.rs/libadwaita/latest/libadwaita/index.html

### 2) iced

**What it is.** iced is a cross-platform GUI library inspired by Elm. It uses a retained widget/runtime model with an Elm-like update/view architecture. The official site describes it as a cross-platform GUI library for Rust focused on simplicity and type safety.

**Wayland/KDE Plasma.** iced 0.14.0 is current on docs.rs. `iced_winit` provides the winit-based shell; winit supports Wayland. This means Wayland/KDE Plasma runtime is plausible, but the look and integration are not KDE/Qt native. Behavior quality depends on iced/winit backend maturity on the target compositor.

**System tray.** I did not find a built-in system tray API in iced/iced_widget docs. System tray would likely require `tray-icon` or `ksni`, with careful event-loop/thread integration because tray crates often have main-thread/platform requirements.

**Native file dialogs.** I did not find a built-in native file dialog in iced core widgets. The usual primary-source companion is `rfd`, which provides native dialogs with Linux GTK or XDG Desktop Portal backends. This is workable but external.

**Maturity/maintenance.** Active and modern. docs.rs shows iced 0.14.0 released 2026-10-27. The API has evolved substantially across releases, so migration/maintenance should expect some churn.

**Suitability for this app.** Moderate. The Elm architecture is clean for a control-plane GUI that drives `shimejictl`. However, a utility with forms, tables/lists, dialogs, tray, and native file pickers will require assembling several pieces. For a PySide6 replacement where "desktop utility" ergonomics matter, iced is more work than GTK4.

**Primary sources:**
- iced site: https://iced.rs/
- iced docs: https://docs.rs/iced/latest/iced/index.html
- iced_widget docs: https://docs.rs/iced_widget/latest/iced_widget/index.html
- iced_winit docs: https://docs.rs/iced_winit/latest/iced_winit/index.html
- rfd docs: https://docs.rs/rfd/latest/rfd/index.html
- tray-icon docs: https://docs.rs/tray-icon/latest/tray_icon/index.html
- ksni docs: https://docs.rs/ksni/latest/ksni/index.html

### 3) egui/eframe

**What it is.** egui is an immediate-mode GUI library; eframe is its official framework for building native/web apps. The egui README describes egui as an easy-to-use immediate mode GUI and notes integrations including eframe.

**Wayland/KDE Plasma.** eframe runs on winit/glow/wgpu backends depending configuration; Wayland support comes through the underlying windowing stack. It works as a cross-platform app, but it is not a native KDE/Qt/GTK widget toolkit. For a utility app, expect a custom-looking UI rather than desktop-native controls.

**System tray.** No built-in system tray in eframe/egui core. External integration via `tray-icon` or `ksni` is possible, but it needs careful handling because eframe owns the event loop/window lifecycle.

**Native file dialogs.** No built-in native file dialog widget in egui core. The common companion is `rfd`; eframe/ecosystem commonly uses it. This is acceptable, but it is another external dependency and not a Qt-like integrated dialog system.

**Maturity/maintenance.** Active. eframe 0.37.0 is on docs.rs (2026-10-15). egui has wide adoption, especially for tools/editors/debug UIs.

**Suitability for this app.** Low to moderate. egui is excellent for fast internal tools and highly custom interfaces, but a PySide6 utility with tables/lists/forms/dialogs and desktop conventions may feel non-native. Immediate mode can also be awkward for complex form state and data-heavy table workflows unless carefully structured. PTY subprocess management is fine in Rust, but UI integration is not the problem egui solves best.

**Primary sources:**
- egui repository README: https://github.com/emilk/egui
- eframe docs: https://docs.rs/eframe/latest/eframe/index.html
- rfd docs: https://docs.rs/rfd/latest/rfd/index.html
- tray-icon docs: https://docs.rs/tray-icon/latest/tray_icon/index.html

### 4) slint

**What it is.** Slint is a declarative UI toolkit for Rust, C++, JavaScript, and Python. The Rust crate exposes the runtime, and Slint UI is typically authored in `.slint` markup. It targets embedded and desktop use cases.

**Wayland/KDE Plasma.** Slint supports multiple backends/renderers. Official backend docs cover winit and Qt backends, and LinuxKMS. The winit backend is the normal desktop path; Qt backend can provide Qt integration where available. On KDE Plasma Wayland, winit backend should work, but native desktop look-and-feel is not the same as QtWidgets. The Qt backend is interesting for KDE environments, but it is a backend choice with its own deployment/behavior implications.

**System tray.** I found no obvious built-in system tray API in the Slint Rust crate all-items/docs scan during research. Tray support would likely need an external crate such as `ksni`/`tray-icon`, coordinated with the Slint application/event loop.

**Native file dialogs.** I found no obvious built-in native file dialog API in Slint core. External dialogs via `rfd` are the practical route. This is workable but not as integrated as GTK's `FileDialog`.

**Maturity/maintenance.** Active. slint 1.19.1 is on docs.rs (2026-11-24). Slint has regular releases and commercial backing.

**Suitability for this app.** Moderate to good, with caveats. Slint is attractive if you want a clean declarative UI and are willing to model forms/lists/dialogs explicitly. It can handle a ~2k-line utility GUI well. The drawbacks are desktop-shell gaps: tray and native dialogs are external, and complex data-table/form behavior may require more deliberate modeling than GTK. For a PySide6/Qt6 replacement, Slint is conceptually close in "UI as structured components" but not a drop-in widget parity layer.

**Primary sources:**
- Slint site: https://slint.dev/
- Slint Rust docs: https://docs.rs/slint/latest/slint/index.html
- Slint backend selection: https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/
- Slint winit backend: https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/
- Slint Qt backend: https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_qt/
- rfd docs: https://docs.rs/rfd/latest/rfd/index.html
- tray-icon docs: https://docs.rs/tray-icon/latest/tray_icon/index.html
- ksni docs: https://docs.rs/ksni/latest/ksni/index.html

## Cross-cutting notes

### System tray on Wayland/KDE Plasma

Wayland does not provide the old X11 systray mechanism; KDE Plasma commonly exposes tray icons through StatusNotifierItem (SNI). Therefore, toolkit-native tray support is less important than solid SNI integration.

- `ksni` 0.3.7 implements the KDE/freedesktop StatusNotifierItem specification — a direct fit for KDE Plasma.
- `tray-icon` 0.27.0 provides cross-platform tray support and on Linux/BSD uses libappindicator or KSNI; its docs note platform/main-thread/event-loop considerations.

For this app, **`ksni` is the most protocol-direct primary source for KDE Plasma**, while `tray-icon` may be convenient if cross-platform abstraction is desired.

### Native file dialogs

- **GTK4** has `GtkFileDialog` built in.
- **rfd** 0.18.0 provides native file dialogs and can use GTK or XDG Desktop Portal backends on Linux; this is the practical companion for iced/eframe/slint.

For KDE/Wayland portability, portal-backed dialogs are often desirable; `rfd`'s Linux backend options are relevant. GTK4's native dialog is still the simplest integrated path.

### PTY / external CLI

All four options can run `shimejictl` via a Rust PTY/subprocess crate; this is not a GUI-toolkit discriminator. The key is to keep PTY I/O off the UI thread and marshal updates into the toolkit's event loop/model. GTK4 has main-loop integration conventions; iced/egui/slint have their own command/update mechanisms.

## Final assessment

For a **~2000-line PySide6/Qt6 utility GUI** with conventional controls, dialogs, and KDE Plasma Wayland deployment:

1. **gtk4-rs/libadwaita — recommended.** Best desktop-native fit, built-in async file dialogs, mature Linux behavior, clear mapping from QtWidgets-style forms/lists/dialogs. Add `ksni` or `tray-icon` for tray.
2. **slint — viable alternative.** Good if you want a declarative UI and accept external tray/dialog integration and some custom desktop plumbing.
3. **iced — viable but more assembly required.** Clean architecture, but less out-of-the-box desktop utility completeness.
4. **egui/eframe — least conventional fit.** Great for tools, weaker for native-feeling KDE utility forms/tables/dialogs.

## Source index

- https://gtk-rs.org/gtk4-rs/stable/latest/book/index.html
- https://docs.rs/gtk4/latest/gtk4/struct.FileDialog.html
- https://docs.gtk.org/gtk4/class.FileDialog.html
- https://docs.gtk.org/gdk4/wayland.html
- https://docs.rs/libadwaita/latest/libadwaita/index.html
- https://iced.rs/
- https://docs.rs/iced/latest/iced/index.html
- https://docs.rs/iced_widget/latest/iced_widget/index.html
- https://docs.rs/iced_winit/latest/iced_winit/index.html
- https://github.com/emilk/egui
- https://docs.rs/eframe/latest/eframe/index.html
- https://slint.dev/
- https://docs.rs/slint/latest/slint/index.html
- https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/
- https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/
- https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_qt/
- https://docs.rs/rfd/latest/rfd/index.html
- https://docs.rs/tray-icon/latest/tray_icon/index.html
- https://docs.rs/ksni/latest/ksni/index.html
- https://raw.githubusercontent.com/rust-windowing/winit/master/README.md
