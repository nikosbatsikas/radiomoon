//! Web studio for the headless server: a small HTTP server (worker threads) whose requests are answered on the main thread,
//! because the audio output and the application state live there.
use crate::app::{fmt_ms, App, Mode, VERSION};
use crate::studio::Action;
use serde_json::{json, Value};
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct Req {
    pub method: String,
    pub path: String,
    pub query: String,
    pub body: Vec<u8>,
    pub auth: Option<String>,
    pub reply: Sender<Resp>,
}

pub struct Resp {
    pub status: u16,
    pub ctype: &'static str,
    pub body: Vec<u8>,
}

impl Resp {
    fn json(v: Value) -> Resp {
        Resp { status: 200, ctype: "application/json; charset=utf-8", body: v.to_string().into_bytes() }
    }
    fn err(status: u16, msg: &str) -> Resp {
        Resp { status, ctype: "application/json; charset=utf-8", body: json!({ "error": msg }).to_string().into_bytes() }
    }
}

const INDEX: &str = include_str!("../web/index.html");

/// Credentials of the web studio (file `webserver.json` in the data folder). The first start creates it with a random password.
pub fn credentials(data: &std::path::Path) -> (String, String, bool) {
    let f = data.join("webserver.json");
    if let Ok(t) = std::fs::read_to_string(&f) {
        if let Ok(v) = serde_json::from_str::<Value>(&t) {
            let u = v["user"].as_str().unwrap_or("admin").to_string();
            let p = v["password"].as_str().unwrap_or("").to_string();
            if !p.is_empty() {
                return (u, p, false);
            }
        }
    }
    use rand::Rng;
    let chars: Vec<char> = "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789".chars().collect();
    let mut rng = rand::thread_rng();
    let pw: String = (0..14).map(|_| chars[rng.gen_range(0..chars.len())]).collect();
    let _ = std::fs::create_dir_all(data);
    let _ = std::fs::write(&f, serde_json::to_string_pretty(&json!({ "user": "admin", "password": pw })).unwrap_or_default());
    ("admin".into(), pw, true)
}

/// Who is calling: the web studio login of `webserver.json` (administrator) or a RadioMoon user with the password of the program.
/// The permissions of that user then apply to the actions, exactly as in the window.
fn who(app: &App, auth: &Option<String>, creds: &(String, String)) -> Option<crate::model::User> {
    use base64::Engine;
    let h = auth.as_deref()?;
    let b = h.strip_prefix("Basic ").or_else(|| h.strip_prefix("basic "))?;
    let raw = base64::engine::general_purpose::STANDARD.decode(b.trim()).ok()?;
    let s = String::from_utf8_lossy(&raw).to_string();
    let (u, p) = s.split_once(':')?;
    if u == creds.0 && p == creds.1 {
        return Some(crate::model::User { name: u.to_string(), admin: true, ..Default::default() });
    }
    if app.data.s.users_enabled {
        let name = u.trim().to_lowercase();
        return app.data.users.iter().find(|x| x.name.to_lowercase() == name && crate::app::hash_pw(&x.salt, p) == x.hash).cloned();
    }
    None
}

/// Starts the listener and returns the receiving end of the requests; the caller answers them with [`handle`].
pub fn spawn(bind: &str) -> anyhow::Result<Receiver<Req>> {
    let server = tiny_http::Server::http(bind).map_err(|e| anyhow::anyhow!("cannot listen on {bind}: {e}"))?;
    let server = std::sync::Arc::new(server);
    let (tx, rx) = channel::<Req>();
    for _ in 0..4 {
        let server = server.clone();
        let tx = tx.clone();
        std::thread::spawn(move || {
            for mut rq in server.incoming_requests() {
                let auth = rq.headers().iter().find(|h| h.field.equiv("Authorization")).map(|h| h.value.as_str().to_string());
                if auth.is_none() {
                    let r = tiny_http::Response::from_string("Login required")
                        .with_status_code(401)
                        .with_header(tiny_http::Header::from_bytes(&b"WWW-Authenticate"[..], &b"Basic realm=\"RadioMoon\""[..]).unwrap());
                    let _ = rq.respond(r);
                    continue;
                }
                // an upload is written to the disk by this thread (it can be big); the main thread only decides where and imports it afterwards
                if rq.method().as_str() == "POST" && rq.url().starts_with("/api/upload") {
                    let url = rq.url().to_string();
                    let query = url.split_once('?').map(|x| x.1.to_string()).unwrap_or_default();
                    let ask = |path: &str, query: String| -> Resp {
                        let (rtx, rrx) = channel();
                        let req = Req { method: "POST".into(), path: path.into(), query, body: Vec::new(), auth: auth.clone(), reply: rtx };
                        if tx.send(req).is_err() {
                            return Resp::err(500, "server is stopping");
                        }
                        rrx.recv_timeout(std::time::Duration::from_secs(120)).unwrap_or_else(|_| Resp::err(504, "busy"))
                    };
                    let mut resp = ask("/api/upload", query.clone());
                    if resp.status == 200 {
                        let dest = serde_json::from_slice::<Value>(&resp.body).ok().and_then(|v| v["dest"].as_str().map(|x| x.to_string())).unwrap_or_default();
                        let part = format!("{dest}.part");
                        let wrote = std::fs::File::create(&part).and_then(|mut f| std::io::copy(&mut rq.as_reader(), &mut f).map(|_| ())).and_then(|_| std::fs::rename(&part, &dest));
                        resp = match wrote {
                            Ok(()) => ask("/api/imported", format!("path={}&{}", enc_q(&dest), query)),
                            Err(e) => {
                                let _ = std::fs::remove_file(&part);
                                Resp::err(500, &format!("cannot write the file: {e}"))
                            }
                        };
                    } else if resp.status == 401 {
                        std::thread::sleep(std::time::Duration::from_millis(1200));
                    }
                    let mut r = tiny_http::Response::from_data(resp.body)
                        .with_status_code(resp.status)
                        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], resp.ctype.as_bytes()).unwrap());
                    if resp.status == 401 {
                        r = r.with_header(tiny_http::Header::from_bytes(&b"WWW-Authenticate"[..], &b"Basic realm=\"RadioMoon\""[..]).unwrap());
                    }
                    let _ = rq.respond(r);
                    continue;
                }
                let mut body = Vec::new();
                let _ = std::io::Read::read_to_end(&mut std::io::Read::take(rq.as_reader(), 1 << 20), &mut body);
                let url = rq.url().to_string();
                let (path, query) = match url.split_once('?') {
                    Some((p, q)) => (p.to_string(), q.to_string()),
                    None => (url.clone(), String::new()),
                };
                let (rtx, rrx) = channel();
                let req = Req { method: rq.method().as_str().to_string(), path, query, body, auth, reply: rtx };
                if tx.send(req).is_err() {
                    break;
                }
                let resp = rrx.recv_timeout(std::time::Duration::from_secs(10)).unwrap_or_else(|_| Resp::err(504, "busy"));
                if resp.status == 401 {
                    std::thread::sleep(std::time::Duration::from_millis(1200)); // slows down password guessing
                }
                let mut r = tiny_http::Response::from_data(resp.body)
                    .with_status_code(resp.status)
                    .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], resp.ctype.as_bytes()).unwrap())
                    .with_header(tiny_http::Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap());
                if resp.status == 401 {
                    r = r.with_header(tiny_http::Header::from_bytes(&b"WWW-Authenticate"[..], &b"Basic realm=\"RadioMoon\""[..]).unwrap());
                }
                let _ = rq.respond(r);
            }
        });
    }
    Ok(rx)
}

fn enc_q(s: &str) -> String {
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

fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let h = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                match h {
                    Some(v) => {
                        out.push(v);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn qget(q: &str, key: &str) -> String {
    q.split('&').filter_map(|p| p.split_once('=')).find(|(k, _)| *k == key).map(|(_, v)| unescape(v)).unwrap_or_default()
}

fn mode_name(m: Mode) -> &'static str {
    match m {
        Mode::Auto => "auto",
        Mode::Manual => "manual",
        Mode::Offline => "offline",
    }
}

fn track_json(t: &crate::model::Track) -> Value {
    json!({ "id": t.id, "kind": t.kind, "label": t.label(), "artist": t.artist, "title": t.title, "dur": t.duration_ms, "durs": fmt_ms(t.duration_ms) })
}

fn log_rev(app: &App) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for t in &app.log {
        t.id.hash(&mut h);
        t.src.hash(&mut h);
    }
    h.finish()
}

fn state(app: &App) -> Value {
    let (elapsed, paused, volume, live) = app
        .player
        .as_ref()
        .map(|p| (p.elapsed_ms(), p.is_paused(), p.volume, p.live_active()))
        .unwrap_or((0, false, 1.0, false));
    let starts = app.starts();
    let from = app.current.unwrap_or(0).min(app.log.len());
    let log: Vec<Value> = app
        .log
        .iter()
        .enumerate()
        .skip(from)
        .take(300)
        .map(|(i, t)| {
            let mut v = track_json(t);
            v["i"] = json!(i);
            v["start"] = json!(starts.get(i).map(|s| s.format("%H:%M:%S").to_string()).unwrap_or_default());
            v["src"] = json!(t.src);
            v
        })
        .collect();
    let now = app.current.and_then(|c| app.log.get(c)).map(track_json);
    let hist: Vec<Value> = app.history.iter().take(30).map(|(a, b)| json!({ "time": a, "label": b })).collect();
    json!({
        "version": VERSION,
        "station": app.data.s.station,
        "lang": app.data.s.language,
        "now": now,
        "current": app.current,
        "elapsed": elapsed,
        "pos": app.player.as_ref().map(|p| p.pos_ms()).unwrap_or(0),
        "log_rev": log_rev(app),
        "paused": paused,
        "volume": volume,
        "loop": app.loop_current,
        "stopnext": app.stop_next,
        "mode": mode_name(app.mode),
        "live": live,
        "status": app.status,
        "log": log,
        "history": hist,
        "audio": app.player.is_some(),
    })
}

fn library(app: &App, q: &str) -> Value {
    let text = qget(q, "q").to_lowercase();
    let kind = qget(q, "kind");
    let limit: usize = qget(q, "limit").parse().unwrap_or(100).min(500);
    let kind = if kind.is_empty() { "music".to_string() } else { kind };
    let words: Vec<&str> = text.split_whitespace().collect();
    let items: Vec<Value> = app
        .data
        .tracks
        .iter()
        .filter(|t| t.kind == kind && t.enabled)
        .filter(|t| {
            if words.is_empty() {
                return true;
            }
            let hay = format!("{} {} {} {}", t.artist, t.title, t.album, t.genre).to_lowercase();
            words.iter().all(|w| hay.contains(w))
        })
        .take(limit)
        .map(track_json)
        .collect();
    json!({ "items": items, "total": app.data.tracks.iter().filter(|t| t.kind == kind).count() })
}

fn pallets(app: &App) -> Value {
    let list: Vec<Value> = app
        .data
        .pallets
        .iter()
        .map(|p| {
            let btns: Vec<Value> = p
                .buttons
                .iter()
                .filter(|b| b.track_id != 0)
                .filter_map(|b| {
                    app.data.track(b.track_id).map(|t| {
                        let label = if b.label.is_empty() { t.label() } else { b.label.clone() };
                        json!({ "id": t.id, "label": label, "color": b.color })
                    })
                })
                .collect();
            json!({ "name": p.name, "buttons": btns })
        })
        .collect();
    json!({ "pallets": list })
}

fn kinds(app: &App) -> Value {
    let mut k: Vec<String> = Vec::new();
    for t in &app.data.tracks {
        if !k.contains(&t.kind) {
            k.push(t.kind.clone());
        }
    }
    json!(k)
}

fn action(app: &mut App, body: &Value) -> Resp {
    let name = body["do"].as_str().unwrap_or("");
    let idx = body["index"].as_u64().map(|v| v as usize);
    match name {
        "next" => app.handle(Action::Next),
        "stop" => app.handle(Action::Stop),
        "pause" => app.handle(Action::Pause),
        "restart" => app.handle(Action::Restart),
        "loop" => app.handle(Action::ToggleLoop),
        "stopnext" => app.handle(Action::StopNext),
        "regen" => app.handle(Action::RegenAuto),
        "clear" => app.handle(Action::ClearLog),
        "live" => app.handle(Action::ToggleLive),
        "stopjingles" => app.handle(Action::StopJingles),
        "mode" => {
            let m = match body["mode"].as_str().unwrap_or("") {
                "auto" => Mode::Auto,
                "manual" => Mode::Manual,
                "offline" => Mode::Offline,
                _ => return Resp::err(400, "unknown mode"),
            };
            app.handle(Action::SetMode(m));
        }
        "volume" => {
            let v = body["value"].as_f64().unwrap_or(1.0).clamp(0.0, 1.5) as f32;
            app.handle(Action::Volume(v));
        }
        "play" => match idx {
            Some(i) if i < app.log.len() => app.handle(Action::PlayIndex(i)),
            _ => return Resp::err(400, "bad index"),
        },
        "remove" => match idx {
            Some(i) if i < app.log.len() => app.handle(Action::RemoveLog(i)),
            _ => return Resp::err(400, "bad index"),
        },
        "up" | "down" => match idx {
            Some(i) if i < app.log.len() => {
                app.handle(Action::Select(i));
                app.handle(if name == "up" { Action::MoveItemUp } else { Action::MoveItemDown });
            }
            _ => return Resp::err(400, "bad index"),
        },
        "jingle" => match body["id"].as_u64() {
            Some(id) => app.handle(Action::Jingle(id)),
            None => return Resp::err(400, "bad id"),
        },
        "add" => {
            let Some(id) = body["id"].as_u64() else { return Resp::err(400, "bad id") };
            let kind = app.data.track(id).map(|t| t.kind.clone()).unwrap_or_default();
            app.lib_kind = match kind.as_str() {
                "sweeper" => "sweeper",
                "voice" => "voice",
                _ => "music",
            };
            app.lib_sel = Some(id);
            app.handle(if body["where"].as_str() == Some("next") { Action::InsertSel } else { Action::AddSel });
        }
        "reload" => app.reload_all(),
        "startlive" => match idx {
            Some(i) => app.handle(Action::StartLive(i)),
            None => return Resp::err(400, "bad index"),
        },
        _ => return Resp::err(400, "unknown action"),
    }
    Resp::json(state(app))
}

/// Answers one request; runs on the main thread.
pub fn handle(app: &mut App, r: &Req, creds: &(String, String)) -> Resp {
    let Some(user) = who(app, &r.auth, creds) else { return Resp::err(401, "wrong user name or password") };
    let (uname, uadmin) = (user.name.clone(), user.admin);
    app.user = Some(user);
    let post = r.method == "POST";
    let body: Value = if post && !r.body.is_empty() { serde_json::from_slice(&r.body).unwrap_or(Value::Null) } else { Value::Null };
    match (r.method.as_str(), r.path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => Resp { status: 200, ctype: "text/html; charset=utf-8", body: INDEX.as_bytes().to_vec() },
        ("GET", "/api/login") => Resp::json(json!({ "user": uname, "admin": uadmin, "version": VERSION })),
        ("GET", "/api/state") => Resp::json(state(app)),
        ("GET", "/api/library") => {
            let mut v = library(app, &r.query);
            v["kinds"] = kinds(app);
            Resp::json(v)
        }
        ("GET", "/api/pallets") => Resp::json(pallets(app)),
        ("POST", "/api/action") => action(app, &body),
        ("GET", "/api/audio") => audio(app, &r.query),
        ("POST", "/api/upload") => upload_target(app, &r.query),
        ("POST", "/api/imported") => imported(app, &r.query),
        _ => Resp::err(404, "not found"),
    }
}

/// The audio file of a library item (the studio on another computer listens to it).
fn audio(app: &App, q: &str) -> Resp {
    let id: u64 = qget(q, "id").parse().unwrap_or(0);
    let Some(t) = app.data.track(id) else { return Resp::err(404, "no such item") };
    let path = app.map_path(&t.path);
    match std::fs::read(&path) {
        Ok(b) => {
            let ctype = match std::path::Path::new(&path).extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
                Some("mp3") => "audio/mpeg",
                Some("ogg") | Some("opus") => "audio/ogg",
                Some("flac") => "audio/flac",
                Some("wav") => "audio/wav",
                Some("m4a") | Some("aac") => "audio/mp4",
                _ => "application/octet-stream",
            };
            Resp { status: 200, ctype, body: b }
        }
        Err(_) => Resp::err(404, "the file is not on the server"),
    }
}

fn clean_name(n: &str) -> String {
    let n = n.rsplit(|c| c == '/' || c == '\\').next().unwrap_or("");
    let n: String = n.chars().map(|c| if c.is_control() || "<>:\"|?*".contains(c) { '_' } else { c }).collect();
    n.trim().trim_start_matches('.').to_string()
}

/// First step of an upload: where the file is written (a folder of the data folder of the server).
fn upload_target(app: &mut App, q: &str) -> Resp {
    let name = clean_name(&qget(q, "name"));
    let ext_ok = std::path::Path::new(&name).extension().map(|x| crate::library::EXTS.contains(&x.to_string_lossy().to_lowercase().as_str())).unwrap_or(false);
    if name.is_empty() || !ext_ok {
        return Resp::err(400, "not an audio file");
    }
    let kind = { let k = qget(q, "kind"); if k.is_empty() { "music".to_string() } else { k } };
    let kind: String = kind.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let perm = match kind.as_str() {
        "spot" => "lib_spots",
        "jingle" => "lib_jingles",
        "asset" => "lib_assets",
        "sweeper" => "lib_sweepers",
        "voice" => "lib_voice",
        _ => "lib_songs",
    };
    if !app.can(perm) {
        return Resp::err(403, "You are not allowed to add items to this library");
    }
    let dir = app.base.join("media").join(&kind);
    if std::fs::create_dir_all(&dir).is_err() {
        return Resp::err(500, "cannot create the media folder");
    }
    let mut dest = dir.join(&name);
    let mut n = 1;
    while dest.exists() {
        let stem = std::path::Path::new(&name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ext = std::path::Path::new(&name).extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        dest = dir.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    Resp::json(json!({ "dest": dest.to_string_lossy() }))
}

/// Second step: the uploaded file is read (tags, duration) and added to the library.
fn imported(app: &mut App, q: &str) -> Resp {
    let path = qget(q, "path");
    let p = std::path::Path::new(&path);
    if !p.starts_with(app.base.join("media")) || !p.is_file() {
        return Resp::err(400, "bad file");
    }
    let kind = qget(q, "kind");
    let b = |k: &str| qget(q, k) == "true";
    let o = crate::library::ImportOpts {
        kind: if kind.is_empty() { "music".into() } else { kind },
        cat1: qget(q, "cat1"),
        autocue: b("autocue"),
        autocue_db: qget(q, "autocue_db").parse().unwrap_or(-30.0),
        bookmark: b("bookmark"),
        gain_db: qget(q, "gain").parse().unwrap_or(0.0),
        read_tags: b("read_tags"),
        parse_filename: b("parse_filename"),
    };
    if crate::library::import_one(&app.db, p, &o) {
        app.reload_tracks();
        Resp::json(json!({ "ok": true }))
    } else {
        Resp::err(500, "the file could not be added to the library")
    }
}
