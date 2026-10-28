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
        let x = app.data.users.iter().find(|x| x.name.to_lowercase() == name)?;
        if x.mfa.is_empty() {
            return (crate::app::hash_pw(&x.salt, p) == x.hash).then(|| x.clone());
        }
        // two-step login: the password is followed by the 6 digit code of the authenticator app
        let (pw, code) = crate::totp::split_code(p)?;
        if crate::app::hash_pw(&x.salt, pw) != x.hash {
            return None;
        }
        return mfa_check(&name, h, &x.mfa, code).then(|| x.clone());
    }
    None
}

/// Checked sign-ins stay valid for 12 hours (the browser and the client send the same credentials with every request, and a code is only
/// valid for 30 seconds). After 5 wrong codes of a user the sign-in is locked for 5 minutes.
static MFA_OK: std::sync::Mutex<Option<std::collections::HashMap<String, u64>>> = std::sync::Mutex::new(None);
static MFA_FAIL: std::sync::Mutex<Option<std::collections::HashMap<String, (u32, u64)>>> = std::sync::Mutex::new(None);

fn mfa_check(user: &str, header: &str, secret: &str, code: &str) -> bool {
    let now = crate::totp::now();
    let (Ok(mut ok), Ok(mut fail)) = (MFA_OK.lock(), MFA_FAIL.lock()) else { return false };
    let ok = ok.get_or_insert_with(Default::default);
    let fail = fail.get_or_insert_with(Default::default);
    if ok.get(header).map(|t| now < *t).unwrap_or(false) {
        return true;
    }
    if let Some((n, until)) = fail.get(user) {
        if *n >= 5 && now < *until {
            return false;
        }
    }
    if crate::totp::verify(secret, code, now) {
        fail.remove(user);
        if ok.len() > 500 {
            ok.retain(|_, t| now < *t);
        }
        if ok.len() > 500 {
            ok.clear();
        }
        ok.insert(header.to_string(), now + 12 * 3600);
        return true;
    }
    let e = fail.entry(user.to_string()).or_insert((0, 0));
    if e.0 >= 5 && now >= e.1 {
        e.0 = 0;
    }
    e.0 += 1;
    e.1 = now + 300;
    false
}

/// Starts the listener and returns the receiving end of the requests; the caller answers them with [`handle`].
pub fn spawn(bind: &str) -> anyhow::Result<Receiver<Req>> {
    let server = tiny_http::Server::http(bind).map_err(|e| anyhow::anyhow!("cannot listen on {bind}: {e}"))?;
    let server = std::sync::Arc::new(server);
    let (tx, rx) = channel::<Req>();
    // some workers wait for the data of a database tunnel of a client
    for _ in 0..32 {
        let server = server.clone();
        let tx = tx.clone();
        std::thread::spawn(move || {
            for mut rq in server.incoming_requests() {
                let auth = rq.headers().iter().find(|h| h.field.equiv("Authorization")).map(|h| h.value.as_str().to_string());
                if auth.is_none() {
                    // the page with the sign-in form is public (it holds nothing); the rest needs the sign-in. There is no WWW-Authenticate
                    // header: the page asks for the user name, the password and the code itself, not the password box of the browser.
                    let is_page = rq.method().as_str() == "GET" && (rq.url() == "/" || rq.url() == "/index.html");
                    let r = if is_page {
                        tiny_http::Response::from_string(INDEX)
                            .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap())
                            .with_header(tiny_http::Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap())
                    } else {
                        tiny_http::Response::from_string("Login required").with_status_code(401)
                    };
                    let _ = rq.respond(r);
                    continue;
                }
                if let Some(rest) = rq.url().strip_prefix("/api/tunnel/") {
                    if !rest.starts_with("open") {
                        let rest = rest.to_string();
                        tunnel_http(rq, &rest);
                        continue;
                    }
                }
                // heavy work on a file (sending it, the waveform, the analysis) is done by this thread: the main thread only gives the path
                if rq.method().as_str() == "GET" && ["/api/audio", "/api/wave", "/api/analyze", "/api/tags"].iter().any(|p| rq.url() == *p || rq.url().starts_with(&format!("{p}?"))) {
                    let url = rq.url().to_string();
                    heavy_http(rq, &url, auth.clone(), &tx);
                    continue;
                }
                // the database of a program without a server (a backup file) is written to the disk by this thread; the main thread restores it
                if rq.method().as_str() == "POST" && rq.url() == "/api/restore" {
                    let ask = |path: &str, query: String| -> Resp {
                        let (rtx, rrx) = channel();
                        let req = Req { method: "POST".into(), path: path.into(), query, body: Vec::new(), auth: auth.clone(), reply: rtx };
                        if tx.send(req).is_err() {
                            return Resp::err(500, "server is stopping");
                        }
                        rrx.recv_timeout(std::time::Duration::from_secs(900)).unwrap_or_else(|_| Resp::err(504, "busy"))
                    };
                    let mut resp = ask("/api/restore_target", String::new());
                    if resp.status == 200 {
                        let dest = serde_json::from_slice::<Value>(&resp.body).ok().and_then(|v| v["dest"].as_str().map(|x| x.to_string())).unwrap_or_default();
                        let wrote = std::fs::File::create(&dest).and_then(|mut f| std::io::copy(&mut rq.as_reader(), &mut f).map(|_| ()));
                        resp = match wrote {
                            Ok(()) => ask("/api/restore_run", format!("path={}", enc_q(&dest))),
                            Err(e) => Resp::err(500, &format!("cannot write the file: {e}")),
                        };
                        let _ = std::fs::remove_file(&dest);
                    } else if resp.status == 401 {
                        std::thread::sleep(std::time::Duration::from_millis(1200));
                    }
                    let r = tiny_http::Response::from_data(resp.body)
                        .with_status_code(resp.status)
                        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], resp.ctype.as_bytes()).unwrap());
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
                    let r = tiny_http::Response::from_data(resp.body)
                        .with_status_code(resp.status)
                        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], resp.ctype.as_bytes()).unwrap());
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
        t.overlay_pos.hash(&mut h);
        t.fixed_start.hash(&mut h);
        t.kind.hash(&mut h);
    }
    app.log.len().hash(&mut h);
    h.finish()
}

fn state(app: &App, full: bool) -> Value {
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
        .take(if full { 300 } else { 0 })
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
        "data_rev": app.data_rev,
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
        "streams": app.streamers.iter().map(|(i, _, st)| json!({ "i": i, "status": st.status_text(), "listeners": st.listeners() })).collect::<Vec<_>>(),
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
        "reload" => {
            app.reload_all();
            app.clap_apply_settings();
        }
        // the sound outputs were changed in the settings by a client: the server opens the chosen sound cards
        "apply_outputs" => {
            if !app.can("settings") {
                return Resp::err(403, "You are not allowed to change the settings");
            }
            app.reload_all();
            app.apply_outputs();
        }
        "clap_scan" | "clap_add" | "clap_remove" | "clap_enable" | "clap_param" | "clap_move" => {
            use crate::clap_host::ClapOp;
            let u = |k: &str| body[k].as_u64().unwrap_or(0) as usize;
            let op = match name {
                "clap_scan" => ClapOp::Scan,
                "clap_add" => ClapOp::Add(body["path"].as_str().unwrap_or("").to_string(), body["id"].as_str().unwrap_or("").to_string()),
                "clap_remove" => ClapOp::Remove(u("slot")),
                "clap_enable" => ClapOp::Enable(u("slot"), body["on"].as_bool().unwrap_or(true)),
                "clap_param" => ClapOp::Param(u("slot"), body["id"].as_u64().unwrap_or(0) as u32, body["value"].as_f64().unwrap_or(0.0), body["save"].as_bool().unwrap_or(true)),
                _ => ClapOp::Move(u("from"), u("to")),
            };
            if !app.can("settings") {
                return Resp::err(403, "You are not allowed to change the settings");
            }
            if let Err(e) = app.clap_local(op) {
                return Resp::err(400, &e);
            }
        }
        "delete_tracks" => {
            if !app.can("settings") {
                return Resp::err(403, "You are not allowed to delete items of the library");
            }
            let ids: Vec<u64> = body["ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default();
            let m = app.delete_library_items(&ids, body["files"].as_bool().unwrap_or(false));
            return Resp::json(json!({ "ok": true, "message": m }));
        }
        "normalize" => {
            // the analysis of the files runs on the server (the client does not download the songs)
            if !app.can("settings") {
                return Resp::err(403, "You are not allowed to change the volume of the items");
            }
            let ids: Vec<u64> = body["ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default();
            let target = body["target"].as_f64().unwrap_or(0.0) as f32;
            let items: Vec<(u64, String)> = ids.iter().filter_map(|id| app.data.track(*id).map(|t| (*id, app.map_path(&t.path)))).collect();
            let count = items.len();
            let (db, st, done) = (app.db.clone(), app.scan_status.clone(), app.scan_done.clone());
            std::thread::spawn(move || {
                let tracks = db.list_tracks(None).unwrap_or_default();
                let (mut ok, mut failed) = (0, 0);
                for (i, (id, path)) in items.iter().enumerate() {
                    if let Ok(mut s) = st.lock() {
                        *s = format!("Normalizing {} / {}", i + 1, items.len());
                    }
                    match (tracks.iter().find(|t| t.id == *id).cloned(), crate::waveform::peak_db(path)) {
                        (Some(mut t), Some(peak)) => {
                            t.gain_db = target - peak;
                            if db.update_track(&t).is_ok() {
                                ok += 1;
                            } else {
                                failed += 1;
                            }
                        }
                        _ => failed += 1,
                    }
                }
                if let Ok(mut s) = st.lock() {
                    *s = format!("Normalize finished: {ok} items{}", if failed > 0 { format!(", {failed} failed") } else { String::new() });
                }
                done.store(true, std::sync::atomic::Ordering::SeqCst);
            });
            return Resp::json(json!({ "ok": true, "message": format!("Normalizing {count} items on the server") }));
        }
        "stream_start" => {
            app.reload_all();
            app.start_stream();
        }
        "stream_stop" => app.stop_stream(),
        "startlive" => match idx {
            Some(i) => app.handle(Action::StartLive(i)),
            None => return Resp::err(400, "bad index"),
        },
        _ => return Resp::err(400, "unknown action"),
    }
    Resp::json(state(app, body["lite"].as_bool() != Some(true)))
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
        ("GET", "/api/state") => Resp::json(state(app, !r.query.contains("lite"))),
        ("GET", "/api/log") => Resp::json(log_json(app)),
        ("GET", "/api/library") => {
            let mut v = library(app, &r.query);
            v["kinds"] = kinds(app);
            Resp::json(v)
        }
        ("GET", "/api/pallets") => Resp::json(pallets(app)),
        ("POST", "/api/action") => action(app, &body),
        ("GET", "/api/trackpath") => trackpath(app, &r.query),
        ("GET", "/api/outputs") => Resp::json(json!({ "outputs": crate::player::list_outputs() })),
        ("GET", "/api/clap") => Resp::json(crate::clap_host::view_json(&app.clap_view())),
        ("GET", "/api/clients") => clients(app),
        ("GET", p) if p.starts_with("/clients/") => client_file(app, &p["/clients/".len()..]),
        ("POST", "/api/tunnel/open") => tunnel_open(app),
        // the update of the server to the newest release
        ("GET", "/api/update") if uadmin => Resp::json(crate::srvupdate::state(VERSION)),
        ("POST", "/api/update_check") if uadmin => {
            crate::srvupdate::check_async();
            Resp::json(json!({ "ok": true }))
        }
        ("POST", "/api/update_install") if uadmin => match crate::srvupdate::install(VERSION) {
            Ok(()) => Resp::json(json!({ "ok": true })),
            Err(e) => Resp::err(400, &e),
        },
        ("GET", "/api/update") | ("POST", "/api/update_check") | ("POST", "/api/update_install") => Resp::err(403, "Only an administrator can update the server"),
        ("POST", "/api/restore_target") => {
            if !uadmin {
                return Resp::err(403, "Only an administrator can replace the database of the server");
            }
            let dir = app.base.join("restore");
            if std::fs::create_dir_all(&dir).is_err() {
                return Resp::err(500, "cannot create the folder");
            }
            Resp::json(json!({ "dest": dir.join("incoming.rmbak").to_string_lossy() }))
        }
        ("POST", "/api/restore_run") => {
            if !uadmin {
                return Resp::err(403, "Only an administrator can replace the database of the server");
            }
            let path = qget(&r.query, "path");
            let p = std::path::Path::new(&path);
            if !p.starts_with(app.base.join("restore")) || path.contains("..") || !p.is_file() {
                return Resp::err(400, "bad file");
            }
            let bdir = app.backup_dir();
            match crate::backup::import(&app.db, p, VERSION, &bdir) {
                Ok(msg) => {
                    app.reload_all();
                    // what belongs to the other computer: its sound cards, its folders
                    app.data.s.out_main.clear();
                    app.data.s.out_monitor.clear();
                    app.data.s.out_instants.clear();
                    app.data.s.folder_maps.clear();
                    app.data.s.backup.dir = "backups".into();
                    app.save_settings();
                    app.apply_outputs();
                    Resp::json(json!({ "ok": true, "message": msg }))
                }
                Err(e) => Resp::err(500, &format!("{e:#}")),
            }
        }
        ("POST", "/api/upload") => upload_target(app, &r.query),
        ("POST", "/api/imported") => imported(app, &r.query),
        _ => Resp::err(404, "not found"),
    }
}

/// The audio file of a library item (the studio on another computer listens to it).
/// The path of the file of a library item on the server (asked by the worker threads, which do the heavy work).
fn trackpath(app: &App, q: &str) -> Resp {
    let id: u64 = qget(q, "id").parse().unwrap_or(0);
    let Some(t) = app.data.track(id) else { return Resp::err(404, "no such item") };
    Resp::json(json!({ "path": app.map_path(&t.path) }))
}

fn respond_resp(rq: tiny_http::Request, resp: Resp) {
    let r = tiny_http::Response::from_data(resp.body)
        .with_status_code(resp.status)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], resp.ctype.as_bytes()).unwrap());
    let _ = rq.respond(r);
}

static WAVES: std::sync::Mutex<Option<std::collections::HashMap<String, std::sync::Arc<Vec<u8>>>>> = std::sync::Mutex::new(None);

/// /api/audio (the file), /api/wave (its waveform: 2 bytes per point), /api/analyze (peak or auto cue) and /api/tags (the tags of the file).
fn heavy_http(rq: tiny_http::Request, url: &str, auth: Option<String>, tx: &Sender<Req>) {
    let (path_part, query) = url.split_once('?').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((url.to_string(), String::new()));
    let (rtx, rrx) = channel();
    let req = Req { method: "GET".into(), path: "/api/trackpath".into(), query: query.clone(), body: Vec::new(), auth, reply: rtx };
    if tx.send(req).is_err() {
        return respond_resp(rq, Resp::err(500, "server is stopping"));
    }
    let resp = rrx.recv_timeout(std::time::Duration::from_secs(30)).unwrap_or_else(|_| Resp::err(504, "busy"));
    if resp.status != 200 {
        if resp.status == 401 {
            std::thread::sleep(std::time::Duration::from_millis(1200));
        }
        return respond_resp(rq, resp);
    }
    let file = serde_json::from_slice::<Value>(&resp.body).ok().and_then(|v| v["path"].as_str().map(|x| x.to_string())).unwrap_or_default();
    let fp = std::path::Path::new(&file);
    if !fp.is_file() {
        return respond_resp(rq, Resp::err(404, "the file is not on the server"));
    }
    match path_part.as_str() {
        "/api/audio" => {
            let ctype = match fp.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
                Some("mp3") => "audio/mpeg",
                Some("ogg") | Some("opus") => "audio/ogg",
                Some("flac") => "audio/flac",
                Some("wav") => "audio/wav",
                Some("m4a") | Some("aac") => "audio/mp4",
                _ => "application/octet-stream",
            };
            match std::fs::File::open(fp) {
                Ok(f) => {
                    let len = f.metadata().map(|m| m.len() as usize).ok();
                    let r = tiny_http::Response::new(tiny_http::StatusCode(200), vec![tiny_http::Header::from_bytes(&b"Content-Type"[..], ctype.as_bytes()).unwrap()], f, len, None);
                    let _ = rq.respond(r);
                }
                Err(_) => respond_resp(rq, Resp::err(404, "the file is not on the server")),
            }
        }
        "/api/wave" => {
            let key = format!("{file}|{}", fp.metadata().map(|m| m.len()).unwrap_or(0));
            let cached = WAVES.lock().ok().and_then(|g| g.as_ref().and_then(|m| m.get(&key).cloned()));
            let data = match cached {
                Some(d) => Some(d),
                None => crate::waveform::peaks_of(&file).map(|p| {
                    let bytes: Vec<u8> = p.iter().flat_map(|v| ((v.clamp(0.0, 1.0) * 65535.0) as u16).to_le_bytes()).collect();
                    let d = std::sync::Arc::new(bytes);
                    if let Ok(mut g) = WAVES.lock() {
                        let m = g.get_or_insert_with(Default::default);
                        if m.len() > 300 {
                            m.clear();
                        }
                        m.insert(key, d.clone());
                    }
                    d
                }),
            };
            match data {
                Some(d) => respond_resp(rq, Resp { status: 200, ctype: "application/octet-stream", body: (*d).clone() }),
                None => respond_resp(rq, Resp::err(500, "the file cannot be decoded")),
            }
        }
        "/api/analyze" => {
            let db: f32 = qget(&query, "db").parse().unwrap_or(-40.0);
            let out = if qget(&query, "what") == "cue" {
                crate::waveform::autocue(&file, db).map(|(a, b, d)| json!({ "a": a, "b": b, "d": d }))
            } else {
                crate::waveform::peak_db(&file).map(|p| json!({ "peak": p }))
            };
            respond_resp(rq, out.map(Resp::json).unwrap_or_else(|| Resp::err(500, "the file cannot be decoded")));
        }
        _ => {
            let m = crate::library::read_meta(fp, true);
            respond_resp(rq, Resp::json(json!({ "title": m.title, "artist": m.artist, "album": m.album, "genre": m.genre, "year": m.year, "dur": m.dur })));
        }
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
    if qget(q, "kind") == "clap" {
        if !app.can("settings") {
            return Resp::err(403, "You are not allowed to add plugins");
        }
        if !name.to_lowercase().ends_with(".clap") {
            return Resp::err(400, "not a CLAP plugin (.clap)");
        }
        let dir = crate::clap_host::set_plugin_dir(&app.base);
        if std::fs::create_dir_all(&dir).is_err() {
            return Resp::err(500, "cannot create the plugin folder");
        }
        return Resp::json(json!({ "dest": dir.join(&name).to_string_lossy() }));
    }
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
    // one folder for every library: media/songs, media/spots, media/jingles ...
    let folder = match kind.as_str() {
        "music" => "songs",
        "spot" => "spots",
        "jingle" => "jingles",
        "asset" => "assets",
        "sweeper" => "sweepers",
        "voice" => "voice",
        other => other,
    };
    let dir = app.base.join("media").join(folder);
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
    if qget(q, "kind") == "clap" {
        if !p.starts_with(app.base.join("plugins")) || !p.is_file() {
            return Resp::err(400, "bad file");
        }
        let _ = app.clap_local(crate::clap_host::ClapOp::Scan);
        return Resp::json(json!({ "ok": true }));
    }
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
    if let Ok(id) = qget(q, "track_id").parse::<u64>() {
        // an item of the library that was imported from another program: it now points to the copy on the server
        // (the items were added by the client straight to the database: the server reads the library again when it does not know the item)
        let mut found = app.data.track(id).cloned();
        if found.is_none() {
            app.reload_tracks();
            found = app.data.track(id).cloned();
        }
        return match found {
            Some(mut t) => {
                t.path = path.clone();
                if app.db.update_track(&t).is_ok() {
                    if let Some(x) = app.data.tracks.iter_mut().find(|x| x.id == id) {
                        x.path = path.clone();
                    }
                    Resp::json(json!({ "ok": true }))
                } else {
                    Resp::err(500, "the library could not be updated")
                }
            }
            None => Resp::err(404, "no such item"),
        };
    }
    if crate::library::import_one(&app.db, p, &o) {
        app.reload_tracks();
        Resp::json(json!({ "ok": true }))
    } else {
        Resp::err(500, "the file could not be added to the library")
    }
}

/// The RadioMoon clients (Windows, Linux, macOS) that came with the server: the web page offers them for download.
fn clients(app: &App) -> Resp {
    let mut list: Vec<Value> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(app.app_dir.join("clients")) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if let Ok(m) = e.metadata() {
                if m.is_file() {
                    let os = if name.contains("windows7") { "Windows 7" } else if name.contains("windows") { "Windows" } else if name.contains("macos") { "macOS" } else if name.contains("linux") { "Linux" } else { "" };
                    let arch = ["x64", "x86", "arm64"].iter().find(|a| name.contains(&format!("-{a}"))).copied().unwrap_or("");
                    list.push(json!({ "name": name, "size": m.len(), "os": os, "arch": arch }));
                }
            }
        }
    }
    list.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Resp::json(json!({ "clients": list }))
}

fn client_file(app: &App, name: &str) -> Resp {
    let name = unescape(name);
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.is_empty() {
        return Resp::err(400, "bad name");
    }
    match std::fs::read(app.app_dir.join("clients").join(&name)) {
        Ok(b) => Resp { status: 200, ctype: "application/octet-stream", body: b },
        Err(_) => Resp::err(404, "not found"),
    }
}

/// The whole playlist as it is in the memory of the server (the studio of the client shows the same items).
fn log_json(app: &App) -> Value {
    let items: Vec<Value> = app
        .log
        .iter()
        .map(|t| json!({ "t": serde_json::to_value(t).unwrap_or(Value::Null), "src": t.src, "reason": t.reason, "ov": t.overlay_pos, "ovc": t.overlay_custom_ms, "fs": t.fixed_start }))
        .collect();
    json!({ "rev": log_rev(app), "items": items })
}

// ------------------------------------------------------------------ database tunnel
// The client of a server has no connection of its own to the database: it asks the server to open one (to the database of the
// server) and carries the bytes over the same HTTP port. The id of a tunnel is a secret that only a signed-in client receives.
struct Tun {
    w: std::sync::Mutex<std::net::TcpStream>,
    buf: std::sync::Mutex<(Vec<u8>, bool)>,
    cv: std::sync::Condvar,
    last: std::sync::Mutex<std::time::Instant>,
}

fn tunnels() -> &'static std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<Tun>>> {
    static T: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<Tun>>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

fn tunnel_open(app: &mut App) -> Resp {
    use std::io::Read;
    use std::net::ToSocketAddrs;
    {
        // tunnels that nobody uses any more
        let mut g = tunnels().lock().unwrap();
        g.retain(|_, t| t.last.lock().map(|l| l.elapsed().as_secs() < 600).unwrap_or(false));
    }
    let host = if app.cfg.host.trim().is_empty() { "127.0.0.1".to_string() } else { app.cfg.host.clone() };
    let addr = match (host.as_str(), app.cfg.port).to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(a) => a,
        None => return Resp::err(500, "the address of the database is not valid"),
    };
    let sock = match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5)) {
        Ok(s) => s,
        Err(e) => return Resp::err(500, &format!("the server cannot reach its database: {e}")),
    };
    let _ = sock.set_nodelay(true);
    let Ok(mut rd) = sock.try_clone() else { return Resp::err(500, "cannot open the tunnel") };
    use rand::Rng;
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz0123456789".chars().collect();
    let mut rng = rand::thread_rng();
    let id: String = (0..32).map(|_| chars[rng.gen_range(0..chars.len())]).collect();
    let t = std::sync::Arc::new(Tun { w: std::sync::Mutex::new(sock), buf: std::sync::Mutex::new((Vec::new(), false)), cv: std::sync::Condvar::new(), last: std::sync::Mutex::new(std::time::Instant::now()) });
    let t2 = t.clone();
    std::thread::spawn(move || {
        let mut b = vec![0u8; 262144];
        loop {
            match rd.read(&mut b) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let mut g = t2.buf.lock().unwrap();
                    g.0.extend_from_slice(&b[..n]);
                    t2.cv.notify_all();
                }
            }
        }
        let mut g = t2.buf.lock().unwrap();
        g.1 = true;
        t2.cv.notify_all();
    });
    tunnels().lock().unwrap().insert(id.clone(), t);
    Resp::json(json!({ "id": id, "user": app.cfg.user, "password": app.cfg.password, "database": app.cfg.database }))
}

fn tunnel_reply(rq: tiny_http::Request, status: u16, body: Vec<u8>, closed: bool) {
    let mut r = tiny_http::Response::from_data(body)
        .with_status_code(status)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/octet-stream"[..]).unwrap())
        .with_header(tiny_http::Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap());
    if closed {
        r = r.with_header(tiny_http::Header::from_bytes(&b"X-Closed"[..], &b"1"[..]).unwrap());
    }
    let _ = rq.respond(r);
}

/// send / recv / close of a tunnel (answered by the worker thread: recv waits for the data of the database).
fn tunnel_http(mut rq: tiny_http::Request, rest: &str) {
    use std::io::{Read, Write};
    let (op, q) = rest.split_once('?').unwrap_or((rest, ""));
    let id = qget(q, "id");
    let t = tunnels().lock().unwrap().get(&id).cloned();
    let Some(t) = t else { return tunnel_reply(rq, 404, b"no such tunnel".to_vec(), true) };
    if let Ok(mut l) = t.last.lock() {
        *l = std::time::Instant::now();
    }
    match op {
        "send" => {
            let mut b = Vec::new();
            let _ = rq.as_reader().take(64 << 20).read_to_end(&mut b);
            let ok = t.w.lock().map(|mut w| w.write_all(&b).is_ok()).unwrap_or(false);
            tunnel_reply(rq, if ok { 200 } else { 502 }, Vec::new(), !ok);
        }
        "recv" => {
            let wait = qget(q, "wait").parse::<u64>().unwrap_or(8000).min(20000);
            let end = std::time::Instant::now() + std::time::Duration::from_millis(wait);
            let mut g = t.buf.lock().unwrap();
            while g.0.is_empty() && !g.1 {
                let now = std::time::Instant::now();
                if now >= end {
                    break;
                }
                g = t.cv.wait_timeout(g, end - now).unwrap().0;
            }
            // a short wait collects the rest of an answer (a big list of songs): fewer requests
            if !g.0.is_empty() && !g.1 && g.0.len() < (256 << 10) {
                g = t.cv.wait_timeout(g, std::time::Duration::from_millis(3)).unwrap().0;
            }
            let n = g.0.len().min(4 << 20);
            let data: Vec<u8> = g.0.drain(..n).collect();
            let closed = g.1 && g.0.is_empty();
            drop(g);
            tunnel_reply(rq, 200, data, closed);
        }
        "close" => {
            if let Some(t) = tunnels().lock().unwrap().remove(&id) {
                if let Ok(w) = t.w.lock() {
                    let _ = w.shutdown(std::net::Shutdown::Both);
                }
            }
            tunnel_reply(rq, 200, Vec::new(), true);
        }
        _ => tunnel_reply(rq, 404, b"unknown".to_vec(), false),
    }
}
