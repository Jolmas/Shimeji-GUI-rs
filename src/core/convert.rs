//! Conversor nativo de paquetes Shimeji a `.wlshm`.
//!
//! Portada a Rust de `shimeji-umbriel/tools/shimeji-to-wlshm-en` y de
//! `shimeji-jap-to-wlshm.py`. Soporta originales con XML japonés, instancias
//! Shimeji-EE y bundles (`manifest.json` + `animation.json`), la reparación de
//! paquetes ya convertidos y las cinco correcciones idempotentes (`scale`,
//! `anchor`, `fall`, `embedded`, `cover`).

use crate::core::japanese;
use crate::core::shimejictl::{CtlEvent, CtlHandle, CtlOutput, run_cmd_sync, spawn_task};
use crate::core::wlshm::WlshmArchive;
use image::{DynamicImage, ImageFormat};
use image::imageops::FilterType;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const FIX_ORDER: [&str; 5] = ["scale", "anchor", "fall", "embedded", "cover"];
const WLPK_HEADER_SIZE: usize = 512;
pub const DEFAULT_BASE: &str = "Shimeji.Shimeji";

/// Mapa de `shimeN.qoi` -> sprite del bundle, tomado del conversor original.
const SPRITE_FOR: [usize; 46] = [
     0,  1,  2,  3,  4,  5,  6,  7,  8,  9,
    10, 11, 12, 13, 10, 25, 10, 17, 18, 19,
    20, 21, 22, 23, 24, 25, 30, 30, 10, 30,
    31, 32, 35,  1,  2,  3, 67,  0,  0,  0,
     0,  0,  1,  2,  3,  0,
];

fn shimejis_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
        .join(".local/share/wl_shimeji/shimejis")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat { Japanese, Ee, Bundle, Wlshm, Unknown }

impl InputFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Japanese => "japanese",
            Self::Ee => "ee",
            Self::Bundle => "shimeji",
            Self::Wlshm => "wlshm",
            Self::Unknown => "unknown",
        }
    }
}

/// Los ZIP de Shimeji-EE suelen traer una única carpeta raíz. La eliminamos
/// para detectar el esquema tanto en paquetes planos como anidados.
fn strip_single_root(names: &BTreeSet<String>) -> BTreeSet<String> {
    let roots: BTreeSet<&str> = names.iter()
        .filter_map(|n| n.split_once('/').map(|(root, _)| root))
        .collect();
    if roots.len() == 1 && names.iter().all(|n| n.contains('/')) {
        let root = *roots.iter().next().unwrap();
        names.iter().filter_map(|n| n.strip_prefix(root).and_then(|rest| rest.strip_prefix('/'))
            .map(str::to_string)).collect()
    } else {
        names.clone()
    }
}

fn format_from_names(names: &BTreeSet<String>) -> InputFormat {
    let names = strip_single_root(names);
    let has_conf = names.iter().any(|n| n.starts_with("conf/"));
    let has_img = names.iter().any(|n| n.starts_with("img/"));
    if has_conf && has_img { return InputFormat::Ee; }
    if names.contains("manifest.json") && names.contains("animation.json") {
        return InputFormat::Bundle;
    }
    InputFormat::Unknown
}

fn xml_has_japanese(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    japanese::is_japanese(text)
}

fn japanese_in_zip(zip: &mut zip::ZipArchive<fs::File>, names: &BTreeSet<String>) -> bool {
    for name in names {
        if !name.to_lowercase().ends_with(".xml") { continue; }
        if !(name.starts_with("conf/") || name.contains("/conf/")) { continue; }
        let Ok(mut file) = zip.by_name(name) else { continue };
        let mut bytes = Vec::new();
        if file.read_to_end(&mut bytes).is_ok() && xml_has_japanese(&bytes) { return true; }
    }
    false
}

fn japanese_in_dir(path: &Path) -> bool {
    let Ok(entries) = fs::read_dir(path.join("conf")) else { return false };
    for entry in entries.flatten() {
        let candidate = entry.path();
        if candidate.extension().and_then(|e| e.to_str()) != Some("xml") { continue; }
        if let Ok(bytes) = fs::read(candidate) {
            if xml_has_japanese(&bytes) { return true; }
        }
    }
    false
}

fn read_xml_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}

fn locate_japanese_conf(root: &Path) -> Result<(PathBuf, PathBuf), String> {
    let conf = root.join("conf");
    let actions = conf.join("actions.xml");
    let behaviors = conf.join("behaviors.xml");
    if actions.is_file() && behaviors.is_file() {
        return Ok((actions, behaviors));
    }
    let mut found_actions = None;
    let mut found_behaviors = None;
    if let Ok(entries) = fs::read_dir(&conf) {
        for entry in entries.flatten() {
            let candidate = entry.path();
            if candidate.extension().and_then(|e| e.to_str()) != Some("xml") { continue; }
            let Ok(text) = read_xml_text(&candidate) else { continue };
            if japanese::is_actions(&text) { found_actions = Some(candidate); }
            else if japanese::is_behaviors(&text) { found_behaviors = Some(candidate); }
        }
    }
    match (found_actions, found_behaviors) {
        (Some(a), Some(b)) => Ok((a, b)),
        _ => Err(format!(
            "No encontré actions.xml/behaviors.xml ni los XML japoneses 動作.xml / 行動.xml dentro de {}",
            conf.display()
        )),
    }
}

/// Extrae un ZIP evitando rutas absolutas y `..` (Zip Slip).
fn unzip_to_dir(input: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(input).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("ZIP no válido: {e}"))?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let mut relative = PathBuf::new();
        for component in Path::new(entry.name()).components() {
            match component {
                Component::Normal(part) => relative.push(part),
                Component::CurDir => {}
                _ => continue,
            }
        }
        if relative.as_os_str().is_empty() { continue; }
        let target = dest.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut output = fs::File::create(&target).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Los paquetes originales suelen traer una única carpeta raíz; el script
/// Python la usa directamente como instancia.
fn unwrap_single_root(path: &Path) -> PathBuf {
    let Ok(mut entries) = fs::read_dir(path)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect::<Vec<_>>())
    else { return path.to_path_buf() };
    entries.sort();
    if entries.len() == 1 && entries[0].is_dir() { entries.remove(0) } else { path.to_path_buf() }
}

fn copy_tree(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = dest.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn is_image_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp")
    )
}

/// Traduce los XML japoneses y prepara una instancia Shimeji-EE de un solo
/// prototipo, igual que `shimeji-jap-to-wlshm.py`.
fn prepare_japanese_instance(root: &Path, name: &str, stage: &Path, log: &dyn Fn(&str)) -> Result<(), String> {
    let (actions_src, behaviors_src) = locate_japanese_conf(root)?;
    let conf_out = stage.join("conf");
    let images_out = stage.join("img").join(name);
    fs::create_dir_all(&conf_out).map_err(|e| e.to_string())?;
    fs::create_dir_all(&images_out).map_err(|e| e.to_string())?;

    for (source, dest_name, expected) in [
        (&actions_src, "actions.xml", "<ActionList"),
        (&behaviors_src, "behaviors.xml", "<BehaviorList"),
    ] {
        let translated = japanese::translate(&read_xml_text(source)?);
        if !translated.contains(expected) {
            log_line(log, &format!("  aviso: {dest_name} puede no haberse traducido por completo; revisa etiquetas/atributos residuales en japonés."));
        }
        fs::write(conf_out.join(dest_name), translated).map_err(|e| e.to_string())?;
    }

    let img_src = root.join("img");
    if !img_src.is_dir() {
        return Err(format!("No encontré el directorio {}", img_src.display()));
    }
    let named = img_src.join(name);
    if named.is_dir() {
        copy_tree(&named, &images_out)?;
        return Ok(());
    }
    let mut prototypes: Vec<String> = fs::read_dir(&img_src)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter(|e| e.path().is_dir() && e.file_name().to_string_lossy() != "unused")
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    if !prototypes.is_empty() {
        prototypes.sort();
        return Err(format!(
            "{} contiene prototipos ({}) pero ninguno se llama '{name}'. Elige uno con el campo Nombre.",
            img_src.display(), prototypes.join(", ")
        ));
    }
    let mut copied = 0usize;
    for entry in fs::read_dir(&img_src).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        if path.is_file() && is_image_file(&path) {
            fs::copy(&path, images_out.join(entry.file_name())).map_err(|e| e.to_string())?;
            copied += 1;
        }
    }
    if copied == 0 {
        return Err(format!("No encontré imágenes en {}", img_src.display()));
    }
    log_line(log, &format!("    imágenes copiadas: {copied}"));
    Ok(())
}

/// Detecta el esquema de entrada mirando el contenido, no solo la extensión.
pub fn detect_format(path: &Path) -> InputFormat {
    if path.is_dir() {
        if japanese_in_dir(path) { return InputFormat::Japanese; }
        let mut names = BTreeSet::new();
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if entry.path().is_dir() { names.insert(format!("{name}/")); }
                else { names.insert(name); }
            }
        }
        let found = format_from_names(&names);
        if found != InputFormat::Unknown { return found; }
        // Una sola carpeta raíz dentro del directorio seleccionado.
        if let Some(child) = fs::read_dir(path).ok().and_then(|mut d| d.next()).and_then(|e| e.ok().map(|e| e.path())) {
            if child.is_dir() { return detect_format(&child); }
        }
        return InputFormat::Unknown;
    }
    if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("wlshm")) {
        return InputFormat::Wlshm;
    }
    let Ok(file) = fs::File::open(path) else { return InputFormat::Unknown };
    let Ok(mut zip) = zip::ZipArchive::new(file) else { return InputFormat::Unknown };
    let names: BTreeSet<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();
    if japanese_in_zip(&mut zip, &names) { return InputFormat::Japanese; }
    format_from_names(&names)
}

#[derive(Debug, Clone, Default)]
pub struct FixOptions {
    pub scale: bool,
    pub anchor: bool,
    pub fall: bool,
    pub embedded: bool,
    pub cover: bool,
}

impl FixOptions {
    pub fn all() -> Self {
        Self { scale: true, anchor: true, fall: true, embedded: true, cover: true }
    }
    pub fn selected(&self, name: &str) -> bool {
        match name {
            "scale" => self.scale,
            "anchor" => self.anchor,
            "fall" => self.fall,
            "embedded" => self.embedded,
            "cover" => self.cover,
            _ => false,
        }
    }
    pub fn list(&self) -> Vec<&'static str> {
        FIX_ORDER.iter().copied().filter(|n| self.selected(n)).collect()
    }
    pub fn from_csv(value: &str) -> Result<Self, String> {
        let mut out = Self::default();
        if value.trim().is_empty() || value.trim() == "none" { return Ok(out); }
        for raw in value.split(',') {
            let name = raw.trim();
            if !FIX_ORDER.contains(&name) {
                return Err(format!("corrección desconocida: {name} (disponibles: {})", FIX_ORDER.join(",")));
            }
            match name {
                "scale" => out.scale = true,
                "anchor" => out.anchor = true,
                "fall" => out.fall = true,
                "embedded" => out.embedded = true,
                "cover" => out.cover = true,
                _ => {}
            }
        }
        Ok(out)
    }
}

#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub source: PathBuf,
    /// Nombre del prototipo; obligatorio para elegir `img/<name>` en originales japoneses.
    /// Si está vacío se deduce del nombre del ZIP/directorio.
    pub name: String,
    pub output: PathBuf,
    /// `true` cuando `output` es un archivo `.wlshm`; si no, es un directorio.
    pub output_is_file: bool,
    pub force: bool,
    pub base: String,
    pub fixes: FixOptions,
    pub height: u32,
    pub fix_only: bool,
    pub import_after: bool,
}

fn log_line(log: &dyn Fn(&str), text: &str) {
    if !text.is_empty() { log(text); }
}

fn canceled(cancel: &Arc<AtomicBool>) -> Result<(), String> {
    if cancel.load(std::sync::atomic::Ordering::SeqCst) {
        Err("Operación cancelada por el usuario".to_string())
    } else {
        Ok(())
    }
}

/// Directorio temporal que se limpia incluso si la conversión falla.
struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> Result<Self, String> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        // La ruta incluye un nonce, pero evita reutilizar restos de otra sesión.
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path { &self.0 }
}

impl Drop for TempDir {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn derive_name(opts: &ConvertOptions) -> String {
    let from_opts = opts.name.trim();
    if !from_opts.is_empty() { return from_opts.to_string(); }
    let deduced = if opts.source.is_dir() {
        opts.source.file_name()
    } else {
        opts.source.file_stem()
    }
    .unwrap_or_default().to_string_lossy().trim().to_string();
    if deduced.is_empty() { "shimeji".to_string() } else { deduced }
}

/// Ejecuta `shimejictl convert` y devuelve el `.wlshm` que generó.
fn run_official_convert(
    source: &Path,
    expected_name: Option<&str>,
    work_dir: &Path,
    output: &Path,
    output_is_file: bool,
    force: bool,
    selection: &str,
    cancel: &Arc<AtomicBool>,
    log: &dyn Fn(&str),
) -> Result<PathBuf, String> {
    if output_is_file {
        if output.is_dir() {
            return Err("el destino debe ser un archivo .wlshm, no una carpeta".to_string());
        }
        if output.exists() && !force {
            return Err(format!("{} ya existe (usa -f para sobrescribir)", output.display()));
        }
    }
    fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
    log_line(log, "[2] Usando el convertidor oficial (shimejictl convert)");
    let mut args = vec![
        "convert".to_string(),
        source.to_string_lossy().to_string(),
        "-O".to_string(),
        work_dir.to_string_lossy().to_string(),
    ];
    if force { args.push("-f".to_string()); }
    log_line(log, &format!("$ shimejictl {}", args.join(" ")));
    let result = run_cmd_sync("shimejictl", &args, Some(selection), Duration::from_secs(120), Some(cancel.clone()))
        .map_err(|e| e.to_string())?;
    log_line(log, &result.output);
    canceled(cancel)?;
    let failures: Vec<&str> = result.output.lines().filter(|line| {
        let lower = line.to_lowercase();
        lower.contains("conversion failed") || lower.contains("no prototypes selected")
    }).collect();
    if result.code != 0 {
        return Err(format!("shimejictl convert falló (exit {})", result.code));
    }
    // shimejictl puede devolver 0 y dejar un .wlshm de 0 bytes cuando falla un
    // prototipo; solo aceptamos paquetes que se puedan abrir como WLPK.
    let mut converted: Vec<PathBuf> = fs::read_dir(work_dir).map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file()
            && p.extension().and_then(|e| e.to_str()) == Some("wlshm")
            && WlshmArchive::open(p).is_ok())
        .collect();
    converted.sort();
    if converted.is_empty() {
        let detail = if failures.is_empty() { String::new() } else { format!(": {}", failures.join("; ")) };
        return Err(format!("shimejictl convert no produjo ningún .wlshm válido{detail}"));
    }
    if !failures.is_empty() {
        log_line(log, "  aviso: shimejictl informó de conversiones fallidas; se omiten esos prototipos");
    }
    let expected = expected_name.map(|name| work_dir.join(format!("Shimeji.{name}.wlshm")));
    let found = expected.filter(|p| converted.contains(p)).or_else(|| converted.first().cloned()).unwrap();
    if output_is_file {
        if let Some(parent) = output.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        fs::copy(&found, &output).map_err(|e| e.to_string())?;
        Ok(output.to_path_buf())
    } else {
        Ok(found)
    }
}

/// Ejecuta la conversión completa. Devuelve la ruta del `.wlshm` resultante.
pub fn run_conversion(opts: &ConvertOptions, cancel: &Arc<AtomicBool>, log: &dyn Fn(&str)) -> Result<PathBuf, String> {
    if !opts.source.exists() {
        return Err(format!("no existe: {}", opts.source.display()));
    }

    let mut output = opts.output.clone();
    if opts.fix_only {
        log_line(log, "[1] Cargando paquete .wlshm");
        if output.is_dir() {
            return Err("en modo «solo corregir» el destino debe ser un archivo .wlshm, no una carpeta".to_string());
        }
        if output.extension().and_then(|e| e.to_str()) != Some("wlshm") {
            output.set_extension("wlshm");
        }
        if output.exists() && !opts.force {
            return Err(format!("{} ya existe (usa -f para sobrescribir)", output.display()));
        }
        let mut package = WlshmArchive::open(&opts.source)?;
        validate(&package, 1, log)?;
        apply_fixes(&mut package, opts, cancel, log)?;
        if package.is_dirty() {
            let bytes = package.save()?;
            if let Some(parent) = output.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
            fs::write(&output, bytes).map_err(|e| e.to_string())?;
            log_line(log, &format!("    paquete actualizado: {} ({} bytes)", output.display(), fs::metadata(&output).map_err(|e| e.to_string())?.len()));
        }
        log_line(log, "[5] Re-validando paquete corregido");
        validate(&package, 1, log)?;
        return Ok(output);
    }

    // Un destino escrito a mano con extensión .wlshm equivale a la opción -o
    // del script; en caso contrario se interpreta como carpeta de salida.
    let output_is_file = opts.output_is_file
        || output.extension().and_then(|e| e.to_str()) == Some("wlshm");
    let kind = detect_format(&opts.source);
    log_line(log, &format!("[1] Formato detectado: {}", kind.label()));

    match kind {
        InputFormat::Japanese => {
            let name = derive_name(opts);
            log_line(log, &format!("[2] Traduciendo XML japonés y preparando instancia EE ({name})"));
            let temp = TempDir::new("shimeji-gui-rs-jap")?;
            let root = if opts.source.is_dir() {
                unwrap_single_root(&opts.source)
            } else {
                let extracted = temp.path().join("original");
                fs::create_dir_all(&extracted).map_err(|e| e.to_string())?;
                unzip_to_dir(&opts.source, &extracted)?;
                // Igual que el script: si el paquete trae una única carpeta raíz, se usa esa.
                unwrap_single_root(&extracted)
            };
            let stage = temp.path().join("ee-instance");
            prepare_japanese_instance(&root, &name, &stage, log)?;
            let work_dir = if output_is_file {
                temp.path().join("out")
            } else {
                opts.output.clone()
            };
            output = run_official_convert(
                &stage, Some(&name), &work_dir, &output, output_is_file,
                opts.force, "1\n", cancel, log,
            )?;
        }
        InputFormat::Ee => {
            let temp = if output_is_file { Some(TempDir::new("shimeji-gui-rs-convert")?) } else { None };
            let work_dir = temp.as_ref().map(|t| t.path().to_path_buf()).unwrap_or_else(|| opts.output.clone());
            output = run_official_convert(
                &opts.source, None, &work_dir, &output, output_is_file,
                opts.force, "A\n", cancel, log,
            )?;
        }
        InputFormat::Bundle => {
            log_line(log, "[2] Construyendo .wlshm con sprites del bundle y lógica base");
            let name = format!("Shimeji.{}", slugify(&opts.source.file_stem().unwrap_or_default().to_string_lossy()));
            if output_is_file {
                if output.exists() && !opts.force {
                    return Err(format!("{} ya existe (usa -f para sobrescribir)", output.display()));
                }
                if let Some(parent) = output.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
            } else {
                fs::create_dir_all(&opts.output).map_err(|e| e.to_string())?;
                output = opts.output.join(format!("{name}.wlshm"));
                if output.exists() && !opts.force {
                    return Err(format!("{} ya existe (usa -f para sobrescribir)", output.display()));
                }
            }
            convert_bundle(&opts.source, &output, &opts.base, log)?;
        }
        InputFormat::Wlshm => {
            return Err("un .wlshm de entrada requiere el modo «solo corregir». Activa esa opción.".to_string());
        }
        InputFormat::Unknown => {
            return Err("formato no reconocido: se esperaba XML japonés, conf/+img/, manifest.json+animation.json o .wlshm".to_string());
        }
    }

    canceled(cancel)?;
    let size = fs::metadata(&output).map_err(|e| e.to_string())?.len();
    log_line(log, &format!("    escrito: {} ({size} bytes)", output.display()));
    log_line(log, "[3] Validando paquete");
    let mut package = WlshmArchive::open(&output)?;
    validate(&package, 46, log)?;

    apply_fixes(&mut package, opts, cancel, log)?;
    if package.is_dirty() {
        let bytes = package.save()?;
        fs::write(&output, &bytes).map_err(|e| e.to_string())?;
        log_line(log, &format!("    paquete actualizado: {} ({} bytes)", output.display(), bytes.len()));
    }
    log_line(log, "[5] Re-validando paquete corregido");
    validate(&package, 46, log)?;
    Ok(output)
}

fn validate(package: &WlshmArchive, min_assets: usize, log: &dyn Fn(&str)) -> Result<Value, String> {
    let required = ["manifest.json", "scripts.json", "actions.json", "behaviors.json"];
    let names = package.names();
    let missing: Vec<&str> = required.iter().copied().filter(|n| !names.iter().any(|x| x == n)).collect();
    if !missing.is_empty() {
        return Err(format!("faltan miembros del tar: {}", missing.join(", ")));
    }
    let manifest = package.get_json("manifest.json")?;
    let actions = package.get_json("actions.json")?;
    let behaviors = package.get_json("behaviors.json")?;
    let assets: Vec<String> = names.iter()
        .filter(|n| n.starts_with("assets/") && n.ends_with(".qoi"))
        .cloned().collect();
    if assets.len() < min_assets {
        return Err(format!("solo {} sprites QOI (se esperaban {min_assets})", assets.len()));
    }
    let first = package.get(&assets[0]).ok_or_else(|| "asset QOI ilegible".to_string())?;
    decode_qoi(first)?;
    let display = manifest.get("display_name").or_else(|| manifest.get("name"))
        .and_then(Value::as_str).unwrap_or("?");
    log_line(log, &format!("  validado: {display} | {} acciones | {} comportamientos | {} sprites",
        actions.as_array().map_or(0, Vec::len),
        behaviors.get("definitions").and_then(Value::as_array).map_or(0, Vec::len),
        assets.len()));
    Ok(manifest)
}

fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut dashed = false;
    for c in text.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dashed = false;
        } else if !dashed && !out.is_empty() {
            out.push('-');
            dashed = true;
        }
    }
    while out.ends_with('-') { out.pop(); }
    if out.is_empty() { "shimeji".to_string() } else { out }
}

fn wlshm_header(name: &str, version: &str) -> Result<Vec<u8>, String> {
    let name_bytes = name.as_bytes();
    let version_bytes = version.as_bytes();
    if name_bytes.is_empty() || name_bytes.len() > 255 || version_bytes.len() > 255
        || 2 + name_bytes.len() + version_bytes.len() > WLPK_HEADER_SIZE {
        return Err("la cabecera WLPK resultante es demasiado larga".to_string());
    }
    let mut header = Vec::with_capacity(WLPK_HEADER_SIZE);
    header.extend_from_slice(b"WLPK");
    header.push(name_bytes.len() as u8);
    header.extend_from_slice(name_bytes);
    header.push(version_bytes.len() as u8);
    header.extend_from_slice(version_bytes);
    header.resize(WLPK_HEADER_SIZE, 0);
    Ok(header)
}

fn decode_qoi(data: &[u8]) -> Result<DynamicImage, String> {
    image::load_from_memory_with_format(data, ImageFormat::Qoi).map_err(|e| e.to_string())
}

fn encode_qoi(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Qoi).map_err(|e| e.to_string())?;
    Ok(bytes)
}

fn tar_add<W: std::io::Write>(builder: &mut tar::Builder<W>, member: &str, data: &[u8]) -> Result<(), String> {
    let mut header = tar::Header::new_ustar();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, member, Cursor::new(data)).map_err(|e| e.to_string())
}

fn base_prototype(base: &str, hint: &str, log: &dyn Fn(&str)) -> Result<PathBuf, String> {
    let root = shimejis_dir();
    let candidate = root.join(base);
    if candidate.join("actions.json").is_file() { return Ok(candidate); }
    let fallback = root.parent().unwrap_or(&root).join("prototypes").join(base);
    if fallback.join("actions.json").is_file() { return Ok(fallback); }

    // La base por defecto del script (`Shimeji.Shimeji`) no suele estar
    // instalada. Usamos un prototipo real para no bloquear la conversión.
    let hint = slugify(hint);
    let mut candidates: Vec<PathBuf> = fs::read_dir(&root).ok().into_iter().flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir() && p.join("actions.json").is_file())
        .collect();
    candidates.sort();
    let label = |p: &Path| p.file_name().unwrap_or_default().to_string_lossy().to_string();
    if let Some(found) = candidates.iter().find(|p| {
        let name = slugify(&label(p)).replace("shimeji-", "");
        !hint.is_empty() && (name.contains(&hint) || hint.contains(&name))
    }).or_else(|| candidates.first()) {
        log_line(log, &format!("  aviso: base '{base}' no disponible; usando '{}'", label(found)));
        return Ok(found.clone());
    }
    Err(format!(
        "el prototipo base '{base}' no existe y no hay prototipos instalados con actions.json en {}",
        root.display()
    ))
}

fn convert_bundle(input: &Path, output: &Path, base: &str, log: &dyn Fn(&str)) -> Result<(), String> {
    let hint = input.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let base_dir = base_prototype(base, &hint, log)?;
    let file = fs::File::open(input).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("ZIP no válido: {e}"))?;
    let names: BTreeSet<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();
    if !names.contains("manifest.json") || !names.contains("animation.json") {
        return Err("el ZIP no es un bundle Shimeji ni una instancia Shimeji-EE".to_string());
    }
    let available_sprites: BTreeSet<usize> = names.iter().filter_map(|name| {
        name.strip_prefix("sprites/")?.strip_suffix(".webp")?.parse().ok()
    }).collect();
    if available_sprites.is_empty() {
        return Err("el bundle no contiene sprites/*.webp".to_string());
    }

    let bundle_manifest: Value = {
        let mut file = zip.by_name("manifest.json").map_err(|e| e.to_string())?;
        let mut text = String::new();
        file.read_to_string(&mut text).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| e.to_string())?
    };
    let display_name = bundle_manifest.get("name").and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| input.file_stem().unwrap_or_default().to_string_lossy().to_string());
    let name = format!("Shimeji.{}", slugify(&display_name));
    let mut manifest: Value = serde_json::from_slice(
        &fs::read(base_dir.join("manifest.json")).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let version = manifest.get("version").and_then(Value::as_str)
        .unwrap_or("1.0").to_string();
    let artist = bundle_manifest.get("author").and_then(|a| a.get("name"))
        .and_then(Value::as_str).unwrap_or("Local Creator");
    if let Value::Object(map) = &mut manifest {
        map.insert("name".into(), json!(name));
        map.insert("display_name".into(), json!(display_name));
        map.insert("description".into(), json!(format!("Converted from {}", input.file_name().unwrap_or_default().to_string_lossy())));
        map.insert("artist".into(), json!(artist));
    }
    let actions: Value = serde_json::from_slice(
        &fs::read(base_dir.join("actions.json")).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let scripts = fs::read(base_dir.join("scripts.json")).map_err(|e| e.to_string())?;
    let behaviors = fs::read(base_dir.join("behaviors.json")).map_err(|e| e.to_string())?;

    let mut tar_buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_buf);
        tar_add(&mut builder, "manifest.json", &serde_json::to_vec(&manifest).map_err(|e| e.to_string())?)?;
        tar_add(&mut builder, "scripts.json", &scripts)?;
        tar_add(&mut builder, "actions.json", &serde_json::to_vec(&actions).map_err(|e| e.to_string())?)?;
        tar_add(&mut builder, "behaviors.json", &behaviors)?;
        let mut dir = tar::Header::new_ustar();
        dir.set_entry_type(tar::EntryType::Directory);
        dir.set_size(0);
        dir.set_cksum();
        builder.append_data(&mut dir, "assets/", Cursor::new(&[])).map_err(|e| e.to_string())?;

        for (index, sprite) in SPRITE_FOR.iter().enumerate() {
            let requested = *sprite;
            let sprite = if available_sprites.contains(&requested) {
                requested
            } else {
                let nearest = *available_sprites.iter()
                    .min_by_key(|n| ((**n as i64 - requested as i64).abs(), **n)).unwrap();
                log_line(log, &format!("  aviso: sprite {requested:04} no existe; usando {nearest:04}"));
                nearest
            };
            let member = format!("sprites/{sprite:04}.webp");
            let bytes = {
                let mut file = zip.by_name(&member)
                    .map_err(|_| format!("falta el sprite {member} en el bundle"))?;
                let mut data = Vec::new();
                file.read_to_end(&mut data).map_err(|e| e.to_string())?;
                data
            };
            let image = image::load_from_memory_with_format(&bytes, ImageFormat::WebP)
                .map_err(|e| format!("{member} no es WebP válido: {e}"))?;
            let resized = image::imageops::resize(&image, 128, 128, FilterType::Lanczos3);
            let qoi = encode_qoi(&DynamicImage::ImageRgba8(resized))?;
            tar_add(&mut builder, &format!("assets/shime{}.qoi", index + 1), &qoi)?;
        }
        builder.finish().map_err(|e| e.to_string())?;
    }
    drop(zip);

    let mut raw = wlshm_header(&name, &version)?;
    raw.extend_from_slice(&tar_buf);
    fs::write(output, &raw).map_err(|e| e.to_string())?;
    log_line(log, &format!("    base: {}", base_dir.display()));
    Ok(())
}

// ---------------------------------------------------------------------------
// Correcciones. Todas son idempotentes y trabajan sobre el tar en memoria.
// ---------------------------------------------------------------------------

fn referenced_images(actions: &Value) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    let Some(list) = actions.as_array() else { return refs };
    for action in list {
        let Some(content) = action.get("content").and_then(Value::as_array) else { continue };
        for item in content {
            let Some(frames) = item.get("frames").and_then(Value::as_array) else { continue };
            for frame in frames {
                for key in ["image", "image_right"] {
                    if let Some(value) = frame.get(key).and_then(Value::as_str) {
                        if !value.is_empty() { refs.insert(value.to_string()); }
                    }
                }
            }
        }
    }
    refs
}

fn scale_point(value: Option<&Value>, factor: f64) -> Option<Value> {
    value.and_then(Value::as_f64).map(|v| json!((v * factor).round() as i64))
}

fn fix_scale(package: &mut WlshmArchive, target: u32, log: &dyn Fn(&str)) -> Result<Vec<String>, String> {
    if !package.has("actions.json") { return Ok(Vec::new()); }
    let mut actions = package.get_json("actions.json")?;
    let refs = referenced_images(&actions);
    let mut heights = Vec::new();
    for name in &refs {
        let member = format!("assets/{name}");
        if let Some(data) = package.get(&member) {
            match decode_qoi(data) {
                Ok(image) => heights.push(image.height()),
                Err(e) => log_line(log, &format!("  aviso: no se pudo leer {name}: {e}")),
            }
        }
    }
    if heights.is_empty() { return Ok(Vec::new()); }

    let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
    for height in &heights { *counts.entry(*height).or_default() += 1; }
    let (common, _) = counts.iter().max_by_key(|(_, count)| **count)
        .map(|(h, c)| (*h, *c)).unwrap();
    if common == target { return Ok(Vec::new()); }
    if heights.iter().any(|h| *h < target) || counts.len() > 1 {
        log_line(log, &format!("  aviso: tamaños mezclados {:?}, se omite el redimensionado", counts));
        return Ok(Vec::new());
    }

    let factor = target as f64 / common as f64;
    let mut resized = 0usize;
    for name in &refs {
        let member = format!("assets/{name}");
        let Some(data) = package.get(&member).map(|d| d.to_vec()) else { continue };
        let image = decode_qoi(&data)?;
        let width = (image.width() as f64 * factor).round().max(1.0) as u32;
        if image.width() == width && image.height() == target { continue; }
        let output = image::imageops::resize(&image, width, target, FilterType::Lanczos3);
        package.set(&member, encode_qoi(&DynamicImage::ImageRgba8(output))?);
        resized += 1;
    }

    if let Some(list) = actions.as_array_mut() {
        for action in list.iter_mut() {
            let Some(content) = action.get_mut("content").and_then(Value::as_array_mut) else { continue };
            for item in content.iter_mut() {
                if let Some(frames) = item.get_mut("frames").and_then(Value::as_array_mut) {
                    for frame in frames.iter_mut() {
                        for key in ["image_anchor_x", "image_anchor_y"] {
                            if frame.get(key).is_some() {
                                let scaled = scale_point(frame.get(key), factor);
                                if let Some(scaled) = scaled { frame[key] = scaled; }
                            }
                        }
                    }
                }
                if let Some(hotspots) = item.get_mut("hotspots").and_then(Value::as_array_mut) {
                    for hotspot in hotspots.iter_mut() {
                        for key in ["x", "y", "width", "height"] {
                            if hotspot.get(key).is_some() {
                                let scaled = scale_point(hotspot.get(key), factor);
                                if let Some(scaled) = scaled { hotspot[key] = scaled; }
                            }
                        }
                    }
                }
            }
        }
    }
    if resized > 0 { package.set_json("actions.json", &actions)?; }
    Ok(if resized > 0 { vec![format!("{resized} sprites {common}px -> {target}px")] } else { Vec::new() })
}

fn transparent_bottom(data: &[u8]) -> Result<(usize, usize), String> {
    let image = decode_qoi(data)?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = rgba.as_raw();
    let mut count = 0usize;
    for y in (0..height as usize).rev() {
        let row = &pixels[y * width as usize * 4..(y + 1) * width as usize * 4];
        if row.chunks_exact(4).any(|p| p[3] != 0) { break; }
        count += 1;
    }
    Ok((count, height as usize))
}

fn fix_anchor(package: &mut WlshmArchive) -> Result<Vec<String>, String> {
    if !package.has("actions.json") { return Ok(Vec::new()); }
    let mut actions = package.get_json("actions.json")?;
    let mut changed = 0usize;
    if let Some(list) = actions.as_array_mut() {
        for action in list.iter_mut() {
            let Some(content) = action.get_mut("content").and_then(Value::as_array_mut) else { continue };
            for item in content.iter_mut() {
                let Some(frames) = item.get_mut("frames").and_then(Value::as_array_mut) else { continue };
                for frame in frames.iter_mut() {
                    for key in ["image", "image_right"] {
                        let Some(image) = frame.get(key).and_then(Value::as_str) else { continue };
                        let Some(data) = package.get(&format!("assets/{image}")).map(|d| d.to_vec()) else { continue };
                        let (bottom, height) = transparent_bottom(&data)?;
                        let Some(anchor) = frame.get("image_anchor_y").and_then(Value::as_i64) else { continue };
                        if anchor < height as i64 - 1 { continue; }
                        let applied = frame.get("wlshm_bottom_trim").and_then(Value::as_i64).unwrap_or(0);
                        let delta = bottom as i64 - applied;
                        if delta == 0 { continue; }
                        frame["image_anchor_y"] = json!(anchor - delta);
                        frame["wlshm_bottom_trim"] = json!(bottom);
                        changed += 1;
                    }
                }
            }
        }
    }
    if changed > 0 { package.set_json("actions.json", &actions)?; }
    Ok(if changed > 0 { vec![format!("{changed} frames ajustados")] } else { Vec::new() })
}

fn fix_fall(package: &mut WlshmArchive) -> Result<Vec<String>, String> {
    if !package.has("behaviors.json") { return Ok(Vec::new()); }
    let mut data = package.get_json("behaviors.json")?;
    let mut changed = Vec::new();
    for section in ["definitions", "root_behavior_list"] {
        let Some(entries) = data.get_mut(section).and_then(Value::as_array_mut) else { continue };
        for entry in entries.iter_mut() {
            if entry.get("name").and_then(Value::as_str) != Some("fall") { continue; }
            let frequency = entry.get("frequency");
            if frequency.is_none() || frequency == Some(&Value::Null) || frequency == Some(&json!(0)) || frequency == Some(&json!(false)) {
                continue;
            }
            changed.push(format!("{section}:{}->0", frequency.unwrap()));
            entry["frequency"] = json!(0);
        }
    }
    if !changed.is_empty() { package.set_json("behaviors.json", &data)?; }
    Ok(changed)
}

fn index_by_name(actions: &Value) -> BTreeMap<String, Value> {
    let mut index = BTreeMap::new();
    if let Some(list) = actions.as_array() {
        for action in list {
            if let Some(name) = action.get("name").and_then(Value::as_str) {
                index.entry(name.to_string()).or_insert_with(|| action.clone());
            }
        }
    }
    index
}

fn is_vulnerable(action: Option<&Value>) -> bool {
    let Some(action) = action else { return false };
    if action.get("type").and_then(Value::as_str) == Some("Move") { return true; }
    if action.get("type").and_then(Value::as_str) != Some("Embedded") { return false; }
    action.get("embedded_type").and_then(Value::as_str)
        .is_some_and(|t| !t.is_empty() && t.to_lowercase() != "none")
}

fn already_fixed(actions: &Value, name: &str) -> bool {
    let index = index_by_name(actions);
    let Some(wrapper) = index.get(name) else { return false };
    if wrapper.get("type").and_then(Value::as_str) != Some("Sequence") { return false; }
    let Some(content) = wrapper.get("content").and_then(Value::as_array) else { return false };
    for item in content {
        if item.get("type").and_then(Value::as_str) != Some("ActionReference") { continue; }
        let Some(inner) = item.get("action_name").and_then(Value::as_str) else { continue };
        if inner != name && is_vulnerable(index.get(inner)) { return true; }
    }
    false
}

fn unique_inner_name(actions: &Value, name: &str, action: &Value) -> String {
    let used: BTreeSet<String> = actions.as_array().map(|list| list.iter()
        .filter_map(|a| a.get("name").and_then(Value::as_str).map(str::to_string))
        .collect()).unwrap_or_default();
    let suffix = if action.get("type").and_then(Value::as_str) == Some("Embedded") { "Embedded" } else { "Inner" };
    let mut candidate = format!("{name}{suffix}");
    let mut number = 2;
    while used.contains(&candidate) {
        candidate = format!("{name}{suffix}{number}");
        number += 1;
    }
    candidate
}

fn make_wrapper(name: &str, inner: &str) -> Value {
    json!({
        "type": "Sequence",
        "name": name,
        "content": [{
            "type": "ActionReference",
            "action_name": inner,
            "duration": null,
            "condition": null,
            "locals_overrides": {},
            "locals_count": 0,
        }],
        "content_count": 1,
        "local_variables": {},
        "local_variables_count": 0,
        "embedded_type": null,
        "loop": false,
        "condition": null,
        "border_type": "Any",
    })
}

fn fix_embedded(package: &mut WlshmArchive) -> Result<Vec<String>, String> {
    if !(package.has("actions.json") && package.has("behaviors.json")) { return Ok(Vec::new()); }
    let mut actions = package.get_json("actions.json")?;
    let behaviors = package.get_json("behaviors.json")?;
    let mut index = index_by_name(&actions);
    let mut called = Vec::new();
    if let Some(definitions) = behaviors.get("definitions").and_then(Value::as_array) {
        for definition in definitions {
            let Some(name) = definition.get("action").and_then(Value::as_str) else { continue };
            if is_vulnerable(index.get(name)) && !called.contains(&name.to_string()) {
                called.push(name.to_string());
            }
        }
    }
    let mut changed = Vec::new();
    for name in called {
        if already_fixed(&actions, &name) { continue; }
        let Some(action) = index.get(&name).cloned() else { continue };
        let inner = unique_inner_name(&actions, &name, &action);
        if let Some(list) = actions.as_array_mut() {
            for entry in list.iter_mut() {
                if entry.get("name").and_then(Value::as_str) == Some(name.as_str()) {
                    entry["name"] = json!(inner);
                    break;
                }
            }
        }
        if let Some(list) = actions.as_array_mut() { list.push(make_wrapper(&name, &inner)); }
        index = index_by_name(&actions);
        changed.push(format!("{name} -> {inner}"));
    }
    if !changed.is_empty() { package.set_json("actions.json", &actions)?; }
    Ok(changed)
}

fn is_no_image_placeholder(data: &[u8]) -> bool {
    let image = image::load_from_memory_with_format(data, ImageFormat::Qoi)
        .or_else(|_| image::load_from_memory(data));
    let Ok(image) = image else { return false };
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width < 256 || height < 256 { return false; }
    let pixels = rgba.as_raw();
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (width, height, 0u32, 0u32);
    let mut found = false;
    for y in 0..height {
        for x in 0..width {
            if pixels[(y * width + x) as usize * 4 + 3] > 0 {
                found = true;
                min_x = min_x.min(x); min_y = min_y.min(y);
                max_x = max_x.max(x); max_y = max_y.max(y);
            }
        }
    }
    if !found { return false; }
    let box_w = max_x - min_x + 1;
    let box_h = max_y - min_y + 1;
    let ratio = box_w as f64 / box_h.max(1) as f64;
    if !(1.5..=4.0).contains(&ratio) { return false; }

    let mut visible = 0usize;
    let mut colors: HashMap<(u8, u8, u8), usize> = HashMap::new();
    for pixel in pixels.chunks_exact(4) {
        if pixel[3] > 16 {
            visible += 1;
            *colors.entry((pixel[0], pixel[1], pixel[2])).or_default() += 1;
        }
    }
    if visible == 0 || visible as f64 / (width * height) as f64 > 0.25 { return false; }
    if colors.len() > 400 { return false; }
    let Some((&(r, g, b), count)) = colors.iter().max_by_key(|(_, count)| **count) else { return false };
    if (*count as f64 / visible as f64) < 0.15 { return false; }
    (r >= 200 && g <= 80 && b <= 80) || r.min(g).min(b) >= 180
}

fn safe_folder_name(name: &str) -> String {
    name.to_lowercase().replace(['/', '\\'], "_").replace('\0', "")
}

fn pick_source_sprite(package: &WlshmArchive, refs: &BTreeSet<String>) -> Option<String> {
    for name in ["shime0000.qoi", "shime1.qoi", "shime0001.qoi", "shime0.qoi", "shime0000.png", "shime1.png"] {
        let member = format!("assets/{name}");
        if refs.contains(name) {
            if let Some(data) = package.get(&member) {
                if !is_no_image_placeholder(data) { return Some(member); }
            }
        }
    }
    let mut names: Vec<String> = refs.iter().cloned().collect();
    names.sort();
    for name in names {
        let member = format!("assets/{name}");
        if let Some(data) = package.get(&member) {
            if !is_no_image_placeholder(data) { return Some(member); }
        }
    }
    None
}

fn fix_cover(package: &mut WlshmArchive, log: &dyn Fn(&str)) -> Result<Vec<String>, String> {
    if !(package.has("manifest.json") && package.has("actions.json")) { return Ok(Vec::new()); }
    let manifest = package.get_json("manifest.json")?;
    let name = manifest.get("display_name").or_else(|| manifest.get("name"))
        .and_then(Value::as_str).unwrap_or("shimeji");
    let actions = package.get_json("actions.json")?;
    let refs = referenced_images(&actions);
    let mut changed = Vec::new();

    let placeholders: Vec<String> = package.names().into_iter().filter(|member| {
        if !member.starts_with("assets/") { return false; }
        let filename = member.rsplit('/').next().unwrap_or(member);
        let is_image = filename.to_lowercase().ends_with(".qoi")
            || filename.to_lowercase().ends_with(".png")
            || filename.to_lowercase().ends_with(".webp");
        is_image && !refs.contains(filename)
            && package.get(member).is_some_and(is_no_image_placeholder)
    }).collect();
    for member in placeholders {
        package.remove(&member);
        changed.push(format!("placeholder eliminado: {member}"));
    }

    let Some(source) = pick_source_sprite(package, &refs) else {
        log_line(log, "  aviso: no hay sprites válidos para la carátula");
        return Ok(changed);
    };
    let folder = safe_folder_name(name);
    for filename in ["stand_01.qoi", "idle_01.qoi", "shime1.qoi"] {
        let target = format!("assets/{folder}/{filename}");
        if !target.to_lowercase().contains(&name.to_lowercase()) { continue; }
        if let Some(current) = package.get(&target) {
            if !is_no_image_placeholder(current) { return Ok(changed); }
        }
        let bytes = package.get(&source).map(|d| d.to_vec()).unwrap_or_default();
        package.set(&target, bytes);
        changed.push(format!("carátula creada: {target} (copia de {})", source.rsplit('/').next().unwrap_or(&source)));
        break;
    }
    Ok(changed)
}

fn apply_fixes(package: &mut WlshmArchive, opts: &ConvertOptions, cancel: &Arc<AtomicBool>, log: &dyn Fn(&str)) -> Result<(), String> {
    let selected = opts.fixes.list();
    if selected.is_empty() {
        log_line(log, "[4] Correcciones desactivadas");
        return Ok(());
    }
    log_line(log, &format!("[4] Aplicando correcciones: {}", selected.join(",")));
    for name in selected {
        canceled(cancel)?;
        let changes = match name {
            "scale" => fix_scale(package, opts.height, log)?,
            "anchor" => fix_anchor(package)?,
            "fall" => fix_fall(package)?,
            "embedded" => fix_embedded(package)?,
            "cover" => fix_cover(package, log)?,
            _ => Vec::new(),
        };
        log_line(log, &format!("    {name}: {}", if changes.is_empty() { "sin cambios".to_string() } else { changes.join(", ") }));
    }
    Ok(())
}

/// Lanza la conversión en un hilo cancelable usando el mismo canal de eventos
/// que el runner de `shimejictl`.
pub fn spawn_conversion(opts: ConvertOptions, events: mpsc::Sender<CtlEvent>) -> CtlHandle {
    spawn_task(move |cancel, tx| {
        let collected = std::cell::RefCell::new(String::new());
        let log = |line: &str| {
            let line = if line.ends_with('\n') { line.to_string() } else { format!("{line}\n") };
            collected.borrow_mut().push_str(&line);
            let _ = tx.send(CtlEvent::Output(line));
        };
        log(&format!("$ convert {} -> {}", opts.source.display(), opts.output.display()));
        match run_conversion(&opts, &cancel, &log) {
            Ok(path) => {
                log(&format!("Conversion OK: {}", path.display()));
                CtlOutput { code: 0, output: collected.into_inner() }
            }
            Err(e) => {
                log(&format!("error: {e}"));
                CtlOutput { code: 1, output: collected.into_inner() }
            }
        }
    }, events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;
    use std::io::Write;

    fn png_bytes(width: u32, height: u32, color: (u8, u8, u8, u8)) -> Vec<u8> {
        let mut buf = Vec::new();
        let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(width, height, image::Rgba(color.into())));
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png).unwrap();
        buf
    }

    fn qoi_bytes(width: u32, height: u32, color: (u8, u8, u8, u8)) -> Vec<u8> {
        let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(width, height, image::Rgba(color.into())));
        encode_qoi(&img).unwrap()
    }

    fn make_wlshm(assets: &[(&str, Vec<u8>)], behaviors: Value) -> Vec<u8> {
        let mut header = wlshm_header("Test", "1.0").unwrap();
        let mut tar_buf = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_buf);
            let add = |builder: &mut tar::Builder<&mut Vec<u8>>, name: &str, data: Vec<u8>| {
                let mut h = tar::Header::new_ustar();
                h.set_size(data.len() as u64);
                h.set_mode(0o644);
                h.set_cksum();
                builder.append_data(&mut h, name, Cursor::new(data)).unwrap();
            };
            add(&mut builder, "manifest.json", json!({"name":"Test","display_name":"Test"}).to_string().into_bytes());
            add(&mut builder, "scripts.json", b"{}".to_vec());
            add(&mut builder, "actions.json", json!([]).to_string().into_bytes());
            add(&mut builder, "behaviors.json", behaviors.to_string().into_bytes());
            for (name, data) in assets { add(&mut builder, name, data.clone()); }
            builder.finish().unwrap();
        }
        header.extend_from_slice(&tar_buf);
        header
    }

    #[test]
    fn slugify_and_fix_csv() {
        assert_eq!(slugify("My Cute Shimeji!"), "my-cute-shimeji");
        assert_eq!(slugify("---"), "shimeji");
        let fixes = FixOptions::from_csv("scale, cover").unwrap();
        assert_eq!(fixes.list(), vec!["scale", "cover"]);
        assert!(FixOptions::from_csv("none").unwrap().list().is_empty());
        assert!(FixOptions::from_csv("bogus").is_err());
    }

    #[test]
    fn detect_bundle_and_ee_from_zip() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("bundle.zip");
        let file = fs::File::create(&bundle).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("manifest.json", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.start_file("animation.json", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.finish().unwrap();
        assert_eq!(detect_format(&bundle), InputFormat::Bundle);

        let ee = dir.path().join("ee.zip");
        let file = fs::File::create(&ee).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("conf/actions.xml", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"<a/>").unwrap();
        zip.start_file("img/0.png", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(&png_bytes(2, 2, (255, 255, 255, 255))).unwrap();
        zip.finish().unwrap();
        assert_eq!(detect_format(&ee), InputFormat::Ee);
    }

    #[test]
    fn detects_and_prepares_japanese_xml() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("original.zip");
        let file = fs::File::create(&source).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("original/conf/動作.xml", options).unwrap();
        zip.write_all("\u{feff}<マスコット><動作リスト><動作 名前=\"振り向く\" /></動作リスト></マスコット>".as_bytes()).unwrap();
        zip.start_file("original/conf/行動.xml", options).unwrap();
        zip.write_all("<マスコット><行動リスト><行動 名前=\"落ちる\" 頻度=\"1\" /></行動リスト></マスコット>".as_bytes()).unwrap();
        zip.start_file("original/img/0.png", options).unwrap();
        zip.write_all(&png_bytes(2, 2, (255, 255, 255, 255))).unwrap();
        zip.finish().unwrap();
        assert_eq!(detect_format(&source), InputFormat::Japanese);

        let temp = TempDir::new("shimeji-gui-rs-test-jap").unwrap();
        unzip_to_dir(&source, temp.path()).unwrap();
        let stage = temp.path().join("stage");
        prepare_japanese_instance(&temp.path().join("original"), "miku", &stage, &|_s| {}).unwrap();
        let actions = fs::read_to_string(stage.join("conf/actions.xml")).unwrap();
        let behaviors = fs::read_to_string(stage.join("conf/behaviors.xml")).unwrap();
        assert!(actions.contains("<ActionList>"));
        assert!(actions.contains("Name=\"Look\""));
        assert!(!actions.starts_with('\u{feff}'));
        assert!(behaviors.contains("<BehaviorList>"));
        assert!(stage.join("img/miku/0.png").is_file());
        assert!(stage.join("img/flat").is_dir() == false);
    }

    #[test]
    fn bundle_conversion_with_real_input() {
        let Ok(source) = std::env::var("SHIMEJI_BUNDLE_TEST_ZIP") else { return };
        let output = PathBuf::from(std::env::var("SHIMEJI_BUNDLE_TEST_OUT")
            .unwrap_or_else(|_| "/tmp/opencode/bundle-rust".to_string()));
        let _ = fs::remove_dir_all(&output);
        let opts = ConvertOptions {
            source: source.into(), name: String::new(), output, output_is_file: false, force: true,
            base: std::env::var("SHIMEJI_BUNDLE_TEST_BASE").unwrap_or_else(|_| DEFAULT_BASE.to_string()),
            fixes: if std::env::var_os("SHIMEJI_BUNDLE_TEST_FIX").is_some() {
                FixOptions::all()
            } else {
                FixOptions::default()
            },
            height: 128, fix_only: false, import_after: false,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let result = run_conversion(&opts, &cancel, &|line| println!("{line}")).unwrap();
        assert!(result.is_file());
        assert_eq!(result.extension().and_then(|e| e.to_str()), Some("wlshm"));
    }

    #[test]
    fn japanese_conversion_with_real_input() {
        let Ok(source) = std::env::var("SHIMEJI_JAP_TEST_ZIP") else { return };
        let name = std::env::var("SHIMEJI_JAP_TEST_NAME").unwrap_or_else(|_| "miku".to_string());
        let output = PathBuf::from(std::env::var("SHIMEJI_JAP_TEST_OUT")
            .unwrap_or_else(|_| "/tmp/opencode/jap-rust".to_string()));
        let _ = fs::remove_dir_all(&output);
        let opts = ConvertOptions {
            source: source.into(), name: name.clone(), output, output_is_file: false, force: true,
            base: DEFAULT_BASE.to_string(),
            fixes: if std::env::var_os("SHIMEJI_JAP_TEST_NO_FIX").is_some() {
                FixOptions::default()
            } else {
                FixOptions::all()
            },
            height: 128, fix_only: false, import_after: false,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let result = run_conversion(&opts, &cancel, &|line| println!("{line}")).unwrap();
        assert_eq!(result.file_name().unwrap(), format!("Shimeji.{name}.wlshm").as_str());
    }

    #[test]
    fn fix_fall_and_embedded_are_idempotent() {
        let behaviors = json!({
            "definitions": [
                {"name": "fall", "frequency": 25},
                {"name": "walk", "action": "MoveStand", "frequency": 10},
            ],
            "root_behavior_list": [{"name": "fall", "frequency": 7}],
        });
        let actions = json!([
            {"name": "MoveStand", "type": "Move", "content": []},
        ]);
        let raw = make_wlshm(&[], behaviors);
        let mut package = WlshmArchive::from_bytes(raw).unwrap();
        package.set_json("actions.json", &actions).unwrap();
        fix_fall(&mut package).unwrap();
        fix_embedded(&mut package).unwrap();
        fix_fall(&mut package).unwrap();
        fix_embedded(&mut package).unwrap();
        let behaviors = package.get_json("behaviors.json").unwrap();
        assert_eq!(behaviors["definitions"][0]["frequency"], 0);
        assert_eq!(behaviors["root_behavior_list"][0]["frequency"], 0);
        let actions = package.get_json("actions.json").unwrap();
        assert_eq!(actions.as_array().unwrap().len(), 2);
        assert_eq!(actions[1]["type"], "Sequence");
        assert_eq!(actions[1]["name"], "MoveStand");
        assert_eq!(actions[1]["content"][0]["action_name"], "MoveStandInner");
    }

    #[test]
    fn fix_scale_resizes_sprites_and_anchors() {
        let actions = json!([{
            "name": "Stand", "type": "Embedded", "content": [{
                "frames": [{"image": "shime1.qoi", "image_anchor_x": 10, "image_anchor_y": 40}],
                "hotspots": [{"x": 4, "y": 8, "width": 20, "height": 30}],
            }],
        }]);
        let raw = make_wlshm(&[("assets/shime1.qoi", qoi_bytes(40, 200, (0, 200, 0, 255)))], json!({}));
        let mut package = WlshmArchive::from_bytes(raw).unwrap();
        package.set_json("actions.json", &actions).unwrap();
        let log = |_s: &str| {};
        let changes = fix_scale(&mut package, 128, &log).unwrap();
        assert_eq!(changes.len(), 1);
        let image = decode_qoi(package.get("assets/shime1.qoi").unwrap()).unwrap();
        assert_eq!(image.dimensions(), (26, 128));
        let actions = package.get_json("actions.json").unwrap();
        let frame = &actions[0]["content"][0]["frames"][0];
        assert_eq!(frame["image_anchor_x"], 6);
        assert_eq!(frame["image_anchor_y"], 26);
        assert_eq!(actions[0]["content"][0]["hotspots"][0]["width"], 13);
    }

    #[test]
    fn fix_anchor_aligns_visible_bottom() {
        let mut image = image::RgbaImage::from_pixel(128, 128, image::Rgba([20, 30, 40, 255]));
        for y in 118..128 {
            for x in 0..128 { image.put_pixel(x, y, image::Rgba([0, 0, 0, 0])); }
        }
        let actions = json!([{
            "name": "Stand", "content": [{
                "frames": [{"image": "shime1.qoi", "image_anchor_y": 127}],
            }],
        }]);
        let raw = make_wlshm(&[("assets/shime1.qoi", encode_qoi(&DynamicImage::ImageRgba8(image)).unwrap())], json!({}));
        let mut package = WlshmArchive::from_bytes(raw).unwrap();
        package.set_json("actions.json", &actions).unwrap();
        let first = fix_anchor(&mut package).unwrap();
        assert_eq!(first, vec!["1 frames ajustados"]);
        assert_eq!(fix_anchor(&mut package).unwrap(), Vec::<String>::new());
        let actions = package.get_json("actions.json").unwrap();
        let frame = &actions[0]["content"][0]["frames"][0];
        assert_eq!(frame["image_anchor_y"], 117);
        assert_eq!(frame["wlshm_bottom_trim"], 10);
    }

    #[test]
    fn fix_cover_removes_placeholder_and_creates_cover() {
        let mut placeholder = image::RgbaImage::new(512, 512);
        for y in 200..280 {
            for x in 100..300 { placeholder.put_pixel(x, y, image::Rgba([220, 20, 20, 255])); }
        }
        let mut sprite = image::RgbaImage::new(64, 64);
        for y in 10..50 { for x in 10..40 { sprite.put_pixel(x, y, image::Rgba([30, 120, 200, 255])); } }
        let raw = make_wlshm(&[
            ("assets/shime1.qoi", encode_qoi(&DynamicImage::ImageRgba8(sprite)).unwrap()),
            ("assets/shime2.qoi", encode_qoi(&DynamicImage::ImageRgba8(placeholder)).unwrap()),
        ], json!({}));
        let log = |_s: &str| {};
        let mut package = WlshmArchive::from_bytes(raw).unwrap();
        package.set_json("actions.json", &json!([{
            "content": [{"frames": [{"image": "shime1.qoi"}]}],
        }])).unwrap();
        let changes = fix_cover(&mut package, &log).unwrap();
        assert!(changes.iter().any(|c| c.contains("placeholder eliminado")));
        assert!(changes.iter().any(|c| c.contains("carátula creada")));
        assert!(!package.has("assets/shime2.qoi"));
        assert!(package.has("assets/test/stand_01.qoi"));
        assert_eq!(package.get("assets/test/stand_01.qoi"), package.get("assets/shime1.qoi"));
    }

    #[test]
    fn archive_roundtrip_and_header_are_readable() {
        let raw = make_wlshm(&[("assets/shime1.qoi", qoi_bytes(8, 8, (1, 2, 3, 255)))], json!({}));
        let mut package = WlshmArchive::from_bytes(raw).unwrap();
        package.set("scripts.json", b"{\"x\":1}".to_vec());
        let saved = package.save().unwrap();
        let parsed = crate::core::wlshm::read_wlshm_bytes(saved.clone()).unwrap();
        assert_eq!(parsed.name, "Test");
        assert_eq!(parsed.version, "1.0");
        assert!(parsed.manifest["display_name"] == "Test");

    }
}
