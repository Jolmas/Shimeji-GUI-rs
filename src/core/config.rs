use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IeMode {
    #[default]
    Active,
    Focused,
    Largest,
    None,
}

impl IeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Focused => "focused",
            Self::Largest => "largest",
            Self::None => "none",
        }
    }
}

pub fn normalize_ie_mode(value: &str) -> Option<IeMode> {
    let v = value.trim().to_lowercase();
    if v.is_empty() {
        return None;
    }
    match v.as_str() {
        "active" | "seat" => Some(IeMode::Active),
        "focused" | "workspace" => Some(IeMode::Focused),
        "largest" | "biggest" | "max" => Some(IeMode::Largest),
        "none" | "off" | "disabled" => Some(IeMode::None),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Session {
    #[default]
    Labwc,
    Umbriel,
}

impl Session {
    pub fn key(self) -> &'static str {
        match self {
            Self::Labwc => "labwc",
            Self::Umbriel => "umbriel",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Labwc => "Labwc",
            Self::Umbriel => "Umbriel",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        match key.trim().to_lowercase().as_str() {
            "labwc" => Some(Self::Labwc),
            "umbriel" => Some(Self::Umbriel),
            _ => None,
        }
    }
    pub fn all() -> [Self; 2] {
        [Self::Labwc, Self::Umbriel]
    }
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
    pub data: PathBuf,
    pub config: PathBuf,
    pub plugin_config_dir: PathBuf,
    pub daemon_conf: PathBuf,
    pub autostart: PathBuf,
    pub desktop: PathBuf,
    pub gui_conf: PathBuf,
}

impl Paths {
    pub fn from_env() -> io::Result<Self> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
        Ok(Self::from_home(home))
    }

    pub fn from_home(home: PathBuf) -> Self {
        let xdg_config = env::var("XDG_CONFIG_HOME").unwrap_or_default();
        let config = home.join(".config/wl_shimeji");
        let plugin_config_dir = if xdg_config.starts_with('/') {
            PathBuf::from(xdg_config)
        } else {
            config.clone()
        };
        let data = home.join(".local/share/wl_shimeji");
        let daemon_conf = data.join("shimeji-overlayd.conf");
        let autostart = home.join(".config/autostart/shimeji-gui-rs.desktop");
        let desktop = home.join(".local/share/applications/shimeji-gui-rs.desktop");
        let gui_conf = config.join("shimeji-gui-rs.conf");
        Self { home, data, config, plugin_config_dir, daemon_conf, autostart, desktop, gui_conf }
    }

    pub fn plugin_dir(&self, session: Session) -> PathBuf {
        match session {
            Session::Labwc => self.home.join(".local/lib/wl_shimeji_labwc"),
            Session::Umbriel => self.home.join(".local/lib/wl_shimeji_umbriel"),
        }
    }

    pub fn plugin_conf(&self, session: Session) -> PathBuf {
        self.plugin_config_dir.join(format!("{}-support.conf", session.key()))
    }
}

pub fn read_config_value(path: &Path, key: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let data = line.split('#').next().unwrap_or("").trim();
        if !data.contains('=') {
            continue;
        }
        let mut parts = data.splitn(2, '=');
        let name = parts.next().unwrap_or("").trim();
        let value = parts.next().unwrap_or("").trim();
        if name.eq_ignore_ascii_case(key) {
            return Some(value.to_string());
        }
    }
    None
}

fn same_path(a: &str, b: &Path) -> bool {
    let a = expand_user(a);
    let a = fs::canonicalize(&a).unwrap_or(a);
    let b = fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    a == b
}

fn expand_user(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(s)
}

pub fn detect_session_from_env(xdg_current_desktop: &str, xdg_session_desktop: &str) -> Option<Session> {
    let desktop = format!("{} {}", xdg_current_desktop, xdg_session_desktop).to_lowercase();
    if desktop.contains("umbriel") {
        return Some(Session::Umbriel);
    }
    if desktop.contains("labwc") || desktop.contains("wlroots") {
        return Some(Session::Labwc);
    }
    None
}

pub fn detect_session() -> Option<Session> {
    detect_session_from_env(
        &env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        &env::var("XDG_SESSION_DESKTOP").unwrap_or_default(),
    )
}

pub fn effective_session(paths: &Paths) -> Session {
    let location = read_config_value(&paths.daemon_conf, "plugins_location");
    if let Some(loc) = location {
        for session in Session::all() {
            if same_path(&loc, &paths.plugin_dir(session)) {
                return session;
            }
        }
    }
    detect_session().unwrap_or(Session::Labwc)
}

pub fn save_plugins_location(paths: &Paths, session: Session) -> io::Result<()> {
    let target = paths.plugin_dir(session).to_string_lossy().to_string();
    if let Some(parent) = paths.daemon_conf.parent() {
        fs::create_dir_all(parent)?;
    }
    let lines: Vec<String> = fs::read_to_string(&paths.daemon_conf)
        .unwrap_or_default()
        .lines()
        .map(|l| l.to_string())
        .collect();

    let mut updated: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in &lines {
        let data = line.split('#').next().unwrap_or("").trim();
        if data.contains('=') {
            let mut parts = data.splitn(2, '=');
            let key = parts.next().unwrap_or("").trim().to_lowercase();
            if key == "plugins_location" {
                if !replaced {
                    updated.push(format!("plugins_location={target}"));
                    replaced = true;
                }
                continue;
            }
        }
        updated.push(line.clone());
    }
    if !replaced {
        updated.push(format!("plugins_location={target}"));
    }
    fs::write(&paths.daemon_conf, updated.join("\n") + "\n")
}

pub fn read_ie_mode(paths: &Paths, session: Session) -> Option<IeMode> {
    let path = paths.plugin_conf(session);
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let data = line.split('#').next().unwrap_or("").trim();
        if !data.contains('=') {
            continue;
        }
        let mut parts = data.splitn(2, '=');
        let key = parts.next().unwrap_or("").trim().to_lowercase();
        let value = parts.next().unwrap_or("").trim();
        if key == "ie_mode" || key == format!("{}_ie_mode", session.key()) {
            if let Some(mode) = normalize_ie_mode(value) {
                return Some(mode);
            }
        }
    }
    None
}

pub fn effective_ie_mode_from(env_value: Option<&str>, file_mode: Option<IeMode>) -> IeMode {
    if let Some(val) = env_value {
        if let Some(mode) = normalize_ie_mode(val) {
            return mode;
        }
    }
    file_mode.unwrap_or(IeMode::Active)
}

pub fn effective_ie_mode(paths: &Paths, session: Session) -> IeMode {
    let env_name = format!("{}_IE_MODE", session.key().to_uppercase());
    let env_val = env::var(&env_name).ok();
    effective_ie_mode_from(env_val.as_deref(), read_ie_mode(paths, session))
}

pub fn save_plugin_ie_mode(paths: &Paths, session: Session, mode: IeMode) -> io::Result<()> {
    let path = paths.plugin_conf(session);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let lines: Vec<String> = fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.to_string())
        .collect();

    let mut updated: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in &lines {
        let data = line.split('#').next().unwrap_or("").trim();
        if data.contains('=') {
            let mut parts = data.splitn(2, '=');
            let key = parts.next().unwrap_or("").trim().to_lowercase();
            if key == "ie_mode" || key == format!("{}_ie_mode", session.key()) {
                if !replaced {
                    updated.push(format!("ie_mode={}", mode.as_str()));
                    replaced = true;
                }
                continue;
            }
        }
        updated.push(line.clone());
    }
    if !replaced {
        if updated.is_empty() {
            updated.push(format!(
                "# {}Support: ventana en la que las mascotas pueden pararse.",
                session.label()
            ));
            updated.push("# active | focused | largest | none".to_string());
        }
        updated.push(format!("ie_mode={}", mode.as_str()));
    }
    fs::write(&path, updated.join("\n") + "\n")?;
    // SAFETY: env var set for child processes launched by this GUI, same as original.
    unsafe { env::set_var(format!("{}_IE_MODE", session.key().to_uppercase()), mode.as_str()); }
    Ok(())
}


pub fn load_gui_dark_mode(paths: &Paths) -> Option<bool> {
    let text = fs::read_to_string(&paths.gui_conf).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("theme=") {
            return match v.trim().to_lowercase().as_str() {
                "dark" | "oscuro" => Some(true),
                "light" | "claro" => Some(false),
                _ => None,
            };
        }
    }
    None
}

pub fn save_gui_dark_mode(paths: &Paths, dark: bool) -> io::Result<()> {
    if let Some(parent) = paths.gui_conf.parent() { fs::create_dir_all(parent)?; }
    let mut lines: Vec<String> = Vec::new();
    if let Ok(text) = fs::read_to_string(&paths.gui_conf) {
        for line in text.lines() {
            if line.trim_start().starts_with("theme=") { continue; }
            lines.push(line.to_string());
        }
    }
    lines.push(format!("theme={}", if dark { "dark" } else { "light" }));
    fs::write(&paths.gui_conf, lines.join("\n") + "\n")
}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_paths(tmp: &Path) -> Paths {
        Paths {
            home: tmp.to_path_buf(),
            data: tmp.join(".local/share/wl_shimeji"),
            config: tmp.join(".config/wl_shimeji"),
            plugin_config_dir: tmp.join(".config/wl_shimeji"),
            daemon_conf: tmp.join(".local/share/wl_shimeji/shimeji-overlayd.conf"),
            autostart: tmp.join(".config/autostart/shimeji-gui-rs.desktop"),
            desktop: tmp.join(".local/share/applications/shimeji-gui-rs.desktop"),
            gui_conf: tmp.join(".config/wl_shimeji/shimeji-gui-rs.conf"),
        }
    }

    #[test]
    fn normalize_ie_mode_aliases() {
        assert_eq!(normalize_ie_mode("active"), Some(IeMode::Active));
        assert_eq!(normalize_ie_mode(" seat "), Some(IeMode::Active));
        assert_eq!(normalize_ie_mode("FOCUSED"), Some(IeMode::Focused));
        assert_eq!(normalize_ie_mode("workspace"), Some(IeMode::Focused));
        assert_eq!(normalize_ie_mode("largest"), Some(IeMode::Largest));
        assert_eq!(normalize_ie_mode("BIGGEST"), Some(IeMode::Largest));
        assert_eq!(normalize_ie_mode("max"), Some(IeMode::Largest));
        assert_eq!(normalize_ie_mode("none"), Some(IeMode::None));
        assert_eq!(normalize_ie_mode("off"), Some(IeMode::None));
        assert_eq!(normalize_ie_mode("disabled"), Some(IeMode::None));
        assert_eq!(normalize_ie_mode(""), None);
        assert_eq!(normalize_ie_mode("wat"), None);
    }

    #[test]
    fn read_config_value_ignores_comments_and_matches_case_insensitive() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("conf");
        fs::write(&p, "# comment\nplugins_location = /a/b # inline\nOTHER=1\n").unwrap();
        assert_eq!(read_config_value(&p, "plugins_location").as_deref(), Some("/a/b"));
        assert_eq!(read_config_value(&p, "PLUGINS_LOCATION").as_deref(), Some("/a/b"));
        assert_eq!(read_config_value(&p, "missing"), None);
    }

    #[test]
    fn save_plugins_location_preserves_comments_and_replaces_key() {
        let dir = tempdir().unwrap();
        let paths = test_paths(dir.path());
        fs::create_dir_all(paths.daemon_conf.parent().unwrap()).unwrap();
        fs::write(&paths.daemon_conf, "# header\nfoo=1\nplugins_location=/old\nbar=2\n").unwrap();
        save_plugins_location(&paths, Session::Umbriel).unwrap();
        let got = fs::read_to_string(&paths.daemon_conf).unwrap();
        assert_eq!(got, "# header\nfoo=1\nplugins_location=/home/luis/.local/lib/wl_shimeji_umbriel\nbar=2\n".replace("/home/luis", dir.path().to_str().unwrap()));
    }

    #[test]
    fn save_plugins_location_appends_when_missing() {
        let dir = tempdir().unwrap();
        let paths = test_paths(dir.path());
        fs::create_dir_all(paths.daemon_conf.parent().unwrap()).unwrap();
        fs::write(&paths.daemon_conf, "# header\nfoo=1\n").unwrap();
        save_plugins_location(&paths, Session::Labwc).unwrap();
        let got = fs::read_to_string(&paths.daemon_conf).unwrap();
        let expected_target = paths.plugin_dir(Session::Labwc).to_string_lossy().to_string();
        assert!(got.contains(&format!("plugins_location={expected_target}")));
        assert!(got.starts_with("# header\nfoo=1\n"));
    }

    #[test]
    fn save_plugin_ie_mode_preserves_other_keys_and_comments() {
        let dir = tempdir().unwrap();
        let paths = test_paths(dir.path());
        let conf = paths.plugin_conf(Session::Labwc);
        fs::create_dir_all(conf.parent().unwrap()).unwrap();
        fs::write(&conf, "# c\nie_top_inset=10\nie_mode=active\n# end\n").unwrap();
        save_plugin_ie_mode(&paths, Session::Labwc, IeMode::Largest).unwrap();
        let got = fs::read_to_string(&conf).unwrap();
        assert_eq!(got, "# c\nie_top_inset=10\nie_mode=largest\n# end\n");
    }

    #[test]
    fn save_plugin_ie_mode_creates_header_when_new() {
        let dir = tempdir().unwrap();
        let paths = test_paths(dir.path());
        save_plugin_ie_mode(&paths, Session::Umbriel, IeMode::Focused).unwrap();
        let got = fs::read_to_string(paths.plugin_conf(Session::Umbriel)).unwrap();
        assert_eq!(got, "# UmbrielSupport: ventana en la que las mascotas pueden pararse.\n# active | focused | largest | none\nie_mode=focused\n");
    }

    #[test]
    fn detect_session_from_env_values() {
        assert_eq!(detect_session_from_env("KDE:Umbriel", ""), Some(Session::Umbriel));
        assert_eq!(detect_session_from_env("", "labwc:wlroots"), Some(Session::Labwc));
        assert_eq!(detect_session_from_env("KDE", "plasma"), None);
    }

    #[test]
    fn effective_ie_mode_precedence() {
        assert_eq!(effective_ie_mode_from(Some("focused"), Some(IeMode::Largest)), IeMode::Focused);
        assert_eq!(effective_ie_mode_from(Some("bogus"), Some(IeMode::Largest)), IeMode::Largest);
        assert_eq!(effective_ie_mode_from(None, Some(IeMode::None)), IeMode::None);
        assert_eq!(effective_ie_mode_from(None, None), IeMode::Active);
    }
}
