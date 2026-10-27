//! Application state and studio logic (playout, auto fill, permissions, data loading).
use crate::config::Config;
use crate::control::ControlState;
use crate::db::Db;
use crate::dbserver::DbServer;
use crate::engine::{self, Data, Generator};
use crate::backup;
use crate::export;
use crate::i18n::tr;
use crate::stream::Streamer;
use crate::model::*;
use crate::player::Player;
use crate::waveform;
use chrono::{Datelike, Duration as CDur, Local, NaiveDateTime, Timelike};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Auto,
    Manual,
    Offline,
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD: &str = env!("RM_GIT");

/// Set before App::new by the client window: the program does not play (the server does) and must not touch the playlist.
pub static REMOTE_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn hash_pw(salt: &str, pw: &str) -> String {
    let mut h = Sha256::new();
    h.update(salt.as_bytes());
    h.update(pw.as_bytes());
    format!("{:x}", h.finalize())
}

pub fn fmt_ms(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Durations are always shown as minutes:seconds.
pub fn fmt_dur(ms: u64) -> String {
    fmt_ms(ms)
}

/// Countdown with tenths: 3:07.4
pub fn fmt_count(ms: u64) -> String {
    let t = ms / 100;
    format!("{}:{:02}.{}", t / 600, (t / 10) % 60, t % 10)
}

/// Editable time with tenths: 3:07.4
pub fn fmt_precise(ms: u64) -> String {
    fmt_count(ms)
}

/// Accepts 3:07, 3:07.4, 187, 187.4 or 1:03:07.
pub fn parse_time(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return Some(0);
    }
    let parts: Vec<&str> = s.split(':').collect();
    let mut secs = 0f64;
    for p in &parts {
        secs = secs * 60.0 + p.trim().replace(',', ".").parse::<f64>().ok()?;
    }
    if secs < 0.0 {
        return None;
    }
    Some((secs * 1000.0).round() as u64)
}

pub fn find_audio(dir: &str, stem: &str) -> Option<String> {
    for ext in crate::library::EXTS {
        let p = Path::new(dir).join(format!("{stem}.{ext}"));
        if p.exists() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    None
}

pub struct App {
    pub server: DbServer,
    pub db: Db,
    pub cfg: Config,
    pub base: PathBuf,
    pub data: Data,
    pub player: Option<Player>,
    pub remote: Option<crate::remote::Remote>,
    pub status: String,
    pub log: Vec<Track>,
    pub current: Option<usize>,
    pub sel: Option<usize>,
    pub scroll_to: Option<usize>,
    pub mode: Mode,
    pub gen: Option<Generator>,
    pub final_loaded: String,
    pub last_fill: Instant,
    /// Counts the reloads of the library and the documents: the clients read them again when this number changes.
    pub data_rev: u64,
    pub stop_next: bool,
    pub loop_current: bool,
    pub lib_kind: &'static str,
    pub lib_sel: Option<u64>,
    pub filter: String,
    pub super_search: bool,
    pub scan_status: Arc<Mutex<String>>,
    pub scan_done: Arc<AtomicBool>,
    pub pallet_idx: usize,
    pub page: usize,
    pub jingle_now: Option<(u64, String, Instant, u64)>,
    pub wave: waveform::WaveStore,
    pub history: Vec<(String, String)>,
    pub preview_name: String,
    pub preview_dur: u64,
    pub user: Option<User>,
    pub login_name: String,
    pub login_pass: String,
    pub login_err: String,
    pub last_activity: Instant,
    pub cp_open: bool,
    pub cp: ControlState,
    pub app_dir: PathBuf,
    /// Running internet streams: (index in settings, tap id, streamer).
    pub streamers: Vec<(usize, u32, Streamer)>,
    pub autoplay_pending: bool,
    pub about_open: bool,
    pub about_tex: Option<eframe::egui::TextureHandle>,
    pub last_backup_day: String,
    pub last_backup_check: Instant,
    /// live rebroadcast: selected source, the schedule slot that started it, a slot switched off by hand
    pub live_sel: usize,
    pub live_slot: Option<usize>,
    pub live_dismissed: Option<usize>,
    /// mode before a live source stopped the automation
    pub live_prev_mode: Option<Mode>,
    pub last_live_check: Instant,
}

impl App {
    pub fn new(server: DbServer, db: Db, cfg: Config, base: PathBuf, app_dir: PathBuf, upgrade_msg: String) -> Self {
        let mut app = App {
            server,
            db,
            cfg,
            base,
            data: Data::default(),
            player: None,
            remote: None,
            status: String::from("Ready"),
            log: vec![],
            current: None,
            sel: None,
            scroll_to: None,
            mode: Mode::Manual,
            gen: None,
            final_loaded: String::new(),
            last_fill: Instant::now(),
            data_rev: 0,
            stop_next: false,
            loop_current: false,
            lib_kind: "music",
            lib_sel: None,
            filter: String::new(),
            super_search: true,
            scan_status: Arc::new(Mutex::new(String::new())),
            scan_done: Arc::new(AtomicBool::new(false)),
            pallet_idx: 0,
            page: 0,
            jingle_now: None,
            wave: waveform::new_store(),
            history: vec![],
            preview_name: String::new(),
            preview_dur: 0,
            user: None,
            login_name: String::new(),
            login_pass: String::new(),
            login_err: String::new(),
            last_activity: Instant::now(),
            cp_open: false,
            cp: ControlState::default(),
            app_dir,
            streamers: Vec::new(),
            autoplay_pending: false,
            about_open: false,
            about_tex: None,
            last_backup_day: String::new(),
            last_backup_check: Instant::now(),
            live_sel: 0,
            live_slot: None,
            live_dismissed: None,
            live_prev_mode: None,
            last_live_check: Instant::now(),
        };
        app.reload_all();
        crate::i18n::set_language(&app.data.s.language.clone(), &app.lang_dirs());
        app.last_backup_day = app.db.get_setting("last_backup_day").ok().flatten().unwrap_or_default();
        if !upgrade_msg.is_empty() {
            app.status = upgrade_msg;
        }
        let remote = REMOTE_MODE.load(std::sync::atomic::Ordering::SeqCst);
        if !remote {
            app.apply_outputs();
        }
        app.log = app.db.load_log().unwrap_or_default();
        app.history = app.db.history(40).unwrap_or_default();
        if app.data.s.start_mode == "auto" && !remote {
            app.mode = Mode::Auto;
            app.autoplay_pending = true;
            // the playlist of the last run is not replayed: the schedule builds a new one (different songs at every start)
            app.log.clear();
            app.save_log();
        }
        if app.data.s.streams.is_empty() {
            let first = app.data.s.stream.clone(); // settings of older versions had a single server
            app.data.s.streams.push(first);
            app.save_settings();
        }
        if !remote && app.data.s.streams.iter().any(|c| c.enabled && c.autostart) {
            app.start_stream();
        }
        app
    }

    // ---------- permissions ----------
    pub fn needs_login(&self) -> bool {
        self.data.s.users_enabled && !self.data.users.is_empty() && self.user.is_none()
    }

    pub fn can(&self, perm: &str) -> bool {
        if !self.data.s.users_enabled || self.data.users.is_empty() {
            return true;
        }
        self.user.as_ref().map(|u| u.admin || u.perms.iter().any(|p| p == perm)).unwrap_or(false)
    }

    pub fn login(&mut self) {
        let name = self.login_name.trim().to_lowercase();
        let found = self.data.users.iter().find(|u| u.name.to_lowercase() == name).cloned();
        match found {
            Some(u) if hash_pw(&u.salt, &self.login_pass) == u.hash => {
                self.user = Some(u);
                self.login_err.clear();
                self.login_pass.clear();
                self.last_activity = Instant::now();
            }
            _ => self.login_err = "Wrong user name or password".into(),
        }
    }

    pub fn logout(&mut self) {
        self.user = None;
        self.cp_open = false;
    }

    // ---------- data ----------
    /// Deletes items from the library, and their audio files too when asked (with a server the server does it). Returns the message.
    pub fn delete_library_items(&mut self, ids: &[u64], files: bool) -> String {
        if let Some(r) = &self.remote {
            return match crate::remote::post_json(&r.link, &serde_json::json!({ "do": "delete_tracks", "ids": ids, "files": files })) {
                Ok(v) => {
                    self.reload_tracks();
                    v["message"].as_str().unwrap_or("Deleted").to_string()
                }
                Err(e) => e,
            };
        }
        let paths: Vec<String> = ids.iter().filter_map(|id| self.data.track(*id).map(|t| t.path.clone())).collect();
        self.stop();
        if self.db.delete_tracks(ids).is_err() {
            return "The items could not be deleted".into();
        }
        let (mut gone, mut failed) = (0, 0);
        if files {
            for p in &paths {
                // a file that is still used by another item of the library stays
                if p.is_empty() || self.data.tracks.iter().any(|t| t.path == *p && !ids.contains(&t.id)) {
                    continue;
                }
                let f = self.map_path(p);
                if std::path::Path::new(&f).is_file() {
                    if std::fs::remove_file(&f).is_ok() {
                        gone += 1;
                    } else {
                        failed += 1;
                    }
                }
            }
        }
        self.reload_tracks();
        self.log = self.db.load_log().unwrap_or_default();
        if files {
            format!("{} items deleted, {gone} files deleted{}", ids.len(), if failed > 0 { format!(", {failed} files could not be deleted") } else { String::new() })
        } else {
            format!("{} items deleted from the library", ids.len())
        }
    }

    pub fn reload_tracks(&mut self) {
        self.data_rev += 1;
        self.data.tracks = self.db.list_tracks(None).unwrap_or_default();
        self.data.reindex();
    }

    pub fn reload_docs(&mut self) {
        self.data_rev += 1;
        let db = &self.db;
        self.data.cats = db.docs("category").unwrap_or_default();
        self.data.clocks = db.docs("clock").unwrap_or_default();
        self.data.force = db.docs("force").unwrap_or_default();
        self.data.playlists = db.docs("playlist").unwrap_or_default();
        self.data.campaigns = db.docs("campaign").unwrap_or_default();
        self.data.breaks = db.docs("break").unwrap_or_default();
        self.data.bpl = db.docs("bpl").unwrap_or_default();
        self.data.finals = db.docs("finallog").unwrap_or_default();
        self.data.memos = db.docs("memo").unwrap_or_default();
        self.data.pallets = db.docs("pallet").unwrap_or_default();
        self.data.users = db.docs("user").unwrap_or_default();
        if self.data.breaks.is_empty() {
            for h in 0..24 {
                for m in [0, 30] {
                    let mut b = BreakDef { time: format!("{h:02}:{m:02}"), name: String::new(), immediate_mask: 0, visible: true, ..Default::default() };
                    let _ = self.db.save_doc("break", &mut b);
                }
            }
            self.data.breaks = self.db.docs("break").unwrap_or_default();
        }
        if self.data.pallets.is_empty() {
            let mut p = Pallet { name: "Default".into(), pages: 1, rows: 4, cols: 4, buttons: vec![], ..Default::default() };
            p.buttons = vec![PBtn::default(); 16];
            let _ = self.db.save_doc("pallet", &mut p);
            self.data.pallets = self.db.docs("pallet").unwrap_or_default();
        }
    }

    pub fn reload_all(&mut self) {
        self.data.s = self.db.load_settings();
        self.reload_docs();
        self.reload_tracks();
    }

    pub fn save_settings(&mut self) {
        self.db.save_settings(&self.data.s);
    }

    pub fn save_doc<T: Serialize + HasId>(&mut self, kind: &str, x: &mut T) {
        if let Err(e) = self.db.save_doc(kind, x) {
            self.status = format!("Save error: {e}");
        }
        self.reload_docs();
    }

    pub fn del_doc(&mut self, id: u64) {
        let _ = self.db.del_doc(id);
        self.reload_docs();
    }

    pub fn lang_dirs(&self) -> Vec<PathBuf> {
        vec![self.app_dir.clone(), self.base.clone()]
    }

    pub fn backup_dir(&self) -> PathBuf {
        let d = Path::new(&self.data.s.backup.dir);
        if d.is_absolute() {
            d.to_path_buf()
        } else {
            self.base.join(d)
        }
    }

    /// Starts every enabled internet server (each one has its own encoder and connection).
    /// Entering AUTO mode starts the first song of the log when nothing is playing.
    pub fn auto_start(&mut self) {
        if self.mode != Mode::Auto || !self.autoplay_pending {
            return;
        }
        if self.current.is_some() {
            self.autoplay_pending = false;
            return;
        }
        if !self.log.is_empty() {
            self.autoplay_pending = false;
            self.advance(0);
        }
    }

    pub fn start_stream(&mut self) {
        if self.remote.is_some() {
            self.remote_cmd(serde_json::json!({ "do": "stream_start" }));
            return;
        }
        self.stop_stream();
        let Some(p) = self.player.as_mut() else {
            self.status = "No audio output available".into();
            return;
        };
        let mut v = Vec::new();
        for (i, c) in self.data.s.streams.iter().enumerate() {
            if c.enabled {
                let (id, rx) = p.add_tap();
                v.push((i, id, Streamer::start(c.clone(), rx)));
            }
        }
        self.streamers = v;
    }

    pub fn stop_stream(&mut self) {
        if self.remote.is_some() {
            self.remote_cmd(serde_json::json!({ "do": "stream_stop" }));
            return;
        }
        for (_, id, st) in self.streamers.drain(..) {
            st.stop();
            if let Some(p) = self.player.as_mut() {
                p.remove_tap(id);
            }
        }
    }

    /// Runs the daily automatic database backup at the configured time.
    pub fn auto_backup_tick(&mut self) {
        if self.last_backup_check.elapsed() < std::time::Duration::from_secs(30) {
            return;
        }
        self.last_backup_check = Instant::now();
        let b = self.data.s.backup.clone();
        if !b.enabled {
            return;
        }
        let now = Local::now();
        let today = now.format("%Y-%m-%d").to_string();
        let due = engine::parse_hm(&b.time).map(|t| now.time() >= t).unwrap_or(false);
        if due && self.last_backup_day != today {
            self.last_backup_day = today.clone();
            let _ = self.db.set_setting("last_backup_day", &today);
            let (db, dir) = (self.db.clone(), self.backup_dir());
            std::thread::spawn(move || {
                let _ = std::fs::create_dir_all(&dir);
                let name = format!("radiomoon-auto-{}.rmbak", Local::now().format("%Y%m%d-%H%M"));
                if backup::export(&db, &dir.join(name), VERSION).is_ok() {
                    backup::prune(&dir, b.keep);
                }
            });
            self.status = "Automatic database backup started".into();
        }
    }

    // ---------------------------------------------------------------- live rebroadcast
    /// Puts source number `idx` on the air. `slot` is the schedule slot that asked for it (None = by hand).
    pub fn start_live_source(&mut self, idx: usize, mute: bool, slot: Option<usize>) -> bool {
        let Some(src) = self.data.s.live_sources.get(idx).cloned() else {
            self.status = tr("No live source defined (Control Panel > Live rebroadcast)");
            return false;
        };
        let Some(p) = self.player.as_mut() else { return false };
        let fade_ms = (self.data.s.live_fade_s.clamp(0.0, 30.0) * 1000.0) as u32;
        match p.start_live(&src.url, &src.device, src.gain_db, false, fade_ms) {
            Ok(_) => {
                self.live_slot = slot;
                self.live_sel = idx;
                if mute && self.live_prev_mode.is_none() {
                    // the automation stops while the live source is on the air; it is resumed afterwards
                    self.live_prev_mode = Some(self.mode);
                    if let Some(p) = self.player.as_mut() {
                        p.fade_out_all(fade_ms as u64); // the automation fades out while the live source fades in
                    }
                    self.current = None;
                    self.mode = Mode::Manual;
                    self.gen = None;
                }
                self.status = format!("{}: {}", tr("Live source on air"), src.name);
                true
            }
            Err(e) => {
                self.status = format!("{} ({}): {e}", tr("Live source failed"), src.name);
                false
            }
        }
    }

    /// Stops the live source. A scheduled one stays off until its slot ends.
    pub fn stop_live_source(&mut self) {
        let fade_ms = (self.data.s.live_fade_s.clamp(0.0, 30.0) * 1000.0) as u32;
        if let Some(p) = self.player.as_mut() {
            p.end_live(fade_ms);
        }
        if let Some(i) = self.live_slot.take() {
            self.live_dismissed = Some(i);
        }
        self.live_resume_automation();
        self.status = tr("Live source stopped");
    }

    /// After the live source: back to the mode the studio was in; AUTO continues with the next item.
    fn live_resume_automation(&mut self) {
        if let Some(m) = self.live_prev_mode.take() {
            self.mode = m;
            self.gen = None;
            if m == Mode::Auto {
                let fade_ms = (self.data.s.live_fade_s.clamp(0.0, 30.0) * 1000.0) as u64;
                if let Some(p) = self.player.as_mut() {
                    p.fade_in_next(fade_ms); // the automation comes back while the live source fades out
                }
                self.autoplay_pending = true;
                self.auto_fill();
                self.expand_pending();
                self.auto_start();
            }
        }
    }

    /// Starts and ends the scheduled live sources (checked every second).
    pub fn live_tick(&mut self) {
        if self.last_live_check.elapsed() < std::time::Duration::from_secs(1) {
            return;
        }
        self.last_live_check = Instant::now();
        // settings of version 0.5.0 had a single source
        if self.data.s.live_sources.is_empty() && (!self.data.s.live_url.is_empty() || !self.data.s.live_device.is_empty()) {
            let s = &mut self.data.s;
            s.live_sources.push(LiveSource { name: "Live 1".into(), device: std::mem::take(&mut s.live_device), url: std::mem::take(&mut s.live_url), gain_db: s.live_gain_db });
            self.save_settings();
        }
        if self.data.s.live_slots.is_empty() && self.live_slot.is_none() {
            return;
        }
        let now = Local::now();
        let (t, wd) = (now.time(), now.weekday().num_days_from_monday() as usize);
        let mut active: Option<(usize, usize, bool)> = None; // slot, source, mute
        for (i, sl) in self.data.s.live_slots.iter().enumerate() {
            if !sl.enabled {
                continue;
            }
            let (Some(a), Some(b)) = (engine::parse_hm(&sl.start), engine::parse_hm(&sl.end)) else { continue };
            let day = if a < b {
                if t >= a && t < b { Some(wd) } else { None }
            } else if a > b {
                // over midnight: the days are those of the start
                if t >= a { Some(wd) } else if t < b { Some((wd + 6) % 7) } else { None }
            } else {
                None
            };
            if let Some(d) = day {
                if sl.days[d] {
                    if let Some(si) = self.data.s.live_sources.iter().position(|x| x.name == sl.source) {
                        active = Some((i, si, sl.mute));
                        break;
                    }
                }
            }
        }
        if self.live_dismissed.is_some() && self.live_dismissed != active.map(|a| a.0) {
            self.live_dismissed = None;
        }
        let live = self.player.as_ref().map(|p| p.live_active()).unwrap_or(false);
        match active {
            Some((i, si, mute)) => {
                if self.live_slot != Some(i) && self.live_dismissed != Some(i) && !(live && self.live_slot.is_none()) && !self.start_live_source(si, mute, Some(i)) {
                    self.live_dismissed = Some(i);
                }
            }
            None => {
                if self.live_slot.is_some() {
                    let fade_ms = (self.data.s.live_fade_s.clamp(0.0, 30.0) * 1000.0) as u32;
                    if let Some(p) = self.player.as_mut() {
                        p.end_live(fade_ms);
                    }
                    self.live_slot = None;
                    self.live_resume_automation();
                    self.status = tr("Scheduled live source ended");
                }
            }
        }
    }

    pub fn apply_outputs(&mut self) {
        let restart = !self.streamers.is_empty();
        if restart {
            self.stop_stream();
        }
        let s = &self.data.s;
        match Player::new(&s.out_main, &s.out_monitor, &s.out_instants) {
            Ok(p) => {
                self.player = Some(p);
                self.clap_apply_settings();
            }
            Err(e) => {
                self.player = None;
                self.status = format!("No audio output: {e}");
            }
        }
        if restart {
            self.start_stream();
        }
    }

    pub fn save_log(&self) {
        let ids: Vec<u64> = self.log.iter().filter(|t| t.id != 0).map(|t| t.id).collect();
        // the database is written only when the list changed
        static LAST: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
        if let Ok(mut l) = LAST.lock() {
            if *l == ids {
                return;
            }
            *l = ids.clone();
        }
        let _ = self.db.save_log(&ids);
    }

    pub fn now_virtual(&self) -> NaiveDateTime {
        Local::now().naive_local() + CDur::seconds(self.data.s.time_offset_s as i64)
    }

    pub fn map_path(&self, p: &str) -> String {
        for (from, to) in &self.data.s.folder_maps {
            if !from.is_empty() && p.to_lowercase().starts_with(&from.to_lowercase()) {
                if let Some(rest) = p.get(from.len()..) {
                    return format!("{to}{rest}");
                }
            }
        }
        p.to_string()
    }

    // ---------- playout ----------
    pub fn stop(&mut self) {
        if let Some(p) = self.player.as_mut() {
            p.stop();
        }
        self.current = None;
    }

    fn build_stitch(&self, t: &Track, idx: usize) -> Result<Vec<(String, u64, u64)>, String> {
        let c = &self.data.s.stitch;
        let mut parts: Vec<(String, u64, u64)> = Vec::new();
        let push = |parts: &mut Vec<(String, u64, u64)>, p: &str| {
            if !p.is_empty() {
                parts.push((self.map_path(p), 0, 0));
            }
        };
        match t.kind.as_str() {
            "stitch_hooks" => {
                push(&mut parts, &c.hooks_open);
                let mut n = 0;
                for s in self.log.iter().skip(idx + 1) {
                    if s.kind == "music" && s.hook_out_ms > s.hook_in_ms {
                        parts.push((self.map_path(&s.path), s.hook_in_ms, s.hook_out_ms - s.hook_in_ms));
                        push(&mut parts, &c.hooks_sep);
                        n += 1;
                        if n >= c.hooks_count.max(1) {
                            break;
                        }
                    }
                }
                if n == 0 {
                    parts.clear();
                    push(&mut parts, &c.hooks_default);
                } else {
                    push(&mut parts, &c.hooks_close);
                }
            }
            "stitch_time" => {
                let now = self.now_virtual();
                let h = find_audio(&c.time_dir, &format!("h{:02}", now.hour()));
                let m = find_audio(&c.time_dir, &format!("m{:02}", now.minute()));
                match (h, m) {
                    (Some(h), Some(m)) => {
                        push(&mut parts, &c.time_open);
                        parts.push((h, 0, 0));
                        parts.push((m, 0, 0));
                        push(&mut parts, &c.time_close);
                    }
                    _ => push(&mut parts, &c.time_default),
                }
            }
            _ => {
                if !c.news_voice.is_empty() && Path::new(&self.map_path(&c.news_voice)).exists() {
                    push(&mut parts, &c.news_open);
                    push(&mut parts, &c.news_voice);
                    push(&mut parts, &c.news_close);
                } else {
                    push(&mut parts, &c.news_default);
                }
            }
        }
        if parts.is_empty() {
            Err("stitcher has no audio configured".into())
        } else {
            Ok(parts)
        }
    }

    fn overlay_events(&self, i: usize, t: &Track) -> Vec<(u64, String, f32)> {
        let skip = t.mix_in_ms;
        let end = if t.mix_out_ms > 0 && t.mix_out_ms <= t.duration_ms { t.mix_out_ms } else { t.duration_ms };
        let mut ev = Vec::new();
        let mut j = i;
        while j > 0 && self.log[j - 1].is_overlay() {
            j -= 1;
            let o = &self.log[j];
            let pos = match o.overlay_pos.as_str() {
                "intro" => t.intro_ms.max(skip).saturating_sub(o.duration_ms),
                "end" => end.saturating_sub(o.duration_ms),
                "bridge" => end.saturating_sub(o.duration_ms / 2),
                "custom" => skip + o.overlay_custom_ms,
                _ => skip,
            };
            ev.push((pos.max(skip), self.map_path(&o.path), o.gain_db));
        }
        ev
    }

    pub fn play_index(&mut self, i: usize, fade_ms: u64) {
        if i >= self.log.len() {
            return;
        }
        if self.mode == Mode::Offline {
            self.status = "Offline mode: nothing is played".into();
            return;
        }
        if self.log[i].is_overlay() {
            let mut j = i;
            while j < self.log.len() && self.log[j].is_overlay() {
                j += 1;
            }
            if j < self.log.len() {
                self.play_index(j, fade_ms);
            } else {
                self.stop();
            }
            return;
        }
        if self.log[i].kind == "break" {
            let items = engine::expand_break(&self.data, &self.log[i].clone());
            self.log.splice(i..i + 1, items);
            if i < self.log.len() {
                self.play_index(i, fade_ms);
            } else {
                self.stop();
                self.status = "End of log".into();
            }
            return;
        }
        let t = self.log[i].clone();
        let stitch = t.kind.starts_with("stitch_");
        let parts = if stitch { Some(self.build_stitch(&t, i)) } else { None };
        let events = self.overlay_events(i, &t);
        let path = self.map_path(&t.path);
        let agc = if self.data.s.auto_volume { Some((self.data.s.auto_volume_target_db, self.data.s.auto_volume_boost_db, self.data.s.auto_volume_cut_db)) } else { None };
        let Some(p) = self.player.as_mut() else {
            self.status = "No audio output available".into();
            return;
        };
        p.agc = agc;
        let res = match parts {
            Some(Ok(pts)) => p.play_sequence(&pts, t.gain_db, fade_ms),
            Some(Err(e)) => Err(anyhow::anyhow!(e)),
            None => p.play_main(&path, t.mix_in_ms, t.gain_db, fade_ms),
        };
        match res {
            Ok(()) => {
                p.set_events(events);
                self.current = Some(i);
                self.scroll_to = Some(i);
                self.log[i].fixed_start.clear();
                self.after_play(i);
                // the songs that were already played leave the playlist (the overlays of the new song stay with it)
                let mut j = i;
                while j > 0 && self.log[j - 1].is_overlay() {
                    j -= 1;
                }
                if j > 0 {
                    self.log.drain(0..j);
                    self.current = Some(i - j);
                    self.scroll_to = Some(i - j);
                    self.sel = self.sel.and_then(|x| x.checked_sub(j));
                    self.save_log();
                }
            }
            Err(e) => {
                self.status = format!("Cannot play '{}': {e}", t.title);
                self.log[i].fixed_start.clear();
                if i + 1 < self.log.len() {
                    self.play_index(i + 1, 0);
                } else {
                    self.current = None;
                }
            }
        }
    }

    fn after_play(&mut self, i: usize) {
        let t = self.log[i].clone();
        // overlays that belong to this track are history items too
        let mut j = i;
        while j > 0 && self.log[j - 1].is_overlay() {
            j -= 1;
            let o = self.log[j].clone();
            if o.id != 0 {
                let _ = self.db.mark_played(o.id, &o.src, &o.reason);
            }
        }
        if t.id != 0 {
            let _ = self.db.mark_played(t.id, &t.src, &t.reason);
            if t.kind != "stitch_hooks" {
                waveform::spawn(self.map_path(&t.path), t.id, self.wave.clone());
            }
        }
        self.history = self.db.history(40).unwrap_or_default();
        let next: Vec<Track> = self.log.iter().skip(i + 1).take(8).cloned().collect();
        export::now_on_air(&self.base, &self.data.s, &t, &next, &self.history);
        export::custom_log(&self.base, &self.data.s, &t);
        if !self.streamers.is_empty() {
            if let Some((artist, title)) = export::meta_for(&self.data.s, &t) {
                let full = if artist.is_empty() { title } else { format!("{artist} - {title}") };
                for (_, _, st) in &self.streamers {
                    st.set_title(&full);
                }
            }
        }
        self.status = format!("On air: {}", t.label());
    }

    pub fn advance(&mut self, fade_ms: u64) {
        let next = self.current.map(|i| i + 1).unwrap_or(0);
        if next < self.log.len() {
            self.play_index(next, fade_ms);
        } else {
            self.stop();
            self.status = "End of log".into();
        }
    }

    /// Estimated start time of every log item.
    pub fn starts(&self) -> Vec<NaiveDateTime> {
        let now = self.now_virtual();
        let elapsed = self.px_elapsed();
        let n = self.log.len();
        let mut v = vec![now; n];
        if n == 0 {
            return v;
        }
        let c = self.current.unwrap_or(0).min(n - 1);
        v[c] = if self.current.is_some() { now - CDur::milliseconds(elapsed as i64) } else { now };
        for i in c..n - 1 {
            let ms = if self.log[i].is_overlay() { 0 } else { self.log[i].eff_ms() as i64 };
            v[i + 1] = v[i] + CDur::milliseconds(ms);
        }
        for i in (0..c).rev() {
            let ms = if self.log[i].is_overlay() { 0 } else { self.log[i].eff_ms() as i64 };
            v[i] = v[i + 1] - CDur::milliseconds(ms);
        }
        v
    }

    /// Replaces break references that are due within the horizon by their real spots.
    pub fn expand_pending(&mut self) {
        let horizon = (self.data.s.auto_hours.max(1) as i64) * 3_600_000;
        let start = self.current.map(|c| c + 1).unwrap_or(0);
        let mut acc: i64 = 0;
        if let Some(c) = self.current {
            if let Some(t) = self.log.get(c) {
                acc = (t.eff_ms() as i64 - self.px_elapsed() as i64).max(0);
            }
        }
        let mut i = start;
        let mut guard = 0;
        while i < self.log.len() && guard < 5000 {
            guard += 1;
            if self.log[i].kind == "break" && acc <= horizon {
                let items = engine::expand_break(&self.data, &self.log[i].clone());
                let empty = items.is_empty();
                self.log.splice(i..i + 1, items);
                if empty {
                    continue;
                }
            }
            if i < self.log.len() && !self.log[i].is_overlay() {
                acc += self.log[i].eff_ms() as i64;
            }
            i += 1;
        }
    }

    /// Auto mode: loads a scheduled final log, or generates items from the clocks.
    pub fn auto_fill(&mut self) {
        if self.mode != Mode::Auto || self.data.tracks.is_empty() {
            return;
        }
        let now = self.now_virtual();
        let today = engine::dstr(now.date());
        // a saved final log for today is loaded once
        if self.final_loaded != today {
            if let Some(f) = self.data.finals.iter().find(|f| f.date == today).cloned() {
                let items = engine::from_final(&self.data, &f);
                let secs = (now.time().num_seconds_from_midnight()) as u64 * 1000;
                let mut acc = 0u64;
                let mut skipped = 0usize;
                for t in &items {
                    if acc >= secs {
                        break;
                    }
                    if !t.is_overlay() {
                        acc += t.eff_ms().max(1000);
                    }
                    skipped += 1;
                }
                let keep = self.current.map(|c| c + 1).unwrap_or(0);
                self.log.truncate(keep.min(self.log.len()));
                self.log.extend(items.into_iter().skip(skipped));
                self.gen = None;
                self.status = format!("Final log of {today} loaded");
            }
            self.final_loaded = today;
            self.save_log();
        }
        let pending = self.log.len().saturating_sub(self.current.map(|c| c + 1).unwrap_or(0));
        let rem_ms: u64 = self
            .log
            .iter()
            .skip(self.current.unwrap_or(0))
            .filter(|t| !t.is_overlay())
            .map(|t| t.eff_ms().max(1000)) // as the generator counts them: an item without a length never makes the list "too short" for ever
            .sum();
        if pending > 3000 {
            return;
        }
        let end_est = now + CDur::milliseconds(rem_ms as i64);
        let classic = self.data.s.classic_auto;
        let need = if classic { pending < self.data.s.classic_decks.max(1) } else { end_est < now + CDur::hours(self.data.s.auto_hours.max(1) as i64) };
        if !need {
            return;
        }
        if self.gen.is_none() {
            let recent: Vec<(u64, NaiveDateTime)> = self
                .db
                .recent_plays(48)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(id, s)| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok().map(|d| (id, d)))
                .collect();
            self.gen = Some(Generator::new(end_est, &recent, &self.data));
        }
        let until = if classic {
            self.gen.as_ref().map(|g| g.t).unwrap_or(end_est) + CDur::seconds(1)
        } else {
            now + CDur::hours(self.data.s.auto_hours.max(1) as i64)
        };
        if let Some(g) = self.gen.as_mut() {
            let items = g.fill_until(&self.data, until, &engine::Flags::default(), false);
            self.log.extend(items);
        }
        self.expand_pending();
        self.save_log();
    }

    /// Starts items that must begin at an exact time ("start immediately").
    pub fn check_immediate(&mut self) {
        if self.mode == Mode::Offline {
            return;
        }
        let now = self.now_virtual();
        let from = self.current.map(|c| c + 1).unwrap_or(0);
        let mut hit = None;
        for (i, t) in self.log.iter().enumerate().skip(from) {
            if t.fixed_start.is_empty() {
                continue;
            }
            if let Ok(at) = NaiveDateTime::parse_from_str(&t.fixed_start, "%Y-%m-%d %H:%M:%S") {
                if now >= at && now < at + CDur::minutes(10) {
                    hit = Some(i);
                    break;
                }
            }
        }
        if let Some(i) = hit {
            self.play_index(i, 1200);
        }
    }

    pub fn trigger_ms(&self) -> Option<u64> {
        let t = self.current.and_then(|c| self.log.get(c))?;
        if t.kind.starts_with("stitch_") {
            return None;
        }
        Some(if t.mix_out_ms > 0 && t.mix_out_ms <= t.duration_ms { t.mix_out_ms } else { t.duration_ms })
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // a client (remote mode) must never stop the broadcast that runs on the server when its window is closed
        if self.remote.is_none() {
            self.stop_stream();
        }
        if self.server.owned() {
            let _ = self.db.shutdown();
            self.server.mark_shutdown();
        }
    }
}
