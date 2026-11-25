use serde_json::Value;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

pub const WLPK_MAGIC: &[u8; 4] = b"WLPK";
pub const WLPK_HEADER_SIZE: usize = 512;

#[derive(Debug, Clone)]
pub struct WlshmPackage {
    pub raw: Vec<u8>,
    pub name: String,
    pub version: String,
    pub manifest: Value,
}

pub fn read_wlshm(path: &Path) -> Option<WlshmPackage> {
    let raw = fs::read(path).ok()?;
    read_wlshm_bytes(raw)
}

pub fn read_wlshm_bytes(raw: Vec<u8>) -> Option<WlshmPackage> {
    if raw.len() < WLPK_HEADER_SIZE || &raw[..4] != WLPK_MAGIC {
        return None;
    }
    let name_len = *raw.get(4)? as usize;
    let name_start: usize = 5;
    let name_end = name_start.checked_add(name_len)?;
    let name = String::from_utf8_lossy(raw.get(name_start..name_end)?).to_string();
    let version_pos = name_end;
    let version_len = *raw.get(version_pos)? as usize;
    let version_start = version_pos + 1;
    let version_end = version_start.checked_add(version_len)?;
    let version = String::from_utf8_lossy(raw.get(version_start..version_end)?).to_string();

    let mut manifest = Value::Object(Default::default());
    let tar_bytes = &raw[WLPK_HEADER_SIZE..];
    if let Ok(mut ar) = tar::Archive::new(Cursor::new(tar_bytes)).entries() {
        while let Some(Ok(mut entry)) = ar.next() {
            let path = match entry.path() {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(_) => continue,
            };
            if path == "manifest.json" {
                let mut buf = String::new();
                if entry.read_to_string(&mut buf).is_ok() {
                    if let Ok(v) = serde_json::from_str::<Value>(&buf) {
                        manifest = v;
                    }
                }
                break;
            }
        }
    }

    Some(WlshmPackage { raw, name, version, manifest })
}

pub fn write_renamed_wlshm(source: &Path, dest: &Path, new_name: &str, display_name: Option<&str>) -> Result<(), String> {
    let data = read_wlshm(source).ok_or_else(|| "el archivo no es un paquete .wlshm válido".to_string())?;
    let bytes = write_renamed_wlshm_bytes(&data, new_name, display_name)?;
    fs::write(dest, bytes).map_err(|e| e.to_string())
}

pub fn write_renamed_wlshm_bytes(data: &WlshmPackage, new_name: &str, display_name: Option<&str>) -> Result<Vec<u8>, String> {
    let name_bytes = new_name.as_bytes();
    let version_bytes = data.version.as_bytes();
    if name_bytes.is_empty() || name_bytes.len() > 255 {
        return Err("el nombre interno debe tener entre 1 y 255 bytes".to_string());
    }
    let mut header = Vec::new();
    header.extend_from_slice(WLPK_MAGIC);
    header.push(name_bytes.len() as u8);
    header.extend_from_slice(name_bytes);
    header.push(version_bytes.len() as u8);
    header.extend_from_slice(version_bytes);
    if header.len() > WLPK_HEADER_SIZE {
        return Err("la cabecera WLPK resultant es demasiado larga".to_string());
    }
    header.resize(WLPK_HEADER_SIZE, 0);

    let mut manifest = match &data.manifest {
        Value::Object(m) => m.clone(),
        _ => serde_json::Map::new(),
    };
    manifest.insert("name".to_string(), Value::String(new_name.to_string()));
    if let Some(dn) = display_name {
        manifest.insert("display_name".to_string(), Value::String(dn.to_string()));
    }
    let manifest_bytes = serde_json::to_vec(&Value::Object(manifest)).map_err(|e| e.to_string())?;

    let src_tar = &data.raw[WLPK_HEADER_SIZE..];
    let mut src_ar = tar::Archive::new(Cursor::new(src_tar));
    let mut out_buf: Vec<u8> = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut out_buf);
        let entries = src_ar.entries().map_err(|e| e.to_string())?;
        for entry in entries {
            let mut entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path().map_err(|e| e.to_string())?.to_string_lossy().to_string();
            let mut header = entry.header().clone();
            let mut payload: Vec<u8> = Vec::new();
            if entry.header().entry_type().is_file() {
                entry.read_to_end(&mut payload).map_err(|e| e.to_string())?;
            }
            if path == "manifest.json" {
                payload = manifest_bytes.clone();
            }
            header.set_size(payload.len() as u64);
            header.set_cksum();
            if header.entry_type().is_file() {
                builder.append_data(&mut header, &path, Cursor::new(payload)).map_err(|e| e.to_string())?;
            } else {
                builder.append_data(&mut header, &path, Cursor::new(payload)).map_err(|e| e.to_string())?;
            }
        }
        builder.finish().map_err(|e| e.to_string())?;
    }

    let mut out = header;
    out.extend_from_slice(&out_buf);
    Ok(out)
}

pub fn unique_prototype_name(base: &str, used: &std::collections::BTreeSet<String>) -> String {
    let base = if base.trim().is_empty() { "Shimeji" } else { base.trim() };
    let (stem, number) = parse_suffix(base);
    let mut number = number;
    let mut candidate = if number <= 1 { base.to_string() } else { format!("{stem}_{number}") };
    while used.contains(&candidate) {
        number += 1;
        candidate = format!("{stem}_{number}");
    }
    candidate
}

fn parse_suffix(base: &str) -> (&str, i32) {
    if let Some(pos) = base.rfind('_') {
        let (stem, suf) = base.split_at(pos);
        let suf = &suf[1..];
        if !suf.is_empty() && suf.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(n) = suf.parse::<i32>() {
                let stem = if stem.is_empty() { base } else { stem };
                return (stem, n);
            }
        }
    }
    (base, 1)
}

pub fn renamed_display_name(original: Option<&str>, new_internal: &str, old_internal: &str) -> String {
    let original = original.unwrap_or("").trim();
    let original = if original.is_empty() { old_internal } else { original };
    let suffix = if let Some(s) = new_internal.strip_prefix(old_internal) { s } else { "" };
    if suffix.is_empty() {
        return new_internal.to_string();
    }
    let number = suffix.trim_start_matches('_');
    if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) {
        let (stem, _) = parse_suffix(original);
        let stem = if stem.is_empty() { original } else { stem };
        return format!("{stem}_{number}");
    }
    format!("{original}{suffix}")
}

#[derive(Debug, Clone)]
pub struct ImportNotice {
    pub message: String,
}

pub fn prepare_import_files<F>(
    paths: &[PathBuf],
    workdir: &Path,
    mut existing_names: std::collections::BTreeSet<String>,
    read_pkg: F,
) -> (Vec<PathBuf>, Vec<String>)
where
    F: Fn(&Path) -> Option<WlshmPackage>,
{
    let mut prepared = Vec::new();
    let mut notices = Vec::new();
    for src in paths {
        let data = match read_pkg(src) {
            Some(d) if !d.name.is_empty() => d,
            _ => {
                prepared.push(src.clone());
                continue;
            }
        };
        let old_name = data.name.clone();
        if !existing_names.contains(&old_name) {
            prepared.push(src.clone());
            continue;
        }
        let new_name = unique_prototype_name(&old_name, &existing_names);
        let display = renamed_display_name(
            data.manifest.get("display_name").and_then(|v| v.as_str()),
            &new_name,
            &old_name,
        );
        let safe_name = sanitize_filename(&new_name);
        let mut dest = workdir.join(format!("{safe_name}.wlshm"));
        let mut suffix = 2;
        while dest.exists() && dest.canonicalize().ok() != src.canonicalize().ok() {
            dest = workdir.join(format!("{safe_name}_{suffix}.wlshm"));
            suffix += 1;
        }
        match write_renamed_wlshm(src, &dest, &new_name, Some(&display)) {
            Ok(()) => {
                existing_names.insert(new_name.clone());
                prepared.push(dest);
                notices.push(format!("«{old_name}» ya existía: se importará como «{new_name}»."));
            }
            Err(e) => {
                notices.push(format!("No se pudo renombrar «{old_name}»: {e}"));
                prepared.push(src.clone());
            }
        }
    }
    (prepared, notices)
}

fn sanitize_filename(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    let out = out.trim_matches(|c| c == '.' || c == '_').to_string();
    if out.is_empty() { "shimeji".to_string() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use tempfile::tempdir;

    fn make_wlshm(name: &str, version: &str, manifest: Value, extra_files: &[(&str, &[u8])]) -> Vec<u8> {
        let name_b = name.as_bytes();
        let ver_b = version.as_bytes();
        let mut header = Vec::new();
        header.extend_from_slice(WLPK_MAGIC);
        header.push(name_b.len() as u8);
        header.extend_from_slice(name_b);
        header.push(ver_b.len() as u8);
        header.extend_from_slice(ver_b);
        header.resize(WLPK_HEADER_SIZE, 0);
        let mut tar_buf = Vec::new();
        {
            let mut b = tar::Builder::new(&mut tar_buf);
            let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
            let mut mh = tar::Header::new_ustar();
            mh.set_path("manifest.json").unwrap();
            mh.set_size(manifest_bytes.len() as u64);
            mh.set_mode(0o644);
            mh.set_cksum();
            b.append(&mh, &manifest_bytes[..]).unwrap();
            for (p, data) in extra_files {
                let mut h = tar::Header::new_ustar();
                h.set_path(p).unwrap();
                h.set_size(data.len() as u64);
                h.set_mode(0o644);
                h.set_cksum();
                b.append(&h, *data).unwrap();
            }
            b.finish().unwrap();
        }
        header.extend_from_slice(&tar_buf);
        header
    }

    #[test]
    fn read_wlshm_valid() {
        let manifest = serde_json::json!({"name":"Cat","display_name":"Cat"});
        let bytes = make_wlshm("Cat", "1.0", manifest.clone(), &[("img.png", b"PNG")]);
        let pkg = read_wlshm_bytes(bytes).unwrap();
        assert_eq!(pkg.name, "Cat");
        assert_eq!(pkg.version, "1.0");
        assert_eq!(pkg.manifest["display_name"], "Cat");
    }

    #[test]
    fn read_wlshm_invalid() {
        assert!(read_wlshm_bytes(b"nope".to_vec()).is_none());
        let mut bad = make_wlshm("Cat", "1.0", serde_json::json!({}), &[]);
        bad[0] = b'X';
        assert!(read_wlshm_bytes(bad).is_none());
    }

    #[test]
    fn write_renamed_wlshm_updates_header_and_manifest() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("a.wlshm");
        let dest = dir.path().join("b.wlshm");
        let manifest = serde_json::json!({"name":"Cat","display_name":"Cat","foo":1});
        fs::write(&src, make_wlshm("Cat", "1.0", manifest, &[("img.png", b"PNG")])).unwrap();
        write_renamed_wlshm(&src, &dest, "Cat_2", Some("Cat 2")).unwrap();
        let pkg = read_wlshm(&dest).unwrap();
        assert_eq!(pkg.name, "Cat_2");
        assert_eq!(pkg.version, "1.0");
        assert_eq!(pkg.manifest["name"], "Cat_2");
        assert_eq!(pkg.manifest["display_name"], "Cat 2");
        assert_eq!(pkg.manifest["foo"], 1);
        // extra file preserved
        let mut ar = tar::Archive::new(Cursor::new(&pkg.raw[WLPK_HEADER_SIZE..]));
        let mut found = false;
        for e in ar.entries().unwrap() {
            let mut e = e.unwrap();
            if e.path().unwrap().to_string_lossy() == "img.png" {
                let mut s = Vec::new();
                e.read_to_end(&mut s).unwrap();
                assert_eq!(s, b"PNG");
                found = true;
            }
        }
        assert!(found);
    }

    #[test]
    fn unique_and_display_name() {
        let mut used = BTreeSet::new();
        used.insert("Cat".to_string());
        used.insert("Cat_2".to_string());
        assert_eq!(unique_prototype_name("Cat", &used), "Cat_3");
        assert_eq!(renamed_display_name(Some("Cat"), "Cat_2", "Cat"), "Cat_2");
        assert_eq!(renamed_display_name(Some("My Cat"), "Cat_3", "Cat"), "My Cat_3");
    }

    #[test]
    fn prepare_import_renames_collisions() {
        let dir = tempdir().unwrap();
        let work = dir.path().join("work");
        fs::create_dir_all(&work).unwrap();
        let src = dir.path().join("Cat.wlshm");
        fs::write(&src, make_wlshm("Cat", "1.0", serde_json::json!({"name":"Cat","display_name":"Cat"}), &[])).unwrap();
        let mut used = BTreeSet::new();
        used.insert("Cat".to_string());
        let (prepared, notices) = prepare_import_files(&[src.clone()], &work, used, |p| read_wlshm(p));
        assert_eq!(prepared.len(), 1);
        assert_ne!(prepared[0], src);
        assert!(notices[0].contains("ya existía"));
        let pkg = read_wlshm(&prepared[0]).unwrap();
        assert_eq!(pkg.name, "Cat_2");
    }
}
