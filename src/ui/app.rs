use crate::core::command::CtlCommand;
use crate::core::config::{self, IeMode, Paths, Session};
use crate::core::convert::{self, ConvertOptions, FixOptions};
use crate::core::parsing::{filter_behaviors, parse_bool_value, parse_config_list, parse_environments, parse_section, prototype_names, prototype_store_path, EnvironmentInfo};
use crate::core::shimejictl::{spawn_ctl, CtlEvent, CtlHandle, CtlOutput};
use crate::core::wlshm;
use crate::i18n::{self, Key, Lang};
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use std::collections::{HashMap, HashSet};
use serde_json::Value;

#[derive(Debug, Clone)]
enum Pending { PrototypesList, PrototypeInfo(String), Environments, Convert, Import, Stop, EnvClose, Generic }

#[derive(Debug, Clone, Copy)]
enum DialogTarget { ConvertSrcZip, ConvertSrcDir, ConvertSrcWlshm, ConvertOutDir, ConvertOutFile, ImportFiles, ExportDestDir }

#[derive(Default, Clone)]
struct SettingsState {
    open: bool,
    breed: bool, drag: bool, cursor: bool, affordances: bool, dismiss_anim: bool,
    win_interact: bool, win_throw: bool, throw_policy: String, ie_mode: IeMode, session: Session,
    throw_multi: bool, drag_multi: bool, unified_out: bool,
    scale: f64, opacity: f64, layer: String, interp: i32, maxn: i32, tablets: bool, autostart: bool,
    plugins_path: String,
    confirm_restart: bool,
    raw: String,
}

#[derive(Clone)]
struct ConvertState {
    open: bool, src: String, name: String, out: String, force: bool, import_after: bool, log: String,
    fix_only: bool, no_fix: bool, height: u32, base: String, fixes: FixOptions,
}

impl Default for ConvertState {
    fn default() -> Self {
        Self {
            open: false, src: String::new(), name: String::new(), out: dirs_download(), force: true,
            import_after: true, log: String::new(), fix_only: false, no_fix: false,
            height: 128, base: convert::DEFAULT_BASE.to_string(), fixes: FixOptions::all(),
        }
    }
}

pub struct ShimejiApp {
    pub lang: Lang,
    pub dark_mode: bool,
    pub status: String,
    pub log: String,
    pub search: String,
    pub selected_proto: Option<String>,
    pub prototypes: Vec<String>,
    pub behaviors: Vec<String>,
    pub selected_behavior: String,
    pub environments: Vec<EnvironmentInfo>,
    pub quantity: u32, pub pos_x: i32, pub pos_y: i32, pub env_idx: Option<usize>,
    settings: SettingsState,
    convert: ConvertState,
    pending: Option<Pending>,
    ctl_rx: Option<mpsc::Receiver<CtlEvent>>,
    ctl_handle: Option<CtlHandle>,
    confirm_delete: Option<String>,
    confirm_export: Option<String>,
    confirm_dismiss_all: bool,
    confirm_stop: bool,
    restart_rx: Option<mpsc::Receiver<String>>,
    bg_rx: Option<mpsc::Receiver<String>>,
    preview_rx: Option<mpsc::Receiver<(String, Option<egui::ColorImage>)>>,
    preview_tex: Option<egui::TextureHandle>,
    preview_pending: Option<egui::ColorImage>,
    list_preview_tx: mpsc::Sender<(String, Option<egui::ColorImage>)>,
    list_preview_rx: mpsc::Receiver<(String, Option<egui::ColorImage>)>,
    list_previews: HashMap<String, egui::TextureHandle>,
    list_preview_pending: HashSet<String>,
    list_preview_misses: HashSet<String>,
    dialog_rx: Option<mpsc::Receiver<(DialogTarget, Vec<PathBuf>)>>,
    last_import_paths: Vec<PathBuf>,
    import_retry_done: bool,
    needs_proto_refresh: bool,
    raw_info: String,
    last_output: String,
    initialized: bool,
}

impl Default for ShimejiApp {
    fn default() -> Self {
        let (list_preview_tx, list_preview_rx) = mpsc::channel();
        Self {
            lang: Lang::Es, dark_mode: true, status: String::new(), log: String::new(), search: String::new(),
            selected_proto: None, prototypes: vec![], behaviors: vec![], selected_behavior: String::new(),
            environments: vec![], quantity: 1, pos_x: -1, pos_y: -1, env_idx: None,
            settings: SettingsState { open:false, breed:true, drag:true, cursor:true, affordances:true, dismiss_anim:true, win_interact:false, win_throw:false, throw_policy:"looping".into(), ie_mode:IeMode::Active, session:Session::Labwc, throw_multi:true, drag_multi:true, unified_out:true, scale:-1.0, opacity:-1.0, layer:"overlay".into(), interp:0, maxn:512, tablets:false, autostart:false, plugins_path:String::new(), confirm_restart:false, raw:String::new() },
            convert: ConvertState::default(),
            pending: None, ctl_rx: None, ctl_handle: None, confirm_delete: None, confirm_export: None, confirm_dismiss_all: false, confirm_stop: false, restart_rx: None, bg_rx: None, preview_rx: None, preview_tex: None, preview_pending: None, list_preview_tx, list_preview_rx, list_previews: HashMap::new(), list_preview_pending: HashSet::new(), list_preview_misses: HashSet::new(), dialog_rx: None, last_import_paths: Vec::new(), import_retry_done: false, needs_proto_refresh: false, raw_info: String::new(), last_output: String::new(), initialized: false,
        }
    }
}

fn dirs_download() -> String { std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("Downloads").to_string_lossy().to_string() }

impl ShimejiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self { Self::new_with_settings(cc, false) }

    fn new_with_settings(_cc: &eframe::CreationContext<'_>, open_settings: bool) -> Self {
        let mut app = Self::default();
        if let Ok(paths) = Paths::from_env() {
            if let Some(lang) = config::load_gui_lang(&paths) { app.lang = Lang::from_code(&lang); }
            if let Some(proto) = config::load_gui_selected_proto(&paths) { app.selected_proto = Some(proto); }
        }
        // Read the daemon config before the first frame. Otherwise the settings
        // window briefly shows the defaults (BREEDING=true, etc.) and looks like
        // it ignored the config file.
        app.load_config_sync();
        if open_settings {
            app.settings.open = true;
        }
        app
    }

    fn t(&self, key: Key) -> &'static str { i18n::t(self.lang, key) }
    fn push_log(&mut self, s: &str) { self.log.push_str(s); if !s.ends_with('\n') { self.log.push('\n'); } }

    fn run_cmd(&mut self, cmd: CtlCommand, pending: Pending) {
        self.run_cmd_with_input(cmd, None, pending);
    }
    fn run_cmd_with_input(&mut self, cmd: CtlCommand, input: Option<String>, pending: Pending) {
        if self.ctl_handle.is_some() { self.status = "busy".into(); return; }
        let args = cmd.to_args();
        self.push_log(&format!("$ shimejictl {}", args.join(" ")));
        let (tx, rx) = mpsc::channel();
        let handle = spawn_ctl(args, input, Duration::from_secs(120), Some(tx), None);
        self.ctl_rx = Some(rx); self.ctl_handle = Some(handle); self.pending = Some(pending); self.last_output.clear(); self.status = "running...".into();
    }
    fn cancel(&mut self) { if let Some(h) = &self.ctl_handle { h.cancel(); } }

    /// Lanza el conversor nativo en segundo plano, reutilizando el canal de
    /// eventos y el estado `Pending::Convert` del runner de `shimejictl`.
    fn run_convert_task(&mut self, opts: ConvertOptions) {
        if self.ctl_handle.is_some() { self.status = "busy".into(); return; }
        let (tx, rx) = mpsc::channel();
        let handle = convert::spawn_conversion(opts, tx);
        self.ctl_rx = Some(rx);
        self.ctl_handle = Some(handle);
        self.pending = Some(Pending::Convert);
        self.last_output.clear();
        self.status = "running...".into();
    }

    fn poll_ctl(&mut self, ctx: &egui::Context) {
        let mut finished: Option<CtlOutput> = None;
        let mut outputs: Vec<String> = Vec::new();
        if let Some(rx) = &self.ctl_rx {
            while let Ok(ev) = rx.try_recv() {
                match ev { CtlEvent::Output(s) => { self.last_output.push_str(&s); outputs.push(s); }, CtlEvent::Finished(out) => finished = Some(out) }
            }
        }
        for s in outputs { self.push_log(&s); if self.convert.open { self.convert.log.push_str(&s); } }
        if let Some(out) = finished {
            self.status = format!("exit {}", out.code);
            let pending = self.pending.take();
            let text = self.last_output.clone();
            self.ctl_rx = None; self.ctl_handle = None;
            self.handle_finished(pending, out.code, &text);
            if self.needs_proto_refresh && self.ctl_handle.is_none() {
                self.refresh_prototypes();
            }
        }
        if self.ctl_rx.is_some() { ctx.request_repaint_after(Duration::from_millis(100)); }
    }

    fn poll_restart(&mut self, ctx: &egui::Context) {
        let mut msg = None;
        if let Some(rx) = &self.restart_rx {
            if let Ok(m) = rx.try_recv() { msg = Some(m); }
        }
        if let Some(m) = msg {
            self.restart_rx = None;
            self.push_log(&m);
            self.status = "restarted".into();
            self.refresh_environments_after_restart();
        }
        if self.restart_rx.is_some() { ctx.request_repaint_after(Duration::from_millis(200)); }
    }

    fn poll_bg(&mut self, ctx: &egui::Context) {
        let mut msgs = Vec::new();
        if let Some(rx) = &self.bg_rx {
            while let Ok(m) = rx.try_recv() { msgs.push(m); }
        }
        let mut done = false;
        for m in msgs {
            if m == "__BG_DONE__" { done = true; }
            else { self.push_log(&m); self.status = m.lines().last().unwrap_or("").to_string(); }
        }
        if done { self.bg_rx = None; self.refresh_envs(); self.refresh_prototypes(); }
        if self.bg_rx.is_some() { ctx.request_repaint_after(Duration::from_millis(200)); }
    }

    fn poll_preview(&mut self, ctx: &egui::Context) {
        let mut got = None;
        if let Some(rx) = &self.preview_rx {
            if let Ok(msg) = rx.try_recv() { got = Some(msg); }
        }
        if let Some((name, img)) = got {
            self.preview_rx = None;
            if let Some(ref ci) = img {
                if !self.list_previews.contains_key(&name) {
                    let thumb = Self::downscale_color_image(ci, 96);
                    let tex = ctx.load_texture(format!("list-preview-{name}"), thumb, egui::TextureOptions::LINEAR);
                    self.list_previews.insert(name.clone(), tex);
                    self.list_preview_misses.remove(&name);
                }
            }
            if self.selected_proto.as_deref() == Some(name.as_str()) {
                self.preview_pending = img;
            }
        }
        if self.preview_rx.is_some() { ctx.request_repaint_after(Duration::from_millis(100)); }
    }

    fn downscale_color_image(img: &egui::ColorImage, max_size: u32) -> egui::ColorImage {
        let [w, h] = img.size;
        let m = w.max(h) as u32;
        if max_size == 0 || m <= max_size { return img.clone(); }
        let nw = (w as f32 * max_size as f32 / m as f32).round().max(1.0) as usize;
        let nh = (h as f32 * max_size as f32 / m as f32).round().max(1.0) as usize;
        let mut out = egui::ColorImage::new([nw, nh], egui::Color32::TRANSPARENT);
        for y in 0..nh {
            for x in 0..nw {
                let sx = (x * w / nw).min(w - 1);
                let sy = (y * h / nh).min(h - 1);
                out.pixels[y * nw + x] = img.pixels[sy * w + sx];
            }
        }
        out
    }

    fn ensure_list_previews(&mut self) {
        for name in self.prototypes.clone() {
            if self.list_previews.contains_key(&name) || self.list_preview_pending.contains(&name) || self.list_preview_misses.contains(&name) { continue; }
            self.list_preview_pending.insert(name.clone());
            let tx = self.list_preview_tx.clone();
            std::thread::spawn(move || {
                let img = Self::load_preview_thumb(&name);
                let _ = tx.send((name, img));
            });
        }
    }

    fn poll_list_previews(&mut self, ctx: &egui::Context) {
        let mut any = false;
        while let Ok((name, img)) = self.list_preview_rx.try_recv() {
            self.list_preview_pending.remove(&name);
            if let Some(img) = img {
                let tex = ctx.load_texture(format!("list-preview-{name}"), img, egui::TextureOptions::LINEAR);
                self.list_previews.insert(name, tex);
            } else {
                self.list_preview_misses.insert(name);
            }
            any = true;
        }
        if any { ctx.request_repaint(); }
        if !self.list_preview_pending.is_empty() { ctx.request_repaint_after(Duration::from_millis(120)); }
    }

    fn refresh_environments_after_restart(&mut self) {
        self.refresh_envs();
        self.refresh_prototypes();
    }

    fn restart_daemon(&mut self) {
        if self.restart_rx.is_some() { return; }
        let (tx, rx) = mpsc::channel::<String>();
        self.restart_rx = Some(rx);
        self.status = "restarting...".into();
        std::thread::spawn(move || {
            let _ = crate::core::shimejictl::run_ctl_sync(&["stop".into()], None, Duration::from_secs(12));
            std::thread::sleep(Duration::from_millis(1000));
            let out = crate::core::shimejictl::run_ctl_sync(&["environment".into(), "list".into()], None, Duration::from_secs(12));
            let msg = match out {
                Ok(o) => format!("[restart] exit {}\n{}", o.code, o.output),
                Err(e) => format!("[restart] error: {e}"),
            };
            let _ = tx.send(msg);
        });
    }

    fn handle_finished(&mut self, pending: Option<Pending>, code: i32, text: &str) {
        match pending {
            Some(Pending::PrototypesList) if code == 0 => { self.prototypes = prototype_names(text); self.list_preview_misses.clear(); self.ensure_list_previews(); }
            Some(Pending::PrototypeInfo(name)) if code == 0 => {
                let raw = parse_section(text, "Behaviors");
                let store = prototype_store_path(text);
                let defs = Self::load_behavior_defs(&name, store.as_deref());
                self.behaviors = filter_behaviors(&raw, &defs);
                if !self.behaviors.iter().any(|b| b == &self.selected_behavior) { self.selected_behavior.clear(); }
                self.preview_tex = None;
                self.preview_pending = None;
                let (tx, rx) = mpsc::channel();
                self.preview_rx = Some(rx);
                let name_bg = name.clone();
                let store_bg = store.clone();
                std::thread::spawn(move || {
                    let img = Self::load_preview(&name_bg, store_bg.as_deref());
                    let _ = tx.send((name_bg, img));
                });
                self.raw_info = text.to_string();
                self.selected_proto = Some(name.clone());
                if let Ok(paths) = Paths::from_env() { let _ = config::save_gui_selected_proto(&paths, &name); }
            }
            Some(Pending::Environments) if code == 0 => { self.environments = parse_environments(text); }
            Some(Pending::Convert) => {
                if code == 0 && self.convert.import_after {
                    let out = PathBuf::from(&self.convert.out);
                    let mut files: Vec<PathBuf> = if out.extension().and_then(|s| s.to_str()) == Some("wlshm") {
                        vec![out]
                    } else if let Ok(rd) = std::fs::read_dir(&out) {
                        let mut found: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().and_then(|s| s.to_str()) == Some("wlshm")).collect();
                        found.sort();
                        found
                    } else { Vec::new() };
                    files.retain(|p| p.is_file());
                    if !files.is_empty() {
                        self.import_files(files);
                        return;
                    }
                }
                if code == 0 { self.refresh_prototypes(); }
            }
            Some(Pending::Import) => {
                if code != 0 && !self.import_retry_done {
                    self.import_retry_done = true;
                    let lower = text.to_lowercase();
                    if lower.contains("already exists") || lower.contains("ya existe") || lower.contains("use -f") {
                        self.push_log("Import exists, retrying with -f (overwrite)...");
                        let paths = self.last_import_paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
                        self.run_cmd(CtlCommand::PrototypesImport { paths, force: true }, Pending::Import);
                        return;
                    }
                    self.push_log("Import failed, retrying with auto-rename...");
                    let work = std::env::temp_dir().join(format!("shimeji-gui-rs-import-retry-{}", std::process::id()));
                    let _ = std::fs::create_dir_all(&work);
                    let mut existing = std::collections::BTreeSet::from_iter(self.prototypes.iter().cloned());
                    let mut prepared: Vec<PathBuf> = Vec::new();
                    for (i, src) in self.last_import_paths.clone().iter().enumerate() {
                        if let Some(data) = wlshm::read_wlshm(src) {
                            let base = if data.name.is_empty() { format!("Shimeji_{}", i + 1) } else { data.name.clone() };
                            let mut cand = wlshm::unique_prototype_name(&format!("{}_imp", base), &existing);
                            if existing.contains(&cand) { cand = format!("{}_{}", cand, i + 1); }
                            existing.insert(cand.clone());
                            let display = wlshm::renamed_display_name(data.manifest.get("display_name").and_then(|v| v.as_str()), &cand, &base);
                            let safe: String = cand.chars().map(|c| if c.is_ascii_alphanumeric() || c=='_' || c=='.' || c=='-' { c } else { '_' }).collect();
                            let safe = safe.trim_matches(|c: char| c=='.' || c=='_');
                            let safe = if safe.is_empty() { "shimeji".to_string() } else { safe.to_string() };
                            let dest = work.join(format!("{safe}.wlshm"));
                            match wlshm::write_renamed_wlshm(src, &dest, &cand, Some(&display)) {
                                Ok(()) => { self.push_log(&format!("Reintento import: «{}» -> «{}»", base, cand)); prepared.push(dest); }
                                Err(e) => { self.push_log(&format!("No se pudo renombrar para reintento «{}»: {e}", base)); prepared.push(src.clone()); }
                            }
                        } else {
                            prepared.push(src.clone());
                        }
                    }
                    if !prepared.is_empty() {
                        self.last_import_paths = prepared.clone();
                        let paths = prepared.iter().map(|p| p.to_string_lossy().to_string()).collect();
                        self.run_cmd(CtlCommand::PrototypesImport { paths, force: false }, Pending::Import);
                        return;
                    }
                }
                self.refresh_prototypes();
            }
            Some(Pending::Stop) => { self.refresh_prototypes(); self.refresh_envs(); }
            Some(Pending::EnvClose) => { self.refresh_envs(); }
            Some(Pending::Generic) => { if code == 0 { self.refresh_envs(); } }
            _ => {}
        }
    }

    fn refresh_prototypes(&mut self) {
        if self.ctl_handle.is_some() { self.needs_proto_refresh = true; return; }
        self.needs_proto_refresh = false;
        self.run_cmd(CtlCommand::PrototypesList { detailed:false }, Pending::PrototypesList);
    }
    fn refresh_envs(&mut self) { self.run_cmd(CtlCommand::EnvironmentList, Pending::Environments); }
    /// Load `config list` synchronously so the settings UI is correct on its
    /// very first frame. The async refresh would briefly show the defaults
    /// (e.g. BREEDING=true) and look like the file was not read.
    fn load_config_sync(&mut self) {
        let args = CtlCommand::ConfigList.to_args();
        if let Ok(out) = crate::core::shimejictl::run_ctl_sync(&args, None, Duration::from_secs(12)) {
            if out.code == 0 {
                self.settings.raw = out.output.clone();
                self.apply_config_list(&out.output);
            }
        }
    }

    fn summon_mascot(&mut self, name: String, count: u32, x: Option<i32>, y: Option<i32>, env: Option<i32>, interactive: bool) {
        if interactive || count <= 1 {
            self.run_cmd(CtlCommand::Summon { name, count: Some(1), x, y, env, interactive }, Pending::Generic);
            return;
        }
        if self.bg_rx.is_some() { self.status = "busy".into(); return; }
        let (tx, rx) = mpsc::channel::<String>();
        self.bg_rx = Some(rx);
        self.status = format!("summoning {count}...");
        self.push_log(&format!("Batch summon: {count} x {name}"));
        std::thread::spawn(move || {
            for _ in 0..count {
                let args = CtlCommand::Summon { name: name.clone(), count: Some(1), x, y, env, interactive: false }.to_args();
                let _ = crate::core::shimejictl::run_ctl_sync(&args, None, Duration::from_secs(30));
            }
            let _ = tx.send(format!("Summoned {count} x {name}"));
            let _ = tx.send("__BG_DONE__".to_string());
        });
    }

    fn select_proto(&mut self, name: String) {
        self.selected_proto = Some(name.clone());
        if let Ok(paths) = Paths::from_env() { let _ = config::save_gui_selected_proto(&paths, &name); }
        self.behaviors.clear();
        self.selected_behavior.clear();
        self.preview_tex = None;
        self.preview_pending = None;
        let (tx, rx) = mpsc::channel();
        self.preview_rx = Some(rx);
        let name_bg = name.clone();
        std::thread::spawn(move || {
            let img = Self::load_preview(&name_bg, None);
            let _ = tx.send((name_bg, img));
        });
        self.run_cmd(CtlCommand::PrototypesInfo { name: name.clone() }, Pending::PrototypeInfo(name));
    }
    fn selected_env_id(&self) -> Option<i32> { self.env_idx.and_then(|i| self.environments.get(i)).map(|e| e.id) }

    fn load_behavior_defs(name: &str, store_path: Option<&str>) -> Vec<Value> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(paths) = Paths::from_env() {
            candidates.push(paths.data.join("shimejis").join(format!("Shimeji.{name}")).join("behaviors.json"));
            candidates.push(paths.data.join("shimejis").join(name).join("behaviors.json"));
            candidates.push(paths.data.join("prototypes").join(format!("Shimeji.{name}")).join("behaviors.json"));
            candidates.push(paths.data.join("prototypes").join(name).join("behaviors.json"));
        }
        if let Some(sp) = store_path { candidates.push(PathBuf::from(sp).join("behaviors.json")); }
        for p in candidates {
            if let Ok(text) = std::fs::read_to_string(&p) {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    if let Some(arr) = v.get("definitions").and_then(|d| d.as_array()) { return arr.clone(); }
                }
            }
        }
        Vec::new()
    }

    fn load_preview(name: &str, store_path: Option<&str>) -> Option<egui::ColorImage> {
        Self::load_preview_sized(name, store_path, 0)
    }

    fn load_preview_thumb(name: &str) -> Option<egui::ColorImage> {
        Self::load_preview_sized(name, None, 96)
    }

    fn load_preview_sized(name: &str, store_path: Option<&str>, max_size: u32) -> Option<egui::ColorImage> {
        fn collect_imgs(dir: &std::path::Path, depth: usize, out: &mut Vec<PathBuf>) {
            let Ok(rd) = std::fs::read_dir(dir) else { return; };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() && depth < 3 {
                    collect_imgs(&p, depth + 1, out);
                } else if p.is_file() {
                    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                    if matches!(ext.as_str(), "png"|"jpg"|"jpeg"|"gif"|"webp"|"qoi") {
                        out.push(p);
                    }
                }
            }
        }
        let mut dirs: Vec<PathBuf> = Vec::new();
        if let Some(sp) = store_path { dirs.push(PathBuf::from(sp)); }
        if let Ok(paths) = Paths::from_env() {
            for base in [paths.data.join("shimejis"), paths.data.join("prototypes")] {
                dirs.push(base.join(format!("Shimeji.{name}")));
                dirs.push(base.join(name));
                if let Ok(rd) = std::fs::read_dir(&base) {
                    let name_l = name.to_lowercase();
                    let norm = |s: &str| s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect::<String>();
                    let name_n = norm(&name_l);
                    for e in rd.flatten() {
                        let p = e.path();
                        if !p.is_dir() { continue; }
                        let dn = e.file_name().to_string_lossy().to_lowercase();
                        let dn_n = norm(&dn);
                        let dn_stripped = dn.strip_prefix("shimeji.").unwrap_or(&dn);
                        let dn_stripped_n = norm(dn_stripped);
                        if dn == name_l || dn == format!("shimeji.{name_l}") || dn.ends_with(&format!(".{name_l}"))
                            || dn_n == name_n || dn_n == format!("shimeji{name_n}") || dn_stripped_n == name_n {
                            dirs.push(p);
                        }
                    }
                }
            }
        }
        dirs.sort();
        dirs.dedup();
        for d in dirs {
            if !d.exists() { continue; }
            let mut imgs: Vec<PathBuf> = Vec::new();
            collect_imgs(&d, 0, &mut imgs);
            if imgs.is_empty() { continue; }
            imgs.sort();
            imgs.sort_by_key(|p| {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                // Prefer shimeN images by numeric order (shime0/shime0000/shime1 first)
                let shime_n = if let Some(rest) = stem.strip_prefix("shime") {
                    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if !digits.is_empty() { digits.parse::<u32>().ok() } else { None }
                } else { None };
                if let Some(n) = shime_n {
                    return (0usize, n as usize, 0usize);
                }
                let prefer2 = ["stand", "idle", "icon", "preview", "logo"];
                if let Some(idx) = prefer2.iter().position(|k| stem.contains(k)) {
                    return (1usize, idx, 0usize);
                }
                // fallback: any image containing a number, prefer lower
                let num: String = stem.chars().filter(|c| c.is_ascii_digit()).collect();
                let n = num.parse::<usize>().unwrap_or(usize::MAX);
                (2usize, n, 0usize)
            });
            for p in imgs {
                if let Ok(img) = image::open(&p) {
                    let mut rgba = img.to_rgba8();
                    if max_size > 0 {
                        let (w,h) = rgba.dimensions();
                        let m = w.max(h);
                        if m > max_size {
                            let nw = (w as f32 * max_size as f32 / m as f32).round().max(1.0) as u32;
                            let nh = (h as f32 * max_size as f32 / m as f32).round().max(1.0) as u32;
                            rgba = image::imageops::resize(&rgba, nw, nh, image::imageops::FilterType::Triangle);
                        }
                    }
                    let (w,h) = rgba.dimensions();
                    return Some(egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba.into_raw()));
                }
            }
        }
        None
    }

    fn apply_config_list(&mut self, text: &str) {
        let vals = parse_config_list(text);
        let s = &mut self.settings;
        s.breed = parse_bool_value(&vals, "BREEDING", true);
        s.drag = parse_bool_value(&vals, "DRAGGING", true);
        s.cursor = parse_bool_value(&vals, "CURSOR_POSITION", true);
        s.affordances = parse_bool_value(&vals, "AFFORDANCES", true);
        s.dismiss_anim = parse_bool_value(&vals, "DISMISS_ANIMATIONS", true);
        s.win_interact = parse_bool_value(&vals, "WINDOW_INTERACTIONS", false);
        s.win_throw = parse_bool_value(&vals, "WINDOW_THROWING", false);
        s.throw_multi = parse_bool_value(&vals, "ALLOW_THROWING_MULTIHEAD", true);
        s.drag_multi = parse_bool_value(&vals, "ALLOW_DRAGGING_MULTIHEAD", true);
        s.unified_out = parse_bool_value(&vals, "UNIFIED_OUTPUTS", true);
        s.scale = vals.get("MASCOT_SCALE").and_then(|v| v.parse().ok()).unwrap_or(-1.0);
        s.opacity = vals.get("OPACITY").and_then(|v| v.parse().ok()).unwrap_or(-1.0);
        s.interp = vals.get("INTERPOLATION_FRAMERATE").and_then(|v| v.parse().ok()).unwrap_or(0);
        s.maxn = vals.get("MASCOT_LIMIT").and_then(|v| v.parse().ok()).unwrap_or(512);
        s.tablets = parse_bool_value(&vals, "TABLETS_ENABLED", false);
        s.throw_policy = vals.get("WINDOW_THROW_POLICY").cloned().unwrap_or_else(|| "looping".into()).to_lowercase();
        s.layer = vals.get("WLR_SHELL_LAYER").cloned().unwrap_or_else(|| "overlay".into()).to_lowercase();
        if let Ok(paths) = Paths::from_env() {
            s.session = config::effective_session(&paths);
            s.ie_mode = config::effective_ie_mode(&paths, s.session);
            s.autostart = paths.autostart.exists();
            s.plugins_path = paths.plugin_dir(s.session).to_string_lossy().to_string();
        }
    }

    fn save_settings(&mut self) {
        let mut errors: Vec<String> = Vec::new();
        let map = vec![
            ("BREEDING", self.settings.breed.to_string()), ("DRAGGING", self.settings.drag.to_string()),
            ("CURSOR_POSITION", self.settings.cursor.to_string()), ("AFFORDANCES", self.settings.affordances.to_string()),
            ("DISMISS_ANIMATIONS", self.settings.dismiss_anim.to_string()), ("WINDOW_INTERACTIONS", self.settings.win_interact.to_string()),
            ("WINDOW_THROWING", self.settings.win_throw.to_string()), ("WINDOW_THROW_POLICY", self.settings.throw_policy.clone()),
            ("ALLOW_THROWING_MULTIHEAD", self.settings.throw_multi.to_string()), ("ALLOW_DRAGGING_MULTIHEAD", self.settings.drag_multi.to_string()),
            ("UNIFIED_OUTPUTS", self.settings.unified_out.to_string()), ("MASCOT_SCALE", format!("{:.2}", self.settings.scale)),
            ("OPACITY", format!("{:.2}", self.settings.opacity)), ("WLR_SHELL_LAYER", self.settings.layer.clone()),
            ("INTERPOLATION_FRAMERATE", self.settings.interp.to_string()), ("MASCOT_LIMIT", self.settings.maxn.to_string()),
            ("TABLETS_ENABLED", self.settings.tablets.to_string()),
        ];
        for (k,v) in map {
            let args = vec!["config".to_string(), "set".to_string(), k.to_string(), v.to_string()];
            if let Ok(out) = crate::core::shimejictl::run_ctl_sync(&args, None, Duration::from_secs(12)) {
                if out.code != 0 { errors.push(format!("{k}: {}", out.output.trim())); }
            } else { errors.push(format!("{k}: failed")); }
        }
        if let Ok(paths) = Paths::from_env() {
            if let Err(e) = config::save_plugins_location(&paths, self.settings.session) { errors.push(format!("plugins_location: {e}")); }
            if let Err(e) = config::save_plugin_ie_mode(&paths, self.settings.session, self.settings.ie_mode) { errors.push(format!("ie_mode: {e}")); }
            // autostart
            let res = if self.settings.autostart {
                std::fs::create_dir_all(paths.autostart.parent().unwrap_or(&paths.autostart)).and_then(|_| {
                    std::fs::write(&paths.autostart, "[Desktop Entry]\nType=Application\nName=Shimeji GUI (Rust)\nExec=shimeji-gui-rs\nX-KDE-autostart-after=panel\n")
                })
            } else if paths.autostart.exists() { std::fs::remove_file(&paths.autostart) } else { Ok(()) };
            if let Err(e) = res { errors.push(format!("autostart: {e}")); }
        }
        if errors.is_empty() { self.push_log(self.t(Key::ConfigApplied)); } else { self.push_log(&format!("{}\\n{}", self.t(Key::ConfigApplyErrors), errors.join("\\n"))); }
        self.load_config_sync();
    }

    fn open_settings(&mut self) { self.settings.open = true; self.load_config_sync(); }
    fn open_convert(&mut self) { self.convert.open = true; self.convert.log.clear(); }

    fn run_file_dialog(target: DialogTarget, title: &str) -> Vec<PathBuf> {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        let start = home.to_string_lossy().to_string();
        let has_kdialog = std::process::Command::new("sh").args(["-lc", "command -v kdialog >/dev/null 2>&1"]).status().map(|s| s.success()).unwrap_or(false);
        let has_zenity = std::process::Command::new("sh").args(["-lc", "command -v zenity >/dev/null 2>&1"]).status().map(|s| s.success()).unwrap_or(false);
        let on_kde = std::env::var("KDE_FULL_SESSION").is_ok() || std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase().contains("KDE");

        if on_kde && has_kdialog {
            return Self::run_kdialog(target, title, &start);
        }
        if has_zenity {
            return Self::run_zenity(target, title, &start);
        }
        if has_kdialog {
            return Self::run_kdialog(target, title, &start);
        }
        // fallback: rfd
        match target {
            DialogTarget::ConvertSrcZip => rfd::FileDialog::new().add_filter("zip", &["zip"]).set_title(title).pick_file().into_iter().collect(),
            DialogTarget::ConvertSrcDir => rfd::FileDialog::new().set_title(title).pick_folder().into_iter().collect(),
            DialogTarget::ConvertSrcWlshm | DialogTarget::ConvertOutFile => rfd::FileDialog::new().add_filter("wlshm", &["wlshm"]).set_title(title).pick_file().into_iter().collect(),
            DialogTarget::ConvertOutDir | DialogTarget::ExportDestDir => rfd::FileDialog::new().set_title(title).pick_folder().into_iter().collect(),
            DialogTarget::ImportFiles => rfd::FileDialog::new().add_filter("wlshm", &["wlshm"]).set_title(title).pick_files().unwrap_or_default(),
        }
    }

    fn run_kdialog(target: DialogTarget, title: &str, start: &str) -> Vec<PathBuf> {
        let mut cmd = std::process::Command::new("kdialog");
        match target {
            DialogTarget::ConvertSrcZip => { cmd.args(["--getopenfilename", start, "Zip files (*.zip)|*.zip", "--title", title]); }
            DialogTarget::ConvertSrcWlshm => { cmd.args(["--getopenfilename", start, "WLSHM files (*.wlshm)|*.wlshm", "--title", title]); }
            DialogTarget::ConvertOutFile => { cmd.args(["--getsavefilename", start, "WLSHM files (*.wlshm)|*.wlshm", "--title", title]); }
            DialogTarget::ConvertSrcDir | DialogTarget::ConvertOutDir | DialogTarget::ExportDestDir => { cmd.args(["--getexistingdirectory", start, "--title", title]); }
            DialogTarget::ImportFiles => { cmd.args(["--getopenfilename", start, "WLSHM files (*.wlshm)|*.wlshm", "--title", title, "--multiple", "--separate-output"]); }
        }
        let out = cmd.output();
        match out {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty())
                    .map(PathBuf::from)
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn run_zenity(target: DialogTarget, title: &str, start: &str) -> Vec<PathBuf> {
        let mut cmd = std::process::Command::new("zenity");
        match target {
            DialogTarget::ConvertSrcZip => { cmd.args(["--file-selection", &format!("--title={title}"), &format!("--filename={start}/"), "--file-filter=Zip files (*.zip) | *.zip"]); }
            DialogTarget::ConvertSrcWlshm => { cmd.args(["--file-selection", &format!("--title={title}"), &format!("--filename={start}/"), "--file-filter=WLSHM files (*.wlshm) | *.wlshm"]); }
            DialogTarget::ConvertOutFile => { cmd.args(["--file-selection", "--save", "--confirm-overwrite", &format!("--title={title}"), &format!("--filename={start}"), "--file-filter=WLSHM files (*.wlshm) | *.wlshm"]); }
            DialogTarget::ConvertSrcDir | DialogTarget::ConvertOutDir | DialogTarget::ExportDestDir => { cmd.args(["--file-selection", "--directory", &format!("--title={title}"), &format!("--filename={start}/")]); }
            DialogTarget::ImportFiles => { cmd.args(["--file-selection", "--multiple", "--separator=\n", &format!("--title={title}"), &format!("--filename={start}/"), "--file-filter=WLSHM files (*.wlshm) | *.wlshm"]); }
        }
        let out = cmd.output();
        match out {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty())
                    .map(PathBuf::from)
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn open_dialog(&mut self, target: DialogTarget) {
        if self.dialog_rx.is_some() { self.push_log("Dialog already open"); return; }
        let (tx, rx) = mpsc::channel();
        self.dialog_rx = Some(rx);
        let title = match target {
            DialogTarget::ConvertSrcZip => self.t(Key::ConverterZip).to_string(),
            DialogTarget::ConvertSrcDir => self.t(Key::ConverterDir).to_string(),
            DialogTarget::ConvertSrcWlshm => self.t(Key::ConverterWlshm).to_string(),
            DialogTarget::ConvertOutDir => self.t(Key::ConverterOutput).to_string(),
            DialogTarget::ConvertOutFile => self.t(Key::ConverterOutFile).to_string(),
            DialogTarget::ImportFiles => self.t(Key::BtnAddCharacter).to_string(),
            DialogTarget::ExportDestDir => self.t(Key::BtnExport).to_string(),
        };
        self.push_log(&format!("Opening dialog: {title}"));
        std::thread::spawn(move || {
            let paths = Self::run_file_dialog(target, &title);
            let _ = tx.send((target, paths));
        });
    }

    fn poll_dialogs(&mut self, ctx: &egui::Context) {
        let mut result = None;
        if let Some(rx) = &self.dialog_rx {
            match rx.try_recv() {
                Ok(r) => result = Some(r),
                Err(mpsc::TryRecvError::Empty) => { ctx.request_repaint_after(Duration::from_millis(100)); return; }
                Err(mpsc::TryRecvError::Disconnected) => { self.dialog_rx = None; return; }
            }
        }
        if let Some((target, paths)) = result {
            self.dialog_rx = None;
            if paths.is_empty() { self.push_log("Dialog: cancelled or no selection"); return; }
            match target {
                DialogTarget::ConvertSrcZip | DialogTarget::ConvertSrcDir | DialogTarget::ConvertSrcWlshm => {
                    if let Some(p) = paths.first() { self.convert.src = p.to_string_lossy().to_string(); }
                }
                DialogTarget::ConvertOutDir | DialogTarget::ConvertOutFile => {
                    if let Some(p) = paths.first() { self.convert.out = p.to_string_lossy().to_string(); }
                }
                DialogTarget::ImportFiles => {
                    self.import_files(paths);
                }
                DialogTarget::ExportDestDir => {
                    if let Some(dest) = paths.first().cloned() {
                        if let Some(name) = self.selected_proto.clone() {
                            self.export_prototype_zip(name, dest);
                        }
                    }
                }
            }
        }
    }

    fn import_files(&mut self, files: Vec<PathBuf>) {
        let work = std::env::temp_dir().join(format!("shimeji-gui-rs-import-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&work);
        let existing = std::collections::BTreeSet::from_iter(self.prototypes.iter().cloned());
        let (prepared, notices) = wlshm::prepare_import_files(&files, &work, existing, |p| wlshm::read_wlshm(p));
        for n in notices { self.push_log(&n); }
        if prepared.is_empty() { return; }
        self.import_retry_done = false;
        self.last_import_paths = prepared.clone();
        let paths = prepared.iter().map(|p| p.to_string_lossy().to_string()).collect();
        self.run_cmd(CtlCommand::PrototypesImport { paths, force: false }, Pending::Import);
    }

    fn do_import(&mut self) {
        self.open_dialog(DialogTarget::ImportFiles);
    }
    fn do_export(&mut self) {
        let Some(name) = self.selected_proto.clone() else { return; };
        self.confirm_export = Some(name);
    }

    fn export_prototype_zip(&mut self, name: String, dest_dir: PathBuf) {
        if self.bg_rx.is_some() { self.status = "busy".into(); return; }
        let (tx, rx) = mpsc::channel::<String>();
        self.bg_rx = Some(rx);
        self.status = format!("Exporting {name}...");
        self.push_log(&format!("Exporting {name} to {}", dest_dir.display()));
        std::thread::spawn(move || {
            let info = crate::core::shimejictl::run_ctl_sync(&["prototypes".into(), "info".into(), name.clone()], None, std::time::Duration::from_secs(20));
            let rel = match &info {
                Ok(o) if o.code == 0 => crate::core::parsing::prototype_store_path(&o.output),
                _ => None,
            };
            let Some(rel) = rel else {
                let _ = tx.send("No se pudo determinar la ruta de almacenamiento del prototipo.".to_string());
                let _ = tx.send("__BG_DONE__".to_string());
                return;
            };
            let Ok(paths) = crate::core::config::Paths::from_env() else {
                let _ = tx.send("No se pudieron resolver rutas XDG.".to_string());
                let _ = tx.send("__BG_DONE__".to_string());
                return;
            };
            let mut target: Option<std::path::PathBuf> = None;
            for base in [paths.data.join("shimejis"), paths.data.join("prototypes"), paths.data.clone()] {
                let cand = base.join(&rel);
                if cand.exists() { target = Some(cand); break; }
            }
            let Some(target) = target else {
                let _ = tx.send(format!("No se encontró el directorio: {rel}"));
                let _ = tx.send("__BG_DONE__".to_string());
                return;
            };
            let data_resolved = paths.data.canonicalize().unwrap_or(paths.data.clone());
            let target_resolved = target.canonicalize().unwrap_or(target.clone());
            if !target_resolved.starts_with(&data_resolved) {
                let _ = tx.send("Seguridad: ruta fuera de DATA, cancelado.".to_string());
                let _ = tx.send("__BG_DONE__".to_string());
                return;
            }
            let zip_name = format!("{}.zip", target_resolved.file_name().and_then(|s| s.to_str()).unwrap_or(&name));
            let out_zip = dest_dir.join(zip_name);
            // create zip via python3 for portability
            let script = "import os,sys,zipfile
src=sys.argv[1]
out=sys.argv[2]
with zipfile.ZipFile(out,'w',zipfile.ZIP_DEFLATED) as z:
  for root,dirs,files in os.walk(src):
    for f in files:
      p=os.path.join(root,f)
      arc=os.path.relpath(p,os.path.dirname(src))
      z.write(p,arc)
print(out)
";
            let res = std::process::Command::new("python3").args(["-c", script, &target_resolved.to_string_lossy(), &out_zip.to_string_lossy()]).output();
            match res {
                Ok(o) if o.status.success() => { let _ = tx.send(format!("Exportado: {}", out_zip.display())); }
                Ok(o) => { let _ = tx.send(format!("Error exportando: {}", String::from_utf8_lossy(&o.stderr))); }
                Err(e) => { let _ = tx.send(format!("Error ejecutando python3: {e}")); }
            }
            let _ = tx.send("__BG_DONE__".to_string());
        });
    }
    fn do_delete(&mut self) {
        let Some(name) = self.selected_proto.clone() else { return; };
        self.confirm_delete = Some(name);
    }
}

impl eframe::App for ShimejiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(if self.dark_mode { egui::Visuals::dark() } else { egui::Visuals::light() });
        self.poll_ctl(ctx);
        self.poll_restart(ctx);
        self.poll_bg(ctx);
        self.poll_preview(ctx);
        self.poll_list_previews(ctx);
        self.poll_dialogs(ctx);
        if !self.initialized {
            self.initialized = true;
            if let Ok(paths) = crate::core::config::Paths::from_env() {
                if let Some(dark) = crate::core::config::load_gui_dark_mode(&paths) {
                    self.dark_mode = dark;
                    ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
                }
            }
            self.refresh_prototypes();
            self.refresh_envs();
        }
        if let Some(img) = self.preview_pending.take() {
            self.preview_tex = Some(ctx.load_texture("proto-preview", img, egui::TextureOptions::LINEAR));
        }
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(self.t(Key::AppTitle));
                ui.separator();
                ui.label(self.t(Key::LanguageLabel));
                let prev_lang = self.lang;
                egui::ComboBox::from_id_salt("lang").selected_text(self.lang.label()).show_ui(ui, |ui| { for l in Lang::all() { ui.selectable_value(&mut self.lang, l, l.label()); } });
                if self.lang != prev_lang {
                    if let Ok(paths) = Paths::from_env() { let _ = config::save_gui_lang(&paths, self.lang.code()); }
                }
                ui.label(self.t(Key::ThemeLabel));
                let theme_label = if self.dark_mode { self.t(Key::ThemeDark) } else { self.t(Key::ThemeLight) };
                egui::ComboBox::from_id_salt("theme").selected_text(theme_label).show_ui(ui, |ui| {
                    if ui.selectable_label(self.dark_mode, self.t(Key::ThemeDark)).clicked() {
                        self.dark_mode = true;
                        if let Ok(paths) = crate::core::config::Paths::from_env() { let _ = crate::core::config::save_gui_dark_mode(&paths, self.dark_mode); }
                    }
                    if ui.selectable_label(!self.dark_mode, self.t(Key::ThemeLight)).clicked() {
                        self.dark_mode = false;
                        if let Ok(paths) = crate::core::config::Paths::from_env() { let _ = crate::core::config::save_gui_dark_mode(&paths, self.dark_mode); }
                    }
                });
                if ui.button(self.t(Key::TabSettings)).clicked() { self.open_settings(); }
                if ui.button(self.t(Key::ConverterTitle)).clicked() { self.open_convert(); }
                if ui.button(self.t(Key::BtnRestartService)).clicked() { self.restart_daemon(); }
                if ui.button(self.t(Key::BtnStopService)).clicked() { self.confirm_stop = true; }
                if self.ctl_handle.is_some() && ui.button(self.t(Key::BtnCancel)).clicked() { self.cancel(); }
                ui.separator(); ui.label(&self.status);
            });
        });

        egui::SidePanel::left("left").default_width(280.0).min_width(220.0).max_width(420.0).show(ctx, |ui| {
            let hint = self.t(Key::SearchPlaceholder);
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.add(egui::TextEdit::singleline(&mut self.search).hint_text(hint).desired_width(f32::INFINITY));
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button(self.t(Key::BtnRefresh)).clicked() { self.refresh_prototypes(); }
                if ui.button(self.t(Key::BtnReloadPrototypes)).clicked() { self.run_cmd(CtlCommand::PrototypesReloadAll, Pending::Generic); }
                if ui.button(self.t(Key::BtnAddCharacter)).clicked() { self.do_import(); }
                if ui.button(self.t(Key::BtnExport)).clicked() { self.do_export(); }
                if ui.button(self.t(Key::BtnDeleteCharacter)).clicked() { self.do_delete(); }
            });
            ui.separator();
            let filter = self.search.to_lowercase();
            let protos = self.prototypes.clone();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let mut clicked: Option<String> = None;
                for p in &protos {
                    if !filter.is_empty() && !p.to_lowercase().contains(&filter) { continue; }
                    let selected = self.selected_proto.as_deref() == Some(p.as_str());
                    let row_h = 72.0;
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::click());
                    let bg = if selected { ui.visuals().selection.bg_fill }
                             else if resp.hovered() { ui.visuals().widgets.hovered.weak_bg_fill }
                             else { egui::Color32::TRANSPARENT };
                    if bg != egui::Color32::TRANSPARENT {
                        ui.painter().rect_filled(rect, 4.0, bg);
                    }
                    let img_rect = egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 4.0), egui::vec2(56.0, 56.0));
                    if let Some(tex) = self.list_previews.get(p) {
                        let size = tex.size_vec2();
                        let scale = (56.0 / size.x.max(size.y)).min(1.0);
                        let draw_size = size * scale;
                        let centered = egui::Rect::from_center_size(img_rect.center(), draw_size);
                        ui.painter().image(tex.id(), centered, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                    } else {
                        ui.painter().rect_stroke(img_rect, 4.0, egui::Stroke::new(1.0f32, ui.visuals().weak_text_color()), egui::StrokeKind::Inside);
                    }
                    let text_pos = egui::pos2(img_rect.max.x + 8.0, rect.center().y);
                    let color = if selected { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
                    ui.painter().text(text_pos, egui::Align2::LEFT_CENTER, p, egui::TextStyle::Body.resolve(ui.style()), color);
                    if resp.clicked() { clicked = Some(p.clone()); }
                }
                if let Some(p) = clicked { self.select_proto(p); }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(tex) = &self.preview_tex {
                    let size = tex.size_vec2();
                    let max = 192.0_f32;
                    let scale = (max / size.x.max(size.y)).min(1.0);
                    ui.image((tex.id(), size * scale));
                }
                ui.vertical(|ui| {
                    ui.heading(self.selected_proto.clone().unwrap_or_else(|| self.t(Key::NoCharacterSelected).to_string()));
                    if self.selected_proto.is_some() && self.preview_tex.is_none() {
                        ui.weak(self.t(Key::PreviewUnavailable));
                    }
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(self.t(Key::InitialBehavior));
                egui::ComboBox::from_id_salt("behavior").selected_text(if self.selected_behavior.is_empty() { "-" } else { &self.selected_behavior }).show_ui(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        for b in self.behaviors.clone() { ui.selectable_value(&mut self.selected_behavior, b.clone(), b); }
                    });
                });
            });
            ui.group(|ui| {
                ui.label(self.t(Key::SummonGroup));
                ui.horizontal(|ui| {
                    ui.label(self.t(Key::Quantity)); ui.add(egui::DragValue::new(&mut self.quantity).range(1..=50));
                    ui.label("X"); ui.add(egui::DragValue::new(&mut self.pos_x).range(-1..=9999));
                    ui.label("Y"); ui.add(egui::DragValue::new(&mut self.pos_y).range(-1..=9999));
                });
                ui.horizontal(|ui| {
                    ui.label(self.t(Key::TargetScreen));
                    let selected_label = self.env_idx.and_then(|i| self.environments.get(i)).map(|e| format!("{}: {}", e.id, e.name)).unwrap_or_else(|| self.t(Key::RandomAllEnvironments).to_string());
                    egui::ComboBox::from_id_salt("summon_env").selected_text(selected_label).show_ui(ui, |ui| {
                        if ui.selectable_label(self.env_idx.is_none(), self.t(Key::RandomAllEnvironments)).clicked() { self.env_idx = None; }
                        for (i, e) in self.environments.clone().iter().enumerate() {
                            let label = format!("{}: {}", e.id, e.name);
                            if ui.selectable_label(self.env_idx == Some(i), label).clicked() { self.env_idx = Some(i); }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    let name = self.selected_proto.clone().unwrap_or_default();
                    let env = self.selected_env_id();
                    if ui.button(self.t(Key::BtnSummonNow)).clicked() && !name.is_empty() {
                        self.summon_mascot(name.clone(), self.quantity, Some(self.pos_x), Some(self.pos_y), env, false);
                    }
                    if ui.button(self.t(Key::BtnPickPosition)).clicked() && !name.is_empty() {
                        self.summon_mascot(name, self.quantity, None, None, self.selected_env_id(), true);
                    }
                });
            });
            ui.group(|ui| {
                ui.label(self.t(Key::DismissGroup));
                ui.horizontal_wrapped(|ui| {
                    if ui.button(self.t(Key::BtnDismissOne)).clicked() { self.run_cmd(CtlCommand::Dismiss { id: None, all: false, interactive: true, same_type: false, others: false, env: self.selected_env_id() }, Pending::Generic); }
                    if ui.button(self.t(Key::BtnDismissType)).clicked() { self.run_cmd(CtlCommand::Dismiss { id: None, all: false, interactive: true, same_type: true, others: false, env: self.selected_env_id() }, Pending::Generic); }
                    if ui.button(self.t(Key::BtnDismissOthers)).clicked() { self.run_cmd(CtlCommand::Dismiss { id: None, all: false, interactive: true, same_type: false, others: true, env: self.selected_env_id() }, Pending::Generic); }
                    if ui.button(self.t(Key::BtnDismissAll)).clicked() { self.confirm_dismiss_all = true; }
                    if ui.button(self.t(Key::BtnDismissScreen)).clicked() {
                        if let Some(env) = self.selected_env_id() {
                            self.run_cmd(CtlCommand::Dismiss { id: None, all: true, interactive: false, same_type: false, others: false, env: Some(env) }, Pending::Generic);
                        }
                    }
                });
            });
            ui.group(|ui| {
                ui.label(self.t(Key::BehaviorRescueGroup));
                ui.horizontal_wrapped(|ui| {
                    if ui.button(self.t(Key::BtnChangeBehavior)).clicked() && !self.selected_behavior.is_empty() { self.run_cmd(CtlCommand::SetBehavior { behavior: self.selected_behavior.clone(), interactive: true }, Pending::Generic); }
                    if ui.button(self.t(Key::BtnRestoreWindows)).clicked() { self.run_cmd(CtlCommand::RestoreWindows { all: true }, Pending::Generic); }
                });
            });
            ui.separator();
            ui.collapsing(self.t(Key::TabEnvironments), |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button(self.t(Key::BtnRefresh)).clicked() { self.refresh_envs(); }
                    if ui.button(self.t(Key::BtnCloseEnvLayer)).clicked() {
                        if let Some(env) = self.selected_env_id() {
                            self.run_cmd(CtlCommand::EnvironmentClose { id: Some(env), interactive: false }, Pending::EnvClose);
                        }
                    }
                    if ui.button(self.t(Key::BtnCloseEnvSelect)).clicked() {
                        self.run_cmd(CtlCommand::EnvironmentClose { id: None, interactive: true }, Pending::EnvClose);
                    }
                });
                egui::Grid::new("envs").striped(true).show(ui, |ui| {
                    ui.strong(self.t(Key::EnvIdLabel)); ui.strong(self.t(Key::EnvNameLabel)); ui.strong(self.t(Key::EnvSizeLabel)); ui.strong(self.t(Key::EnvMascotsLabel)); ui.end_row();
                    for (i, e) in self.environments.clone().iter().enumerate() {
                        let sel = self.env_idx == Some(i);
                        if ui.selectable_label(sel, e.id.to_string()).clicked() { self.env_idx = Some(i); }
                        ui.label(&e.name); ui.label(&e.size); ui.label(e.mascots.to_string()); ui.end_row();
                    }
                });
            });
            ui.separator();
            ui.collapsing(self.t(Key::InspectorTitle), |ui| {
                egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.raw_info.as_str()).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                });
            });
            ui.separator();
            ui.label(self.t(Key::TabLogs));
            egui::ScrollArea::vertical().stick_to_bottom(true).max_height(180.0).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut self.log.as_str()).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
            });
        });

        self.settings_window(ctx);
        self.convert_window(ctx);
        self.confirm_delete_window(ctx);
        self.confirm_export_window(ctx);
        self.confirm_dismiss_all_window(ctx);
        self.confirm_stop_window(ctx);
        self.confirm_restart_window(ctx);
    }
}

impl ShimejiApp {

    fn delete_prototype(&mut self, name: String) {
        if self.bg_rx.is_some() { self.status = "busy".into(); return; }
        let (tx, rx) = mpsc::channel::<String>();
        self.bg_rx = Some(rx);
        self.status = format!("Deleting {name}...");
        self.push_log(&format!("Deleting prototype: {name}"));
        std::thread::spawn(move || {
            // Try native command first
            let help = crate::core::shimejictl::run_ctl_sync(&["prototypes".into(), "--help".into()], None, std::time::Duration::from_secs(12));
            let mut native_verb: Option<&str> = None;
            if let Ok(out) = &help {
                let t = &out.output;
                for verb in ["remove", "delete"] {
                    if t.contains(&format!(" {verb}")) || t.contains(&format!("\n    {verb}")) || t.contains(&format!("{{{verb}")) {
                        native_verb = Some(verb);
                        break;
                    }
                }
            }
            if let Some(verb) = native_verb {
                let args = vec!["prototypes".into(), verb.into(), name.clone()];
                let out = crate::core::shimejictl::run_ctl_sync(&args, None, std::time::Duration::from_secs(30));
                match out {
                    Ok(o) => { let _ = tx.send(o.output); if o.code == 0 { let _ = tx.send("__BG_DONE__".into()); } else { let _ = tx.send(format!("Delete failed (exit {})", o.code)); let _ = tx.send("__BG_DONE__".into()); } }
                    Err(e) => { let _ = tx.send(format!("Delete error: {e}")); let _ = tx.send("__BG_DONE__".into()); }
                }
                return;
            }

            // Fallback: delete storage dir
            let info = crate::core::shimejictl::run_ctl_sync(&["prototypes".into(), "info".into(), name.clone()], None, std::time::Duration::from_secs(20));
            let rel = match &info {
                Ok(o) if o.code == 0 => crate::core::parsing::prototype_store_path(&o.output),
                _ => None,
            };
            let Some(rel) = rel else {
                let _ = tx.send("No se pudo determinar la ruta de almacenamiento del prototipo.".to_string());
                let _ = tx.send("__BG_DONE__".into());
                return;
            };
            let Ok(paths) = crate::core::config::Paths::from_env() else {
                let _ = tx.send("No se pudieron resolver rutas XDG.".to_string());
                let _ = tx.send("__BG_DONE__".into());
                return;
            };
            let mut target: Option<std::path::PathBuf> = None;
            for base in [paths.data.join("shimejis"), paths.data.join("prototypes"), paths.data.clone()] {
                let cand = base.join(&rel);
                if cand.exists() { target = Some(cand); break; }
            }
            let Some(target) = target else {
                let _ = tx.send(format!("No se encontró el directorio: {rel}"));
                let _ = tx.send("__BG_DONE__".into());
                return;
            };
            let data_resolved = paths.data.canonicalize().unwrap_or(paths.data.clone());
            let target_resolved = target.canonicalize().unwrap_or(target.clone());
            if !target_resolved.starts_with(&data_resolved) {
                let _ = tx.send("Seguridad: ruta fuera de DATA, cancelado.".to_string());
                let _ = tx.send("__BG_DONE__".into());
                return;
            }
            let res = if target_resolved.is_dir() { std::fs::remove_dir_all(&target_resolved) } else { std::fs::remove_file(&target_resolved) };
            match res {
                Ok(()) => { let _ = tx.send(format!("Eliminado: {}", target_resolved.display())); }
                Err(e) => { let _ = tx.send(format!("Error eliminando {}: {e}", target_resolved.display())); }
            }
            // Ask daemon to reload
            let _ = crate::core::shimejictl::run_ctl_sync(&["prototypes".into(), "reload-all".into()], None, std::time::Duration::from_secs(20));
            let _ = tx.send("__BG_DONE__".to_string());
        });
    }
    fn confirm_delete_window(&mut self, ctx: &egui::Context) {
        let Some(name) = self.confirm_delete.clone() else { return; };
        let mut open = true;
        let mut do_it = false;
        egui::Window::new(self.t(Key::ConfirmDeleteTitle)).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            ui.label(format!("{}: {}", self.t(Key::ConfirmDeleteBody), name));
            ui.horizontal(|ui| {
                if ui.button(self.t(Key::BtnDeleteCharacter)).clicked() { do_it = true; }
                if ui.button(self.t(Key::BtnCancel)).clicked() { self.confirm_delete = None; }
            });
        });
        if do_it {
            self.confirm_delete = None;
            self.delete_prototype(name.clone());
        } else if !open {
            self.confirm_delete = None;
        }
    }


    fn confirm_export_window(&mut self, ctx: &egui::Context) {
        let Some(name) = self.confirm_export.clone() else { return; };
        let mut open = true;
        let mut do_it = false;
        egui::Window::new(self.t(Key::ConfirmExportTitle)).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            let body = self.t(Key::ConfirmExportBody).replace("{name}", &name);
            ui.label(body);
            ui.horizontal(|ui| {
                if ui.button(self.t(Key::BtnExport)).clicked() { do_it = true; }
                if ui.button(self.t(Key::BtnCancel)).clicked() { self.confirm_export = None; }
            });
        });
        if do_it {
            self.confirm_export = None;
            self.open_dialog(DialogTarget::ExportDestDir);
        } else if !open {
            self.confirm_export = None;
        }
    }
    fn confirm_dismiss_all_window(&mut self, ctx: &egui::Context) {
        if !self.confirm_dismiss_all { return; }
        let mut open = true;
        let mut do_it = false;
        egui::Window::new(self.t(Key::ConfirmDismissAllTitle)).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            ui.label(self.t(Key::ConfirmDismissAllBody));
            ui.horizontal(|ui| {
                if ui.button(self.t(Key::BtnDismissAll)).clicked() { do_it = true; }
                if ui.button(self.t(Key::BtnCancel)).clicked() { self.confirm_dismiss_all = false; }
            });
        });
        if do_it {
            self.confirm_dismiss_all = false;
            self.run_cmd(CtlCommand::Dismiss { id: None, all: true, interactive: false, same_type: false, others: false, env: self.selected_env_id() }, Pending::Generic);
        } else if !open { self.confirm_dismiss_all = false; }
    }

    fn confirm_stop_window(&mut self, ctx: &egui::Context) {
        if !self.confirm_stop { return; }
        let mut open = true;
        let mut do_it = false;
        egui::Window::new(self.t(Key::ConfirmStopTitle)).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            ui.label(self.t(Key::ConfirmStopBody));
            ui.horizontal(|ui| {
                if ui.button(self.t(Key::BtnStopService)).clicked() { do_it = true; }
                if ui.button(self.t(Key::BtnCancel)).clicked() { self.confirm_stop = false; }
            });
        });
        if do_it {
            self.confirm_stop = false;
            self.run_cmd(CtlCommand::Stop, Pending::Stop);
        } else if !open { self.confirm_stop = false; }
    }

    fn confirm_restart_window(&mut self, ctx: &egui::Context) {
        if !self.settings.confirm_restart { return; }
        let mut open = true;
        let mut do_restart = false;
        egui::Window::new(self.t(Key::BtnRestartService)).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            ui.label(self.t(Key::SettingsSavedRestartMsg));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(self.t(Key::RestartServiceNow)).clicked() { do_restart = true; self.settings.confirm_restart = false; }
                if ui.button(self.t(Key::BtnClose)).clicked() { self.settings.confirm_restart = false; }
            });
        });
        if do_restart { self.restart_daemon(); }
        if !open { self.settings.confirm_restart = false; }
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.settings.open;
        egui::Window::new(self.t(Key::SettingsTitle)).open(&mut open).default_size([620.0, 520.0]).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.checkbox(&mut self.settings.breed, "BREEDING");
                ui.checkbox(&mut self.settings.drag, "DRAGGING");
                ui.checkbox(&mut self.settings.cursor, "CURSOR_POSITION");
                ui.checkbox(&mut self.settings.affordances, "AFFORDANCES");
                ui.checkbox(&mut self.settings.dismiss_anim, "DISMISS_ANIMATIONS");
                ui.checkbox(&mut self.settings.win_interact, "WINDOW_INTERACTIONS");
                ui.checkbox(&mut self.settings.win_throw, "WINDOW_THROWING");
                ui.horizontal(|ui| { ui.label(self.t(Key::ThrowPolicyLabel)); egui::ComboBox::from_id_salt("throwp").selected_text(&self.settings.throw_policy).show_ui(ui, |ui| { for p in ["looping","clamp","bounce","ignore"] { ui.selectable_value(&mut self.settings.throw_policy, p.to_string(), p); } }); });
                ui.horizontal(|ui| { ui.label(self.t(Key::SessionSettingLabel)); egui::ComboBox::from_id_salt("sess").selected_text(self.settings.session.label()).show_ui(ui, |ui| { for s in Session::all() { ui.selectable_value(&mut self.settings.session, s, s.label()); } }); });
                let plugin_dir = Paths::from_env().map(|p| p.plugin_dir(self.settings.session).to_string_lossy().to_string()).unwrap_or_else(|_| self.settings.plugins_path.clone());
                ui.horizontal(|ui| { ui.label(self.t(Key::PluginsDirLabel)); ui.weak(plugin_dir); });
                ui.horizontal(|ui| { ui.label(self.t(Key::IeModeLabel)); egui::ComboBox::from_id_salt("iem").selected_text(self.settings.ie_mode.as_str()).show_ui(ui, |ui| { for m in [IeMode::Active,IeMode::Focused,IeMode::Largest,IeMode::None] { ui.selectable_value(&mut self.settings.ie_mode, m, m.as_str()); } }); });
                ui.checkbox(&mut self.settings.throw_multi, "ALLOW_THROWING_MULTIHEAD");
                ui.checkbox(&mut self.settings.drag_multi, "ALLOW_DRAGGING_MULTIHEAD");
                ui.checkbox(&mut self.settings.unified_out, "UNIFIED_OUTPUTS");
                ui.horizontal(|ui| { ui.label(self.t(Key::ScaleLabel)); ui.add(egui::DragValue::new(&mut self.settings.scale).range(-1.0..=5.0).speed(0.05)); });
                ui.horizontal(|ui| { ui.label(self.t(Key::OpacityLabel)); ui.add(egui::DragValue::new(&mut self.settings.opacity).range(-1.0..=1.0).speed(0.05)); });
                ui.horizontal(|ui| { ui.label(self.t(Key::LayerLabel)); egui::ComboBox::from_id_salt("layer").selected_text(&self.settings.layer).show_ui(ui, |ui| { for p in ["overlay","top"] { ui.selectable_value(&mut self.settings.layer, p.to_string(), p); } }); });
                ui.horizontal(|ui| { ui.label(self.t(Key::InterpLabel)); ui.add(egui::DragValue::new(&mut self.settings.interp).range(-1..=1000)); });
                ui.horizontal(|ui| { ui.label(self.t(Key::MaxLabel)); ui.add(egui::DragValue::new(&mut self.settings.maxn).range(1..=2048)); });
                let l = self.t(Key::TabletsLabel); ui.checkbox(&mut self.settings.tablets, l);
                let l = self.t(Key::AutostartLabel); ui.checkbox(&mut self.settings.autostart, l);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(self.t(Key::BtnReread)).clicked() { self.load_config_sync(); }
                    if ui.button(self.t(Key::BtnApplySave)).clicked() { self.save_settings(); }
                });
                ui.collapsing(self.t(Key::RawConfig), |ui| { egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| { ui.add(egui::TextEdit::multiline(&mut self.settings.raw.as_str()).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace)); }); });
            });
        });
        self.settings.open = open;
    }

    fn convert_window(&mut self, ctx: &egui::Context) {
        let mut open = self.convert.open;
        egui::Window::new(self.t(Key::ConverterTitle)).open(&mut open).default_size([640.0, 560.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.t(Key::ConverterSource));
                ui.text_edit_singleline(&mut self.convert.src);
                if self.convert.fix_only {
                    if ui.button(self.t(Key::ConverterWlshm)).clicked() { self.open_dialog(DialogTarget::ConvertSrcWlshm); }
                } else {
                    if ui.button(self.t(Key::ConverterZip)).clicked() { self.open_dialog(DialogTarget::ConvertSrcZip); }
                    if ui.button(self.t(Key::ConverterDir)).clicked() { self.open_dialog(DialogTarget::ConvertSrcDir); }
                }
            });
            let l_name = self.t(Key::ConverterName);
            ui.horizontal(|ui| {
                ui.label(l_name);
                ui.text_edit_singleline(&mut self.convert.name);
            });
            ui.horizontal(|ui| {
                ui.label(self.t(Key::ConverterOutput));
                ui.text_edit_singleline(&mut self.convert.out);
                if self.convert.fix_only {
                    if ui.button(self.t(Key::ConverterOutFile)).clicked() { self.open_dialog(DialogTarget::ConvertOutFile); }
                } else if ui.button("...").clicked() {
                    self.open_dialog(DialogTarget::ConvertOutDir);
                }
            });
            let l_fix_only = self.t(Key::ConverterFixOnly);
            let l_force = self.t(Key::ConverterForce);
            let l_import = self.t(Key::ConverterImportAfter);
            let l_no_fix = self.t(Key::ConverterNoFix);
            let l_fixes = self.t(Key::ConverterFixes);
            let l_refs = self.t(Key::ConverterFixRefs);
            let l_scale = self.t(Key::ConverterFixScale);
            let l_anchor = self.t(Key::ConverterFixAnchor);
            let l_fall = self.t(Key::ConverterFixFall);
            let l_embedded = self.t(Key::ConverterFixEmbedded);
            let l_cover = self.t(Key::ConverterFixCover);
            let l_height = self.t(Key::ConverterHeight);
            let l_base = self.t(Key::ConverterBase);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.convert.fix_only, l_fix_only);
                ui.checkbox(&mut self.convert.force, l_force);
                ui.checkbox(&mut self.convert.import_after, l_import);
            });
            ui.checkbox(&mut self.convert.no_fix, l_no_fix);
            if !self.convert.no_fix {
                ui.collapsing(l_fixes, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.convert.fixes.refs, l_refs);
                        ui.checkbox(&mut self.convert.fixes.scale, l_scale);
                        ui.checkbox(&mut self.convert.fixes.anchor, l_anchor);
                        ui.checkbox(&mut self.convert.fixes.fall, l_fall);
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.convert.fixes.embedded, l_embedded);
                        ui.checkbox(&mut self.convert.fixes.cover, l_cover);
                    });
                    ui.horizontal(|ui| {
                        ui.label(l_height);
                        ui.add(egui::DragValue::new(&mut self.convert.height).range(16..=1024));
                    });
                    if !self.convert.fix_only {
                        ui.horizontal(|ui| {
                            ui.label(l_base);
                            ui.text_edit_singleline(&mut self.convert.base);
                        });
                    }
                });
            }
            ui.horizontal(|ui| {
                if ui.button(self.t(Key::BtnStartConversion)).clicked() && !self.convert.src.is_empty() && !self.convert.out.is_empty() {
                    let opts = ConvertOptions {
                        source: PathBuf::from(&self.convert.src),
                        name: self.convert.name.trim().to_string(),
                        output: PathBuf::from(&self.convert.out),
                        output_is_file: self.convert.fix_only,
                        force: self.convert.force,
                        base: if self.convert.base.trim().is_empty() { convert::DEFAULT_BASE.to_string() } else { self.convert.base.trim().to_string() },
                        fixes: if self.convert.no_fix { FixOptions::default() } else { self.convert.fixes.clone() },
                        height: self.convert.height,
                        fix_only: self.convert.fix_only,
                        import_after: self.convert.import_after,
                    };
                    self.convert.log.clear();
                    self.run_convert_task(opts);
                }
                if ui.button(self.t(Key::BtnCancel)).clicked() { self.cancel(); }
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| { ui.add(egui::TextEdit::multiline(&mut self.convert.log.as_str()).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace)); });
        });
        self.convert.open = open;
    }


}

fn tray_labels(lang: Lang) -> crate::ui::tray::TrayLabels {
    crate::ui::tray::TrayLabels {
        show: i18n::t(lang, Key::TrayShow).to_string(),
        summon: i18n::t(lang, Key::TraySummon).to_string(),
        dismiss_all: i18n::t(lang, Key::TrayDismissAll).to_string(),
        settings: i18n::t(lang, Key::TraySettings).to_string(),
        quit: i18n::t(lang, Key::TrayQuit).to_string(),
    }
}

/// The GUI runs in a child process so that closing the window really exits
/// that process (destroying the window and its dock icon) while the tray host
/// stays alive in the parent.
fn run_gui() -> eframe::Result<()> {
    let open_settings = std::env::args().any(|arg| arg == "--settings");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 720.0]),
        run_and_return: false,
        ..Default::default()
    };
    eframe::run_native("wl_shimeji GUI", options, Box::new(move |cc| {
        Ok(Box::new(ShimejiApp::new_with_settings(cc, open_settings)) as Box<dyn eframe::App>)
    }))
}

fn spawn_gui_child(open_settings: bool) -> Option<std::process::Child> {
    let exe = std::env::current_exe().ok()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.env("SHIMEJI_GUI_CHILD", "1");
    if open_settings { cmd.arg("--settings"); }
    cmd.spawn().ok()
}

fn stop_gui_child(child: &mut Option<std::process::Child>) {
    if let Some(c) = child.as_mut() {
        let _ = c.kill();
        let _ = c.wait();
    }
    *child = None;
}

pub fn run() -> eframe::Result<()> {
    if std::env::var_os("SHIMEJI_GUI_CHILD").is_some() {
        return run_gui();
    }

    let lang = Paths::from_env().ok()
        .and_then(|paths| config::load_gui_lang(&paths))
        .map(|code| Lang::from_code(&code))
        .unwrap_or(Lang::Es);

    let Some((rx, _ctl)) = crate::ui::tray::spawn_tray(tray_labels(lang)) else {
        // Without a system tray there is nowhere to hide to: just run the GUI
        // normally and let closing the window exit the application.
        return run_gui();
    };

    let mut child = spawn_gui_child(false);
    use crate::ui::tray::TrayEvent;
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(TrayEvent::Show) => {
                if child.is_none() { child = spawn_gui_child(false); }
            }
            Ok(TrayEvent::Settings) => {
                stop_gui_child(&mut child);
                child = spawn_gui_child(true);
            }
            Ok(TrayEvent::Summon) => {
                let name = Paths::from_env().ok()
                    .and_then(|paths| config::load_gui_selected_proto(&paths))
                    .unwrap_or_default();
                if !name.is_empty() {
                    let args = CtlCommand::Summon { name, count: Some(1), x: None, y: None, env: None, interactive: false }.to_args();
                    let _ = crate::core::shimejictl::run_ctl_sync(&args, None, Duration::from_secs(30));
                }
            }
            Ok(TrayEvent::DismissAll) => {
                let args = CtlCommand::Dismiss { id: None, all: true, interactive: false, same_type: false, others: false, env: None }.to_args();
                let _ = crate::core::shimejictl::run_ctl_sync(&args, None, Duration::from_secs(30));
            }
            Ok(TrayEvent::Quit) => {
                stop_gui_child(&mut child);
                return Ok(());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                stop_gui_child(&mut child);
                return Ok(());
            }
        }
        if let Some(c) = child.as_mut() {
            if c.try_wait().ok().flatten().is_some() { child = None; }
        }
    }
}
