//! Database backups (one compressed .rmbak file) and restore.
use crate::db::{Db, SCHEMA_VERSION};
use crate::model::Track;
use anyhow::{bail, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Backup {
    pub format: u32,
    pub app_version: String,
    pub schema: i32,
    pub created: String,
    pub tracks: Vec<Track>,
    pub docs: Vec<(u64, String, String)>,
    pub settings: Vec<(String, String)>,
    pub history: Vec<(u64, u64, String, String, String)>,
    pub log: Vec<u64>,
}

pub fn export(db: &Db, path: &Path, app_version: &str) -> Result<()> {
    let b = Backup {
        format: 1,
        app_version: app_version.to_string(),
        schema: SCHEMA_VERSION,
        created: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        tracks: db.list_tracks(None)?,
        docs: db.all_docs()?,
        settings: db.all_settings()?,
        history: db.all_history()?,
        log: db.log_ids()?,
    };
    let f = File::create(path)?;
    let mut enc = GzEncoder::new(f, Compression::default());
    enc.write_all(serde_json::to_string(&b)?.as_bytes())?;
    enc.finish()?;
    Ok(())
}

/// Replaces the whole database with the content of a backup file. A safety copy of the current
/// data is written first. Returns a message for the user.
pub fn import(db: &Db, path: &Path, app_version: &str, backup_dir: &Path) -> Result<String> {
    let mut text = String::new();
    GzDecoder::new(File::open(path)?).read_to_string(&mut text)?;
    let b: Backup = serde_json::from_str(&text)?;
    if b.format != 1 {
        bail!("This file is not a RadioMoon backup.");
    }
    if b.schema > SCHEMA_VERSION {
        bail!("This backup was made by a newer version of RadioMoon ({}). Install the newer version first.", b.app_version);
    }
    let _ = std::fs::create_dir_all(backup_dir);
    let _ = export(db, &backup_dir.join("before-restore.rmbak"), app_version);
    db.wipe_all()?;
    for t in &b.tracks {
        db.restore_track(t)?;
    }
    db.restore_docs(&b.docs)?;
    let st: Vec<(String, String)> = b
        .settings
        .iter()
        .filter(|(k, _)| k != "app_version" && k != "schema_version" && k != "previous_app_version")
        .cloned()
        .collect();
    db.restore_settings(&st)?;
    db.restore_history(&b.history)?;
    db.save_log(&b.log)?;
    db.set_setting("schema_version", &b.schema.to_string())?;
    db.set_setting("app_version", &b.app_version)?;
    let msg = db.ensure_schema(app_version, backup_dir)?;
    Ok(format!("Restored {} tracks from {} (made {}). {msg}", b.tracks.len(), b.app_version, b.created))
}

/// Backup files of a folder: (file name, size in bytes, modified time as text), newest first.
pub fn list(dir: &Path) -> Vec<(String, u64, String)> {
    let mut v: Vec<(String, u64, std::time::SystemTime)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map(|x| x == "rmbak").unwrap_or(false) {
                if let Ok(m) = e.metadata() {
                    v.push((p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)));
                }
            }
        }
    }
    v.sort_by(|a, b| b.2.cmp(&a.2));
    v.into_iter()
        .map(|(n, s, t)| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            (n, s, dt.format("%Y-%m-%d %H:%M").to_string())
        })
        .collect()
}

/// Keeps only the newest `keep` automatic backups.
pub fn prune(dir: &Path, keep: usize) {
    let autos: Vec<String> = list(dir).into_iter().map(|x| x.0).filter(|n| n.starts_with("radiomoon-auto-")).collect();
    for n in autos.into_iter().skip(keep.max(1)) {
        let _ = std::fs::remove_file(dir.join(n));
    }
}
