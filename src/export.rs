//! Text exports: Internet Updater files, custom logs, reports and playlister files.
use crate::db::HistRow;
use crate::model::{Settings, Track};
use std::fs;
use std::path::{Path, PathBuf};

pub fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn csv_esc(s: &str) -> String {
    if s.contains(';') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn fmt_hms(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

fn resolve(base: &Path, dir: &str) -> PathBuf {
    let p = Path::new(dir);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

/// Message shown as metadata for a library kind: None = do not update, Some(custom or normal).
pub fn meta_for(s: &Settings, t: &Track) -> Option<(String, String)> {
    let kind = if t.kind.is_empty() { "music" } else { t.kind.as_str() };
    for (k, msg) in &s.msg_kinds {
        if k == kind {
            if msg == "NO-UPDATE" {
                return None;
            }
            if !msg.is_empty() {
                return Some((String::new(), msg.clone()));
            }
        }
    }
    Some((t.artist.clone(), t.title.clone()))
}

/// Writes NowOnAir.txt/.xml, AirPlayNext.xml, AirPlayHistory.xml and CurrentAlbumImage.jpg.
pub fn now_on_air(base: &Path, s: &Settings, cur: &Track, next: &[Track], history: &[(String, String)]) {
    let dir = resolve(base, &s.export_dir);
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Some((artist, title)) = meta_for(s, cur) {
        let txt = if artist.is_empty() { title.clone() } else { format!("{artist} - {title}") };
        let _ = fs::write(dir.join("NowOnAir.txt"), txt);
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<NowOnAir><Artist>{}</Artist><Title>{}</Title><Album>{}</Album><Type>{}</Type></NowOnAir>\n",
            xml_esc(&artist),
            xml_esc(&title),
            xml_esc(&cur.album),
            xml_esc(&cur.kind)
        );
        let _ = fs::write(dir.join("NowOnAir.xml"), xml);
    }
    if s.export_next {
        let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<AirPlayNext>\n");
        for t in next.iter().filter(|t| t.kind == "music").take(5) {
            x.push_str(&format!("  <Track><Artist>{}</Artist><Title>{}</Title></Track>\n", xml_esc(&t.artist), xml_esc(&t.title)));
        }
        x.push_str("</AirPlayNext>\n");
        let _ = fs::write(dir.join("AirPlayNext.xml"), x);
    }
    if s.export_history {
        let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<AirPlayHistory>\n");
        for (tm, label) in history.iter().take(10) {
            x.push_str(&format!("  <Item time=\"{}\">{}</Item>\n", xml_esc(tm), xml_esc(label)));
        }
        x.push_str("</AirPlayHistory>\n");
        let _ = fs::write(dir.join("AirPlayHistory.xml"), x);
    }
    if s.export_art {
        if let Some(img) = crate::library::cover_art(&cur.path) {
            let _ = fs::write(dir.join("CurrentAlbumImage.jpg"), img);
        }
    }
}

fn apply_template(tpl: &str, date: &str, time: &str, t: &Track) -> String {
    tpl.replace("{date}", date)
        .replace("{time}", time)
        .replace("{kind}", &t.kind)
        .replace("{artist}", &t.artist)
        .replace("{title}", &t.title)
        .replace("{album}", &t.album)
        .replace("{code}", &t.code())
        .replace("{duration}", &fmt_hms(t.duration_ms))
}

/// Appends one line to the daily custom log text file.
pub fn custom_log(base: &Path, s: &Settings, t: &Track) {
    if !s.custom_log {
        return;
    }
    let dir = resolve(base, &s.custom_log_path);
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let now = chrono::Local::now();
    let line = apply_template(&s.custom_log_format, &now.format("%Y-%m-%d").to_string(), &now.format("%H:%M:%S").to_string(), t);
    let file = dir.join(format!("{}.log", now.format("%Y%m%d")));
    use std::io::Write;
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{line}");
    }
}

pub fn reports_dir(base: &Path) -> PathBuf {
    let d = base.join("reports");
    let _ = fs::create_dir_all(&d);
    d
}

pub fn write_lines(base: &Path, name: &str, lines: &[String]) -> Result<PathBuf, String> {
    let p = reports_dir(base).join(name);
    fs::write(&p, lines.join("\r\n")).map_err(|e| e.to_string())?;
    Ok(p)
}

/// Broadcasting report for copyright authorities (one line per played item).
pub fn broadcast_report(rows: &[HistRow], tracks: &[Track], s: &Settings, csv: bool) -> Vec<String> {
    let mut out = Vec::new();
    let sep = if csv { ";" } else { "\t" };
    if s.report_style == "custom" {
        for r in rows {
            let mut t = tracks.iter().find(|t| t.id == r.track_id).cloned().unwrap_or_default();
            t.artist = r.artist.clone();
            t.title = r.title.clone();
            let (d, tm) = r.at.split_at(10.min(r.at.len()));
            out.push(apply_template(&s.report_template, d, tm.trim(), &t));
        }
        return out;
    }
    out.push(["Date", "Time", "Artist", "Title", "Type", "Duration"].join(sep));
    for r in rows.iter().rev() {
        let dur = tracks.iter().find(|t| t.id == r.track_id).map(|t| fmt_hms(t.duration_ms)).unwrap_or_default();
        let (d, tm) = r.at.split_at(10.min(r.at.len()));
        let cols = [d.to_string(), tm.trim().to_string(), r.artist.clone(), r.title.clone(), r.kind.clone(), dur];
        let cols: Vec<String> = if csv { cols.iter().map(|c| csv_esc(c)).collect() } else { cols.to_vec() };
        out.push(cols.join(sep));
    }
    out
}

/// "Export to playlister": one line per track with its code.
pub fn playlister(tracks: &[&Track]) -> Vec<String> {
    tracks
        .iter()
        .map(|t| format!("{};{};{};{}", t.code(), csv_esc(&t.artist), csv_esc(&t.title), t.duration_ms / 1000))
        .collect()
}
