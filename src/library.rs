use crate::db::Db;
use crate::model::Track;
use lofty::prelude::*;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

pub const EXTS: [&str; 7] = ["mp3", "flac", "wav", "ogg", "m4a", "aac", "opus"];

#[derive(Clone)]
pub struct ImportOpts {
    pub kind: String,
    pub cat1: String,
    pub autocue: bool,
    pub autocue_db: f32,
    pub bookmark: bool,
    pub gain_db: f32,
    pub read_tags: bool,
    pub parse_filename: bool,
}

pub struct Meta {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: String,
    pub dur: u64,
}

pub fn read_meta(p: &Path, read_tags: bool) -> Meta {
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut m = Meta { title: stem.clone(), artist: String::new(), album: String::new(), genre: String::new(), year: String::new(), dur: 0 };
    if let Ok(tf) = lofty::read_from_path(p) {
        m.dur = tf.properties().duration().as_millis() as u64;
        if read_tags {
            if let Some(tag) = tf.primary_tag().or_else(|| tf.first_tag()) {
                if let Some(t) = tag.title().map(|s| s.to_string()).filter(|s| !s.is_empty()) {
                    m.title = t;
                }
                m.artist = tag.artist().map(|s| s.to_string()).unwrap_or_default();
                m.album = tag.album().map(|s| s.to_string()).unwrap_or_default();
                m.genre = tag.genre().map(|s| s.to_string()).unwrap_or_default();
                m.year = tag.year().map(|y| y.to_string()).unwrap_or_default();
            }
        }
    }
    m
}

/// Embedded cover art of an audio file (raw image bytes).
pub fn cover_art(path: &str) -> Option<Vec<u8>> {
    let tf = lofty::read_from_path(path).ok()?;
    let tag = tf.primary_tag().or_else(|| tf.first_tag())?;
    tag.pictures().first().map(|p| p.data().to_vec())
}

pub fn build_track(p: &Path, o: &ImportOpts) -> Track {
    let m = read_meta(p, o.read_tags);
    let mut t = Track {
        kind: o.kind.clone(),
        title: m.title,
        artist: m.artist,
        album: m.album,
        genre: m.genre,
        year: m.year,
        path: p.to_string_lossy().to_string(),
        duration_ms: m.dur,
        cat1: o.cat1.clone(),
        enabled: true,
        bookmarked: o.bookmark,
        gain_db: o.gain_db,
        ..Default::default()
    };
    if !o.read_tags && o.parse_filename {
        if let Some((a, b)) = t.title.clone().split_once(" - ") {
            t.artist = a.trim().to_string();
            t.title = b.trim().to_string();
        }
    }
    if o.autocue {
        if let Some((a, b, d)) = crate::waveform::autocue(&t.path, o.autocue_db) {
            t.mix_in_ms = a;
            t.mix_out_ms = b;
            if d > 0 {
                t.duration_ms = d;
            }
        }
    }
    t
}

pub fn import_one(db: &Db, p: &Path, o: &ImportOpts) -> bool {
    db.insert_track(&build_track(p, o)).is_ok()
}

/// Imports every audio file in a folder. Files whose file name is already in the library are skipped.
pub fn scan(db: &Db, dir: &Path, o: &ImportOpts, status: &Arc<Mutex<String>>) -> usize {
    let have: HashSet<String> = db
        .list_tracks(None)
        .unwrap_or_default()
        .iter()
        .filter_map(|t| Path::new(&t.path).file_name().map(|f| f.to_string_lossy().to_lowercase()))
        .collect();
    let mut n = 0;
    for e in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if !e.file_type().is_file() {
            continue;
        }
        let p = e.path();
        let ext_ok = p.extension().map(|x| EXTS.contains(&x.to_string_lossy().to_lowercase().as_str())).unwrap_or(false);
        if !ext_ok {
            continue;
        }
        let fname = p.file_name().map(|f| f.to_string_lossy().to_lowercase()).unwrap_or_default();
        if have.contains(&fname) {
            continue;
        }
        if import_one(db, p, o) {
            n += 1;
            if n % 10 == 0 {
                if let Ok(mut s) = status.lock() {
                    *s = format!("Importing... {n} files");
                }
            }
        }
    }
    n
}
