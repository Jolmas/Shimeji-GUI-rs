use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub fn parse_config_list(text: &str) -> BTreeMap<String, String> {
    let re = Regex::new(r"\(([^)]+)\)\s*=\s*(.+)$").unwrap();
    let mut vals = BTreeMap::new();
    for line in text.lines() {
        if let Some(c) = re.captures(line) {
            vals.insert(c[1].trim().to_string(), c[2].trim().to_string());
        }
    }
    vals
}

pub fn parse_bool_value(vals: &BTreeMap<String, String>, key: &str, default: bool) -> bool {
    match vals.get(key) {
        Some(v) => matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        None => default,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentInfo {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub pos: String,
    pub size: String,
    pub scale: String,
    pub mascots: i32,
}

pub fn parse_environments(text: &str) -> Vec<EnvironmentInfo> {
    let re = Regex::new(
        r"Environment\(\s*id=(?P<id>\d+)(?:,\s*name=(?P<name>[^,]*))?(?:,\s*description=(?P<desc>[^,]*))?(?:,\s*pos=\((?P<x>-?\d+),(?P<y>-?\d+)\))?(?:,\s*size=\((?P<w>\d+),(?P<h>\d+)\))?(?:,\s*scale=(?P<scale>[\d.]+))?(?:,\s*with\s+(?P<mascots>\d+)\s+mascots\))?",
    ).unwrap();
    let mut envs = Vec::new();
    for c in re.captures_iter(text) {
        let id = c.name("id").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        let name = c.name("name").map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        let description = c.name("desc").map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        let x = c.name("x").map(|m| m.as_str()).unwrap_or("0");
        let y = c.name("y").map(|m| m.as_str()).unwrap_or("0");
        let w = c.name("w").map(|m| m.as_str()).unwrap_or("?");
        let h = c.name("h").map(|m| m.as_str()).unwrap_or("?");
        let scale = c.name("scale")
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .map(|v| format!("{v:.2}"))
            .unwrap_or_else(|| "1.00".to_string());
        let mascots = c.name("mascots").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        envs.push(EnvironmentInfo {
            id, name, description,
            pos: format!("({x}, {y})"),
            size: format!("{w} x {h}"),
            scale, mascots,
        });
    }
    envs
}

pub fn prototype_names(text: &str) -> Vec<String> {
    let re_num = Regex::new(r"^[\\|`\-+*>\s]*\d+[.):]\s*").unwrap();
    let re_prefix = Regex::new(r"^[\\|`\-+*>]\s*").unwrap();
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for line in text.lines() {
        let mut s = line.trim().to_string();
        if s.is_empty() { continue; }
        s = re_num.replace(&s, "").to_string();
        s = re_prefix.replace(&s, "").to_string();
        let low = s.to_lowercase();
        if s.is_empty() { continue; }
        let bad = ["prototype", "available", "object id", "end", "id:", "author:", "version:"]
            .iter().any(|x| low.contains(x));
        if !bad && seen.insert(s.clone()) {
            out.push(s);
        }
    }
    out
}

pub fn parse_section(text: &str, heading: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut seen = BTreeSet::new();
    let mut active = false;
    let heading_colon = format!("{heading}:");
    let re_field = Regex::new(r"^[A-Za-z][A-Za-z _-]*:\s*").unwrap();
    let re_item = Regex::new(r"^[-*]\s+(.+?)\s*$").unwrap();
    for raw in text.lines() {
        let line = raw.trim();
        if line == heading_colon {
            active = true;
            continue;
        }
        if !active { continue; }
        if re_field.is_match(line) || line.contains("Object id:") || line.contains("[END]") {
            break;
        }
        if let Some(c) = re_item.captures(line) {
            let v = c[1].trim().to_string();
            if seen.insert(v.clone()) {
                values.push(v);
            }
        }
    }
    values
}

pub fn prototype_store_path(info: &str) -> Option<String> {
    let re = Regex::new(r"(?m)^\s*Prototype store path:\s*(.+?)\s*$").unwrap();
    re.captures(info).map(|c| c[1].trim().to_string())
}

pub fn filter_behaviors(raw_behaviors: &[String], defs: &[Value]) -> Vec<String> {
    let mut defs_map: BTreeMap<String, &Value> = BTreeMap::new();
    for d in defs {
        if let Some(name) = d.get("name").and_then(|v| v.as_str()) {
            defs_map.insert(name.to_string(), d);
        }
    }
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for b in raw_behaviors {
        if b.is_empty() { continue; }
        let low = b.to_lowercase();
        if b.starts_with('_') || low.contains("condition") || low == "leavewall" {
            continue;
        }
        if let Some(def) = defs_map.get(b) {
            let hidden = def.get("hidden").and_then(|v| v.as_bool()).unwrap_or(false);
            let is_cond = def.get("is_conditioner").and_then(|v| v.as_bool()).unwrap_or(false);
            if hidden || is_cond { continue; }
        }
        if seen.insert(b.clone()) {
            out.push(b.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_config_list_extracts_keys() {
        let text = "- Multiplication (BREEDING) = true\n- Layer (WLR_SHELL_LAYER) = overlay\n";
        let vals = parse_config_list(text);
        assert_eq!(vals.get("BREEDING").map(String::as_str), Some("true"));
        assert_eq!(vals.get("WLR_SHELL_LAYER").map(String::as_str), Some("overlay"));
        assert!(parse_bool_value(&vals, "BREEDING", false));
        assert!(!parse_bool_value(&vals, "MISSING", false));
    }

    #[test]
    fn parse_environments_line() {
        let text = "Environment( id=1, name=DP-1, description=desc, pos=(0,0), size=(1920,1080), scale=1.0, with 3 mascots)";
        let envs = parse_environments(text);
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].id, 1);
        assert_eq!(envs[0].name, "DP-1");
        assert_eq!(envs[0].size, "1920 x 1080");
        assert_eq!(envs[0].mascots, 3);
        assert_eq!(envs[0].scale, "1.00");
    }

    #[test]
    fn prototype_names_cleans_output() {
        let text = "Available prototypes:\n  1. Cat\n- Dog\n  | 2) Bird\n  Object id: x\n";
        assert_eq!(prototype_names(text), vec!["Cat", "Dog", "Bird"]);
    }

    #[test]
    fn parse_section_and_store_path() {
        let info = "Name: Cat\nBehaviors:\n- Idle\n- Walk\nAffordances:\n- Sit\nPrototype store path: /tmp/Cat\n[END]\n";
        assert_eq!(parse_section(info, "Behaviors"), vec!["Idle", "Walk"]);
        assert_eq!(prototype_store_path(info).as_deref(), Some("/tmp/Cat"));
    }

    #[test]
    fn filter_behaviors_removes_internal_and_hidden() {
        let defs = vec![json!({"name":"Hidden","hidden":true}), json!({"name":"Cond","is_conditioner":true}), json!({"name":"Idle"})];
        let raw = vec!["Idle".into(), "___CONDITION_0".into(), "LeaveWall".into(), "Hidden".into(), "Cond".into(), "Walk".into()];
        assert_eq!(filter_behaviors(&raw, &defs), vec!["Idle", "Walk"]);
    }
}
