//! Remote mode: the studio window of the desktop program controls a RadioMoon server. The server plays; this window shows the
//! same studio, sends the buttons to the server and keeps a copy of its state. Closing the window never stops the broadcast.
//! The library and the settings are read from the same database; songs are uploaded to the server and played back from it.
use crate::app::{App, Mode};
use crate::studio::Action;
use base64::Engine;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// Bytes sent to / received from the server (shown as the bandwidth in the status bar).
pub static RX_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static TX_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn rx(n: u64) {
    RX_BYTES.fetch_add(n, Ordering::Relaxed);
}
pub fn tx(n: u64) {
    TX_BYTES.fetch_add(n, Ordering::Relaxed);
}

/// Connection quality: 0 normal, 1 low bandwidth, 2 very low bandwidth (slower refresh, no waveform download).
pub static LOW_BW: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn bw_mode() -> u8 {
    LOW_BW.load(Ordering::Relaxed).min(2)
}

pub fn bw_name(m: u8) -> &'static str {
    match m {
        1 => "Low bandwidth",
        2 => "Very low bandwidth",
        _ => "Normal connection",
    }
}

pub enum Cmd {
    Act(Value),
    Quit,
}

#[derive(Default)]
pub struct Shared {
    pub state: Option<Value>,
    /// the playlist of the server, when it changed
    pub log: Option<Vec<crate::model::Track>>,
    pub seq: u64,
    pub user: String,
    pub admin: bool,
    pub err: String,
    pub denied: bool,
    pub online: bool,
}

pub struct Link {
    pub tx: mpsc::Sender<Cmd>,
    pub sh: Arc<Mutex<Shared>>,
    pub base: String,
    pub auth: String,
}

/// "1.2 MB/s"
pub fn fmt_rate(b: f64) -> String {
    if b >= 1_048_576.0 {
        format!("{:.1} MB/s", b / 1_048_576.0)
    } else {
        format!("{:.1} KB/s", b / 1024.0)
    }
}

pub fn base_url(url: &str) -> String {
    let b = url.trim().trim_end_matches('/').to_string();
    if b.starts_with("http://") || b.starts_with("https://") {
        b
    } else {
        format!("http://{b}")
    }
}

/// Host name of a server address (`http://host:8080/` -> `host`).
pub fn host_of(url: &str) -> String {
    let b = base_url(url);
    let h = b.split("://").nth(1).unwrap_or("");
    let h = h.split('/').next().unwrap_or("");
    h.rsplit_once(':').map(|x| x.0).unwrap_or(h).to_string()
}

pub fn enc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(4)).timeout_read(Duration::from_secs(10)).build()
}

fn call(base: &str, auth: &str, post: Option<&Value>, path: &str) -> Result<Value, String> {
    let u = format!("{base}{path}");
    if let Some(b) = post {
        tx(b.to_string().len() as u64 + 200);
    } else {
        tx(200);
    }
    let r = match post {
        Some(b) => agent().post(&u).set("Authorization", auth).set("Content-Type", "application/json").send_string(&b.to_string()),
        None => agent().get(&u).set("Authorization", auth).call(),
    };
    match r {
        Ok(r) => {
            let t = r.into_string().map_err(|e| e.to_string())?;
            rx(t.len() as u64 + 200);
            serde_json::from_str::<Value>(&t).map_err(|e| e.to_string())
        }
        Err(ureq::Error::Status(401, _)) => Err("401".into()),
        Err(ureq::Error::Status(c, r)) => {
            let t = r.into_string().unwrap_or_default();
            let m = serde_json::from_str::<Value>(&t).ok().and_then(|v| v["error"].as_str().map(|x| x.to_string()));
            Err(m.unwrap_or_else(|| format!("server answered {c}")))
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn get_json(l: &Link, path: &str) -> Result<Value, String> {
    call(&l.base, &l.auth, None, path)
}

pub fn post_json(l: &Link, v: &Value) -> Result<Value, String> {
    call(&l.base, &l.auth, Some(v), "/api/action")
}

/// Starts the connection thread: signs in, then keeps the state of the server up to date and sends the commands.
pub fn connect(url: String, user: String, pass: String, ctx: eframe::egui::Context) -> Link {
    let (tx, rx) = mpsc::channel::<Cmd>();
    let sh = Arc::new(Mutex::new(Shared::default()));
    let base = base_url(&url);
    let auth = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}")));
    let (sh2, base2, auth2) = (sh.clone(), base.clone(), auth.clone());
    std::thread::spawn(move || {
        match call(&base2, &auth2, None, "/api/login") {
            Ok(v) => {
                let mut g = sh2.lock().unwrap();
                g.user = v["user"].as_str().unwrap_or("").to_string();
                g.admin = v["admin"].as_bool().unwrap_or(false);
                g.online = true;
            }
            Err(e) if e == "401" => {
                let mut g = sh2.lock().unwrap();
                g.denied = true;
                g.err = "Wrong user name or password".into();
                ctx.request_repaint();
                return;
            }
            Err(e) => sh2.lock().unwrap().err = e,
        }
        let mut last_rev: u64 = u64::MAX;
        let mut carry: Option<Cmd> = None;
        loop {
            loop {
                match carry.take().map(Ok).unwrap_or_else(|| rx.try_recv()) {
                    Ok(Cmd::Quit) => return,
                    Ok(Cmd::Act(mut b)) => match { b["lite"] = json!(true); call(&base2, &auth2, Some(&b), "/api/action") } {
                        Ok(v) => {
                            let mut g = sh2.lock().unwrap();
                            g.state = Some(v);
                            g.seq += 1;
                        }
                        Err(e) => sh2.lock().unwrap().err = e,
                    },
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => return,
                }
            }
            match call(&base2, &auth2, None, "/api/state?lite=1") {
                Ok(v) => {
                    // the playlist is read again only when it changed
                    let rev = v["log_rev"].as_u64().unwrap_or(0);
                    if rev != last_rev {
                        if let Ok(l) = call(&base2, &auth2, None, "/api/log") {
                            let items: Vec<crate::model::Track> = l["items"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .map(|it| {
                                            let mut t: crate::model::Track = serde_json::from_value(it["t"].clone()).unwrap_or_default();
                                            t.src = it["src"].as_str().unwrap_or("").to_string();
                                            t.reason = it["reason"].as_str().unwrap_or("").to_string();
                                            t.overlay_pos = it["ov"].as_str().unwrap_or("").to_string();
                                            t.overlay_custom_ms = it["ovc"].as_u64().unwrap_or(0);
                                            t.fixed_start = it["fs"].as_str().unwrap_or("").to_string();
                                            t
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            last_rev = l["rev"].as_u64().unwrap_or(rev);
                            sh2.lock().unwrap().log = Some(items);
                        }
                    }
                    let mut g = sh2.lock().unwrap();
                    g.state = Some(v);
                    g.seq += 1;
                    g.online = true;
                    g.err.clear();
                }
                Err(e) if e == "401" => {
                    let mut g = sh2.lock().unwrap();
                    g.denied = true;
                    g.err = "Wrong user name or password".into();
                    return;
                }
                Err(e) => {
                    let mut g = sh2.lock().unwrap();
                    g.online = false;
                    g.err = e;
                }
            }
            ctx.request_repaint();
            // the next reading of the state; a button of the studio wakes this thread at once
            let poll = match bw_mode() {
                1 => 1500,
                2 => 4000,
                _ => 400,
            };
            match rx.recv_timeout(Duration::from_millis(poll)) {
                Ok(c) => carry = Some(c),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    });
    Link { tx, sh, base, auth }
}

pub struct Remote {
    pub link: Link,
    seq: u64,
    cur: Option<usize>,
    data_rev: Option<u64>,
    el0: u64,
    pos0: u64,
    at: Instant,
    playing: bool,
    paused: bool,
    pub volume: f32,
    pub live: bool,
    pub online: bool,
    pub err: String,
    pub streams: Vec<(usize, String)>,
    pub listeners: Vec<(usize, i64)>,
    last_data: Instant,
    cp_was_open: bool,
    fails: Mutex<std::collections::HashMap<u64, Instant>>,
    wave_id: u64,
    bw_at: Instant,
    bw_rx: u64,
    bw_tx: u64,
    pub rx_rate: f64,
    pub tx_rate: f64,
}

impl Remote {
    pub fn new(link: Link) -> Self {
        Remote { link, seq: 0, cur: None, data_rev: None, el0: 0, pos0: 0, at: Instant::now(), playing: false, paused: false, volume: 1.0, live: false, online: false, err: String::new(), streams: Vec::new(), listeners: Vec::new(), last_data: Instant::now(), cp_was_open: false, fails: Mutex::new(Default::default()), wave_id: 0, bw_at: Instant::now(), bw_rx: 0, bw_tx: 0, rx_rate: 0.0, tx_rate: 0.0 }
    }
    fn running(&self) -> bool {
        self.playing && !self.paused
    }
    pub fn elapsed(&self) -> u64 {
        self.el0 + if self.running() { self.at.elapsed().as_millis() as u64 } else { 0 }
    }
    pub fn pos(&self) -> u64 {
        self.pos0 + if self.running() { self.at.elapsed().as_millis() as u64 } else { 0 }
    }
}

impl App {
    // ---- values the studio shows (the server's player in remote mode, the own player otherwise) ----
    pub fn px_elapsed(&self) -> u64 {
        match &self.remote {
            Some(r) => r.elapsed(),
            None => self.player.as_ref().map(|p| p.elapsed_ms()).unwrap_or(0),
        }
    }
    pub fn px_pos(&self) -> u64 {
        match &self.remote {
            Some(r) => r.pos(),
            None => self.player.as_ref().map(|p| p.pos_ms()).unwrap_or(0),
        }
    }
    pub fn px_paused(&self) -> bool {
        match &self.remote {
            Some(r) => r.paused,
            None => self.player.as_ref().map(|p| p.is_paused()).unwrap_or(false),
        }
    }
    pub fn px_live(&self) -> bool {
        match &self.remote {
            Some(r) => r.live,
            None => self.player.as_ref().map(|p| p.live_active()).unwrap_or(false),
        }
    }
    pub fn px_volume(&self) -> f32 {
        match &self.remote {
            Some(r) => r.volume,
            None => self.player.as_ref().map(|p| p.volume).unwrap_or(1.0),
        }
    }
    /// The internet broadcasts that run: on the server in remote mode, in this program otherwise.
    pub fn px_streams(&self) -> Vec<(usize, String)> {
        match &self.remote {
            Some(r) => r.streams.clone(),
            None => self.streamers.iter().map(|(i, _, s)| (*i, s.status_text())).collect(),
        }
    }
    /// Listeners of every internet broadcast that runs (-1 = not known).
    pub fn px_listeners(&self) -> Vec<(usize, i64)> {
        match &self.remote {
            Some(r) => r.listeners.clone(),
            None => self.streamers.iter().map(|(i, _, s)| (*i, s.listeners())).collect(),
        }
    }
    /// A command for the server (broadcast start / stop).
    pub fn remote_cmd(&self, v: Value) {
        if let Some(r) = &self.remote {
            let _ = r.link.tx.send(Cmd::Act(v));
        }
    }
    /// A file of the library is there: on the server in remote mode (it is checked by the server), on this computer otherwise.
    pub fn file_ok(&self, p: &str) -> bool {
        self.remote.is_some() || Path::new(&self.map_path(p)).exists()
    }

    /// Every second or so in remote mode: takes over what the server tells (mode, current item, playlist, volume...).
    pub fn remote_tick(&mut self) {
        let Some(mut rem) = self.remote.take() else { return };
        auto_clean(self.base.join("cache"));
        let (st, seq, online, err) = {
            let g = rem.link.sh.lock().unwrap();
            (if g.seq != rem.seq { g.state.clone() } else { None }, g.seq, g.online, g.err.clone())
        };
        rem.online = online;
        rem.err = err;
        if let Some(p) = self.player.as_mut() {
            p.tick(); // the preview on this computer
        }
        if self.scan_done.swap(false, Ordering::SeqCst) {
            self.reload_tracks();
        }
        if let Some(s) = st {
            rem.seq = seq;
            self.mode = match s["mode"].as_str() {
                Some("auto") => Mode::Auto,
                Some("offline") => Mode::Offline,
                _ => Mode::Manual,
            };
            self.loop_current = s["loop"].as_bool().unwrap_or(false);
            self.stop_next = s["stopnext"].as_bool().unwrap_or(false);
            rem.cur = s["current"].as_u64().map(|x| x as usize);
            // the library or the documents were changed on the server (by this or another client): read them again
            if let Some(r) = s["data_rev"].as_u64() {
                if rem.data_rev.map(|o| o != r).unwrap_or(false) {
                    rem.last_data = Instant::now();
                    self.reload_all();
                }
                rem.data_rev = Some(r);
            }
            self.status = s["status"].as_str().unwrap_or("").to_string();
            rem.el0 = s["elapsed"].as_u64().unwrap_or(0);
            rem.pos0 = s["pos"].as_u64().unwrap_or(0);
            rem.at = Instant::now();
            rem.playing = !s["now"].is_null();
            rem.paused = s["paused"].as_bool().unwrap_or(false);
            rem.volume = s["volume"].as_f64().unwrap_or(1.0) as f32;
            rem.live = s["live"].as_bool().unwrap_or(false);
            if let Some(h) = s["history"].as_array() {
                self.history = h.iter().map(|x| (x["time"].as_str().unwrap_or("").to_string(), x["label"].as_str().unwrap_or("").to_string())).collect();
            }
            rem.streams = s["streams"].as_array().map(|a| a.iter().map(|x| (x["i"].as_u64().unwrap_or(0) as usize, x["status"].as_str().unwrap_or("").to_string())).collect()).unwrap_or_default();
            rem.listeners = s["streams"].as_array().map(|a| a.iter().map(|x| (x["i"].as_u64().unwrap_or(0) as usize, x["listeners"].as_i64().unwrap_or(-1))).collect()).unwrap_or_default();
        }
        // the playlist of the server
        let newlog = rem.link.sh.lock().ok().and_then(|mut g| g.log.take());
        if let Some(tracks) = newlog {
            self.log = tracks;
            if self.sel.map(|i| i >= self.log.len()).unwrap_or(false) {
                self.sel = None;
            }
        }
        // the item on air is the one of the server's playlist (never an index that is not in the list)
        self.current = rem.cur.filter(|c| *c < self.log.len());
        if !rem.online {
            self.status = format!("No connection to the server: {}", rem.err);
        }
        // changes made in the control panel (or by another computer) reach the server
        if rem.cp_was_open && !self.cp_open {
            let _ = rem.link.tx.send(Cmd::Act(json!({ "do": "reload" })));
            self.reload_all();
            rem.last_data = Instant::now();
        }
        rem.cp_was_open = self.cp_open;
        if rem.last_data.elapsed() > Duration::from_secs(match bw_mode() { 0 => 900, 1 => 1800, _ => 3600 }) {
            rem.last_data = Instant::now();
            self.reload_all();
        }
        self.remote_wave(&mut rem);
        let el = rem.bw_at.elapsed().as_secs_f64();
        if el >= 1.0 {
            let (r, t) = (RX_BYTES.load(Ordering::Relaxed), TX_BYTES.load(Ordering::Relaxed));
            rem.rx_rate = (r - rem.bw_rx) as f64 / el;
            rem.tx_rate = (t - rem.bw_tx) as f64 / el;
            rem.bw_rx = r;
            rem.bw_tx = t;
            rem.bw_at = Instant::now();
        }
        self.remote = Some(rem);
    }

    /// Sends a studio button to the server. Returns false for the actions that stay on this computer (selection, tabs, pages...).
    pub fn remote_send(&mut self, a: &Action) -> bool {
        let v = match a {
            Action::PlayIndex(i) => json!({ "do": "play", "index": i }),
            Action::Next => json!({ "do": "next" }),
            Action::Restart => json!({ "do": "restart" }),
            Action::ToggleLoop => json!({ "do": "loop" }),
            Action::Pause => json!({ "do": "pause" }),
            Action::StopNext => json!({ "do": "stopnext" }),
            Action::Stop => json!({ "do": "stop" }),
            Action::Volume(x) => json!({ "do": "volume", "value": x }),
            Action::Jingle(id) => json!({ "do": "jingle", "id": id }),
            Action::StopJingles => json!({ "do": "stopjingles" }),
            Action::MoveItemUp | Action::MoveItemDown => match self.sel {
                Some(s) => json!({ "do": if matches!(a, Action::MoveItemUp) { "up" } else { "down" }, "index": s }),
                None => return true,
            },
            Action::RemoveLog(i) => json!({ "do": "remove", "index": i }),
            Action::AddSel | Action::InsertSel => match self.lib_sel {
                Some(id) => json!({ "do": "add", "id": id, "where": if matches!(a, Action::InsertSel) { "next" } else { "end" } }),
                None => return true,
            },
            Action::ClearLog => json!({ "do": "clear" }),
            Action::SetMode(m) => json!({ "do": "mode", "mode": match m { Mode::Auto => "auto", Mode::Manual => "manual", Mode::Offline => "offline" } }),
            Action::ToggleLive => json!({ "do": "live" }),
            Action::StartLive(i) => json!({ "do": "startlive", "index": i }),
            Action::RegenAuto => json!({ "do": "regen" }),
            _ => return false,
        };
        if let Some(r) = &self.remote {
            let _ = r.link.tx.send(Cmd::Act(v));
        }
        true
    }

    /// A song of the server on this computer, for listening and for the waveform: it is downloaded once to the cache folder.
    pub fn local_audio(&self, p: &str) -> String {
        let Some(r) = &self.remote else { return self.map_path(p) };
        let Some(t) = self.data.tracks.iter().find(|t| t.path == p) else { return p.to_string() };
        if r.fails.lock().map(|f| f.get(&t.id).map(|w| w.elapsed() < Duration::from_secs(15)).unwrap_or(false)).unwrap_or(false) {
            return p.to_string(); // it failed a moment ago: not tried again at every frame
        }
        match fetch_audio(&r.link.base, &r.link.auth, t.id, p, &self.base.join("cache")) {
            Some(f) => f.to_string_lossy().to_string(),
            None => {
                if let Ok(mut f) = r.fails.lock() {
                    f.insert(t.id, Instant::now());
                }
                p.to_string()
            }
        }
    }

    /// The server (address and login) of this client.
    pub fn remote_link(&self) -> Option<(String, String)> {
        self.remote.as_ref().map(|r| (r.link.base.clone(), r.link.auth.clone()))
    }

    /// The waveform of the item on air: the server decodes the song and sends only the peaks (some KB), the song is not downloaded.
    fn remote_wave(&mut self, rem: &mut Remote) {
        let cur = self.current.and_then(|c| self.log.get(c)).filter(|t| t.id != 0 && t.kind != "stitch_hooks").map(|t| t.id);
        let Some(id) = cur else { return };
        if rem.wave_id == id {
            return;
        }
        rem.wave_id = id;
        spawn_wave((rem.link.base.clone(), rem.link.auth.clone()), id, self.wave.clone());
    }

    /// The files of the library that are on this computer (an import of another program has the paths of this computer) are uploaded
    /// to the server and the items of the library point to the copy on the server.
    pub fn remote_upload_local(&mut self) {
        let Some(r) = &self.remote else { return };
        let (base, auth) = (r.link.base.clone(), r.link.auth.clone());
        let items: Vec<(u64, String, String)> = self
            .data
            .tracks
            .iter()
            .filter(|t| t.kind != "memo" && !t.path.is_empty() && Path::new(&t.path).is_file())
            .map(|t| (t.id, t.kind.clone(), t.path.clone()))
            .collect();
        let (st, done) = (self.scan_status.clone(), self.scan_done.clone());
        if items.is_empty() {
            self.status = "No file of this computer is in the library".into();
            return;
        }
        std::thread::spawn(move || {
            let (mut ok, mut failed) = (0, 0);
            let mut last = String::new();
            for (i, (id, kind, path)) in items.iter().enumerate() {
                if let Ok(mut s) = st.lock() {
                    *s = format!("Uploading to the server {} / {}: {}", i + 1, items.len(), Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
                }
                match upload_file(&base, &auth, Path::new(path), kind, None, Some(*id)) {
                    Ok(()) => ok += 1,
                    Err(e) => {
                        failed += 1;
                        last = e;
                    }
                }
            }
            if let Ok(mut s) = st.lock() {
                *s = if failed > 0 { format!("Upload finished: {ok} songs, {failed} failed ({last})") } else { format!("Upload finished: {ok} songs are on the server") };
            }
            done.store(true, Ordering::SeqCst);
        });
    }

    /// Imports files or a folder of this computer into the library of the server: every audio file is uploaded, then the server adds it.
    pub fn remote_import(&mut self, path: PathBuf, o: crate::library::ImportOpts) {
        let Some(r) = &self.remote else { return };
        let (base, auth) = (r.link.base.clone(), r.link.auth.clone());
        let (st, done) = (self.scan_status.clone(), self.scan_done.clone());
        std::thread::spawn(move || {
            let files: Vec<PathBuf> = walkdir::WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .map(|e| e.path().to_path_buf())
                .filter(|p| p.extension().map(|x| crate::library::EXTS.contains(&x.to_string_lossy().to_lowercase().as_str())).unwrap_or(false))
                .collect();
            let (mut ok, mut failed) = (0, 0);
            let mut last = String::new();
            for (i, f) in files.iter().enumerate() {
                if let Ok(mut s) = st.lock() {
                    *s = format!("Uploading {} / {}: {}", i + 1, files.len(), f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
                }
                match upload_file(&base, &auth, f, &o.kind.clone(), Some(&o), None) {
                    Ok(()) => ok += 1,
                    Err(e) => {
                        failed += 1;
                        last = e;
                    }
                }
            }
            if let Ok(mut s) = st.lock() {
                *s = if failed > 0 { format!("Upload finished: {ok} files, {failed} failed ({last})") } else { format!("Upload finished: {ok} files") };
            }
            done.store(true, Ordering::SeqCst);
        });
    }
}

/// Downloads a song of the server to the cache folder (once). `path` only gives the extension.
fn fetch_audio(base: &str, auth: &str, id: u64, path: &str, dir: &Path) -> Option<PathBuf> {
    let ext = Path::new(path).extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| "mp3".into());
    let _ = std::fs::create_dir_all(dir);
    let file = dir.join(format!("{id}.{ext}"));
    if file.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        return Some(file);
    }
    let part = dir.join(format!("{id}.part"));
    let resp = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(4))
        .timeout_read(Duration::from_secs(60))
        .build()
        .get(&format!("{base}/api/audio?id={id}"))
        .set("Authorization", auth)
        .call()
        .ok()?;
    let mut f = std::fs::File::create(&part).ok()?;
    let n = std::io::copy(&mut resp.into_reader(), &mut f).ok()?;
    rx(n);
    std::fs::rename(&part, &file).ok()?;
    Some(file)
}

pub fn upload_file(base: &str, auth: &str, f: &Path, kind: &str, o: Option<&crate::library::ImportOpts>, track_id: Option<u64>) -> Result<(), String> {
    let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    let file = std::fs::File::open(f).map_err(|e| e.to_string())?;
    let mut q = format!("name={}&kind={}", enc(&name), enc(kind));
    if let Some(o) = o {
        q.push_str(&format!(
            "&cat1={}&read_tags={}&parse_filename={}&gain={}&bookmark={}&autocue={}&autocue_db={}",
            enc(&o.cat1),
            o.read_tags,
            o.parse_filename,
            o.gain_db,
            o.bookmark,
            o.autocue,
            o.autocue_db
        ));
    }
    if let Some(id) = track_id {
        q.push_str(&format!("&track_id={id}"));
    }
    tx(len);
    let r = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(6))
        .timeout_read(Duration::from_secs(300))
        .timeout_write(Duration::from_secs(300))
        .build()
        .post(&format!("{base}/api/upload?{q}"))
        .set("Authorization", auth)
        .set("Content-Length", &len.to_string())
        .send(file);
    match r {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(c, r)) => {
            let t = r.into_string().unwrap_or_default();
            let m = serde_json::from_str::<Value>(&t).ok().and_then(|v| v["error"].as_str().map(|x| x.to_string()));
            Err(m.unwrap_or_else(|| format!("server answered {c}")))
        }
        Err(e) => Err(e.to_string()),
    }
}

// ------------------------------------------------------------------ database through the server
pub struct DbInfo {
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
}

/// The client has no connection of its own to the database. A local port (only for this computer) carries every connection of the
/// database client to the server, inside the HTTP port of the server.
pub fn start_db_proxy(base: &str, auth: &str) -> Result<DbInfo, String> {
    let o = call(base, auth, Some(&json!({})), "/api/tunnel/open").map_err(|e| if e == "not found" { "This server is older than the client: update the server".to_string() } else { e })?;
    let id = o["id"].as_str().unwrap_or("").to_string();
    let info_user = o["user"].as_str().unwrap_or("").to_string();
    let info_pass = o["password"].as_str().unwrap_or("").to_string();
    let info_db = o["database"].as_str().unwrap_or("").to_string();
    let _ = agent().post(&format!("{base}/api/tunnel/close?id={id}")).set("Authorization", auth).send_bytes(&[]);
    let lst = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = lst.local_addr().map_err(|e| e.to_string())?.port();
    let (base, auth) = (base.to_string(), auth.to_string());
    std::thread::spawn(move || {
        for c in lst.incoming().flatten() {
            let (b, a) = (base.clone(), auth.clone());
            std::thread::spawn(move || relay(c, b, a));
        }
    });
    Ok(DbInfo { port, user: info_user, password: info_pass, database: info_db })
}

fn relay(local: std::net::TcpStream, base: String, auth: String) {
    use std::io::{Read, Write};
    let id = match call(&base, &auth, Some(&json!({})), "/api/tunnel/open") {
        Ok(o) => o["id"].as_str().unwrap_or("").to_string(),
        Err(_) => return,
    };
    let _ = local.set_nodelay(true);
    let ag = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(6)).timeout_read(Duration::from_secs(40)).timeout_write(Duration::from_secs(60)).build();
    // from the server to the database client
    let (l2, base2, auth2, id2, ag2) = (local.try_clone(), base.clone(), auth.clone(), id.clone(), ag.clone());
    if let Ok(mut l2) = l2 {
        std::thread::spawn(move || {
            let mut fails = 0;
            loop {
                match ag2.get(&format!("{base2}/api/tunnel/recv?id={id2}&wait=8000")).set("Authorization", &auth2).call() {
                    Ok(r) => {
                        fails = 0;
                        let closed = r.header("X-Closed") == Some("1");
                        let mut b = Vec::new();
                        let _ = r.into_reader().read_to_end(&mut b);
                        rx(b.len() as u64 + 200);
                        if !b.is_empty() && l2.write_all(&b).is_err() {
                            break;
                        }
                        if closed {
                            break;
                        }
                    }
                    Err(ureq::Error::Status(404, _)) => break,
                    Err(_) => {
                        fails += 1;
                        if fails > 10 {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(300));
                    }
                }
            }
            let _ = l2.shutdown(std::net::Shutdown::Both);
        });
    }
    // from the database client to the server
    let mut l = local;
    let mut buf = vec![0u8; 65536];
    loop {
        match l.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                tx(n as u64 + 200);
                if ag.post(&format!("{base}/api/tunnel/send?id={id}")).set("Authorization", &auth).send_bytes(&buf[..n]).is_err() {
                    break;
                }
            }
        }
    }
    let _ = ag.post(&format!("{base}/api/tunnel/close?id={id}")).set("Authorization", &auth).send_bytes(&[]);
    let _ = l.shutdown(std::net::Shutdown::Both);
}

/// The connection quality is kept in client.json (next to the folders of the servers).
pub fn save_bw(base: &Path) {
    if let Some(f) = base.parent().and_then(|p| p.parent()).map(|p| p.join("client.json")) {
        if let Ok(t) = std::fs::read_to_string(&f) {
            if let Ok(mut v) = serde_json::from_str::<Value>(&t) {
                v["bw"] = json!(bw_mode());
                let _ = std::fs::write(&f, v.to_string());
            }
        }
    }
}

/// (number of files, bytes) of the cache folder of this client.
pub fn cache_size(dir: &Path) -> (usize, u64) {
    let mut n = (0usize, 0u64);
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if let Ok(m) = e.metadata() {
                if m.is_file() {
                    n.0 += 1;
                    n.1 += m.len();
                }
            }
        }
    }
    n
}

/// Deletes everything in the cache folder; returns the freed bytes.
pub fn cache_clear(dir: &Path) -> u64 {
    let mut freed = 0;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let len = e.metadata().map(|m| m.len()).unwrap_or(0);
            if e.path().is_file() && std::fs::remove_file(e.path()).is_ok() {
                freed += len;
            }
        }
    }
    freed
}

/// Keeps the cache small: removes the files that were not touched for `max_age` and then the oldest ones until the folder is
/// under `max_bytes`. Files of the last 10 minutes are never removed (they may be in use).
pub fn cache_clean(dir: &Path, max_bytes: u64, max_age: Duration) {
    let now = std::time::SystemTime::now();
    let mut files: Vec<(PathBuf, u64, Duration)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if let Ok(m) = e.metadata() {
                if m.is_file() {
                    let age = m.modified().ok().and_then(|t| now.duration_since(t).ok()).unwrap_or(Duration::ZERO);
                    files.push((e.path(), m.len(), age));
                }
            }
        }
    }
    let recent = Duration::from_secs(600);
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.retain(|f| {
        if f.2 > max_age && f.2 > recent && std::fs::remove_file(&f.0).is_ok() {
            total = total.saturating_sub(f.1);
            false
        } else {
            true
        }
    });
    files.sort_by(|a, b| b.2.cmp(&a.2)); // the oldest first
    for f in files {
        if total <= max_bytes {
            break;
        }
        if f.2 > recent && std::fs::remove_file(&f.0).is_ok() {
            total = total.saturating_sub(f.1);
        }
    }
}

static LAST_CLEAN: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);

/// At start and then every 30 minutes: files older than a day are removed and the cache is kept under 1 GB.
pub fn auto_clean(dir: PathBuf) {
    let due = match LAST_CLEAN.lock() {
        Ok(mut g) => {
            let d = g.map(|t| t.elapsed() > Duration::from_secs(1800)).unwrap_or(true);
            if d {
                *g = Some(Instant::now());
            }
            d
        }
        Err(_) => false,
    };
    if due {
        std::thread::spawn(move || cache_clean(&dir, 1_000_000_000, Duration::from_secs(24 * 3600)));
    }
}

/// GET of the server; the answer is read completely.
pub fn http_get(link: &(String, String), q: &str, secs: u64) -> Option<Vec<u8>> {
    let resp = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(5)).timeout_read(Duration::from_secs(secs)).build().get(&format!("{}{q}", link.0)).set("Authorization", &link.1).call().ok()?;
    let mut v = Vec::new();
    std::io::Read::read_to_end(&mut resp.into_reader(), &mut v).ok()?;
    rx(v.len() as u64);
    Some(v)
}

/// The waveform of a song is made by the server (2 bytes per point).
pub fn spawn_wave(link: (String, String), id: u64, store: crate::waveform::WaveStore) {
    if let Ok(mut g) = store.lock() {
        *g = (id, Arc::new(Vec::new()));
    }
    std::thread::spawn(move || {
        let Some(b) = http_get(&link, &format!("/api/wave?id={id}"), 120) else { return };
        let peaks: Vec<f32> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]) as f32 / 65535.0).collect();
        if let Ok(mut g) = store.lock() {
            if g.0 == id {
                g.1 = Arc::new(peaks);
            }
        }
    });
}

/// Auto cue on the server: (id, mix in, mix out, duration) arrives in `out`.
pub fn spawn_autocue(link: (String, String), id: u64, db: f32, out: Arc<Mutex<Option<(u64, u64, u64, u64)>>>) {
    std::thread::spawn(move || {
        let Some(b) = http_get(&link, &format!("/api/analyze?id={id}&what=cue&db={db}"), 300) else { return };
        let Ok(v) = serde_json::from_slice::<Value>(&b) else { return };
        if let Ok(mut g) = out.lock() {
            *g = Some((id, v["a"].as_u64().unwrap_or(0), v["b"].as_u64().unwrap_or(0), v["d"].as_u64().unwrap_or(0)));
        }
    });
}

/// The tags of the file of a library item, read by the server.
pub fn fetch_tags(link: &(String, String), id: u64) -> Option<Value> {
    serde_json::from_slice(&http_get(link, &format!("/api/tags?id={id}"), 30)?).ok()
}
