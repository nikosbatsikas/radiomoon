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

fn authorized(header: Option<&str>, user: &str, pass: &str) -> bool {
    use base64::Engine;
    let Some(h) = header else { return false };
    let Some(b) = h.strip_prefix("Basic ").or_else(|| h.strip_prefix("basic ")) else { return false };
    let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(b.trim()) else { return false };
    let s = String::from_utf8_lossy(&raw).to_string();
    s == format!("{user}:{pass}")
}

/// Starts the listener and returns the receiving end of the requests; the caller answers them with [`handle`].
pub fn spawn(bind: &str, user: String, pass: String) -> anyhow::Result<Receiver<Req>> {
    let server = tiny_http::Server::http(bind).map_err(|e| anyhow::anyhow!("cannot listen on {bind}: {e}"))?;
    let server = std::sync::Arc::new(server);
    let (tx, rx) = channel::<Req>();
    for _ in 0..4 {
        let server = server.clone();
        let tx = tx.clone();
        let (user, pass) = (user.clone(), pass.clone());
        std::thread::spawn(move || {
            for mut rq in server.incoming_requests() {
                let auth = rq.headers().iter().find(|h| h.field.equiv("Authorization")).map(|h| h.value.as_str().to_string());
                if !authorized(auth.as_deref(), &user, &pass) {
                    let r = tiny_http::Response::from_string("Login required")
                        .with_status_code(401)
                        .with_header(tiny_http::Header::from_bytes(&b"WWW-Authenticate"[..], &b"Basic realm=\"RadioMoon\""[..]).unwrap());
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
                let req = Req { method: rq.method().as_str().to_string(), path, query, body, reply: rtx };
                if tx.send(req).is_err() {
                    break;
                }
                let resp = rrx.recv_timeout(std::time::Duration::from_secs(10)).unwrap_or_else(|_| Resp::err(504, "busy"));
                let r = tiny_http::Response::from_data(resp.body)
                    .with_status_code(resp.status)
                    .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], resp.ctype.as_bytes()).unwrap())
                    .with_header(tiny_http::Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap());
                let _ = rq.respond(r);
            }
        });
    }
    Ok(rx)
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
    let hist: Vec<Value> = app.history.iter().rev().take(30).map(|(a, b)| json!({ "time": a, "label": b })).collect();
    json!({
        "version": VERSION,
        "station": app.data.s.station,
        "lang": app.data.s.language,
        "now": now,
        "current": app.current,
        "elapsed": elapsed,
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
        _ => return Resp::err(400, "unknown action"),
    }
    Resp::json(state(app))
}

/// Answers one request; runs on the main thread.
pub fn handle(app: &mut App, r: &Req) -> Resp {
    let post = r.method == "POST";
    let body: Value = if post && !r.body.is_empty() { serde_json::from_slice(&r.body).unwrap_or(Value::Null) } else { Value::Null };
    match (r.method.as_str(), r.path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => Resp { status: 200, ctype: "text/html; charset=utf-8", body: INDEX.as_bytes().to_vec() },
        ("GET", "/api/state") => Resp::json(state(app)),
        ("GET", "/api/library") => {
            let mut v = library(app, &r.query);
            v["kinds"] = kinds(app);
            Resp::json(v)
        }
        ("GET", "/api/pallets") => Resp::json(pallets(app)),
        ("POST", "/api/action") => action(app, &body),
        _ => Resp::err(404, "not found"),
    }
}
