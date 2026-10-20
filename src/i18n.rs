//! Translations. The English text of the interface is the key; a language file maps it to another language.
//!
//! Language files are JSON objects: {"_name": "Ελληνικά", "Control Panel": "Πίνακας ελέγχου", ...}.
//! Put new files (for example `de.json`) in the `lang` folder next to the program or in the data folder.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

const EL: &str = include_str!("../lang/el.json");

fn table() -> &'static RwLock<HashMap<String, String>> {
    static T: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();
    T.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn tr(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    match table().read() {
        Ok(t) => t.get(s).cloned().unwrap_or_else(|| s.to_string()),
        Err(_) => s.to_string(),
    }
}

#[allow(dead_code)]
/// Translate a text with `{}` placeholders and fill them in order.
pub fn trf(tpl: &str, args: &[String]) -> String {
    let mut out = tr(tpl);
    for a in args {
        if let Some(i) = out.find("{}") {
            out.replace_range(i..i + 2, a);
        }
    }
    out
}

fn parse(text: &str) -> HashMap<String, String> {
    serde_json::from_str::<HashMap<String, serde_json::Value>>(text)
        .map(|m| m.into_iter().filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string()))).collect())
        .unwrap_or_default()
}

fn file_for(code: &str, dirs: &[PathBuf]) -> Option<String> {
    for d in dirs {
        if let Ok(s) = std::fs::read_to_string(d.join("lang").join(format!("{code}.json"))) {
            return Some(s);
        }
    }
    None
}

pub fn set_language(code: &str, dirs: &[PathBuf]) {
    let map = if code == "en" || code.is_empty() {
        HashMap::new()
    } else if let Some(s) = file_for(code, dirs) {
        parse(&s)
    } else if code == "el" {
        parse(EL)
    } else {
        HashMap::new()
    };
    if let Ok(mut t) = table().write() {
        *t = map;
    }
}

/// All languages: (code, name). English and Greek are built in; any other `*.json` file is added.
pub fn available(dirs: &[PathBuf]) -> Vec<(String, String)> {
    let mut v = vec![("en".to_string(), "English".to_string()), ("el".to_string(), "Ελληνικά".to_string())];
    for d in dirs {
        if let Ok(rd) = std::fs::read_dir(Path::new(d).join("lang")) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().map(|x| x == "json").unwrap_or(false) {
                    let code = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    if code.is_empty() || code == "template" || v.iter().any(|x| x.0 == code) {
                        continue;
                    }
                    let name = std::fs::read_to_string(&p).ok().map(|s| parse(&s)).and_then(|m| m.get("_name").cloned()).unwrap_or_else(|| code.clone());
                    v.push((code, name));
                }
            }
        }
    }
    v
}
