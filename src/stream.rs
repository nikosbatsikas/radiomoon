//! Internet broadcast: encodes the on-air mix to MP3 and sends it to an Icecast or Shoutcast server.
use crate::model::StreamCfg;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use mp3lame_encoder::{Bitrate, Builder, FlushNoGap, InterleavedPcm, Quality};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Streamer {
    stop: Arc<AtomicBool>,
    pub status: Arc<Mutex<String>>,
    meta_tx: Sender<String>,
    listeners: Arc<AtomicI64>, // -1 = not known
}

fn bitrate(k: u32) -> Bitrate {
    match k {
        0..=48 => Bitrate::Kbps48,
        49..=64 => Bitrate::Kbps64,
        65..=96 => Bitrate::Kbps96,
        97..=112 => Bitrate::Kbps112,
        113..=128 => Bitrate::Kbps128,
        129..=160 => Bitrate::Kbps160,
        161..=192 => Bitrate::Kbps192,
        193..=256 => Bitrate::Kbps256,
        _ => Bitrate::Kbps320,
    }
}

fn urlenc(s: &str) -> String {
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

fn set_status(st: &Arc<Mutex<String>>, s: &str) {
    if let Ok(mut g) = st.lock() {
        *g = s.to_string();
    }
}

fn connect(host: &str, port: u16) -> Result<TcpStream, String> {
    let addr = (host, port).to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("host not found")?;
    let s = TcpStream::connect_timeout(&addr, Duration::from_secs(8)).map_err(|e| e.to_string())?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(6)));
    let _ = s.set_write_timeout(Some(Duration::from_secs(10)));
    let _ = s.set_nodelay(true);
    Ok(s)
}

fn handshake(c: &StreamCfg) -> Result<TcpStream, String> {
    let ver = env!("CARGO_PKG_VERSION");
    if c.kind == "shoutcast" || c.kind == "shoutcast2" {
        // Shoutcast: the source port is the listener port + 1
        let v2 = c.kind == "shoutcast2";
        let mut s = connect(&c.host, c.port + 1)?;
        // v2 (DNAS 2.x) names the stream: "password:#streamid"
        let first = if v2 { format!("{}:#{}\r\n", c.password, c.sid.max(1)) } else { format!("{}\r\n", c.password) };
        s.write_all(first.as_bytes()).map_err(|e| e.to_string())?;
        let mut buf = [0u8; 512];
        let mut n = s.read(&mut buf).map_err(|e| e.to_string())?;
        if v2 {
            // "OK2\r\nicy-caps:11\r\n\r\n" may arrive in pieces
            while n < buf.len() && !buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut buf[n..]) {
                    Ok(0) | Err(_) => break,
                    Ok(k) => n += k,
                }
            }
        }
        let reply = String::from_utf8_lossy(&buf[..n]).to_string();
        if !reply.starts_with("OK") {
            return Err(format!("server refused the password ({})", reply.trim()));
        }
        let hdr = format!(
            "icy-name:{}\r\nicy-genre:{}\r\nicy-url:{}\r\nicy-pub:{}\r\nicy-br:{}\r\ncontent-type:audio/mpeg\r\n\r\n",
            c.name, c.genre, c.url, if c.public { 1 } else { 0 }, c.bitrate
        );
        s.write_all(hdr.as_bytes()).map_err(|e| e.to_string())?;
        Ok(s)
    } else {
        let mut s = connect(&c.host, c.port)?;
        let mount = if c.mount.starts_with('/') { c.mount.clone() } else { format!("/{}", c.mount) };
        let auth = B64.encode(format!("{}:{}", c.user, c.password));
        let req = format!(
            "PUT {mount} HTTP/1.1\r\nHost: {}:{}\r\nAuthorization: Basic {auth}\r\nUser-Agent: RadioMoon/{ver}\r\nContent-Type: audio/mpeg\r\nice-name: {}\r\nice-genre: {}\r\nice-description: {}\r\nice-url: {}\r\nice-public: {}\r\nice-audio-info: ice-samplerate=44100;ice-bitrate={};ice-channels=2\r\nExpect: 100-continue\r\n\r\n",
            c.host, c.port, c.name, c.genre, c.description, c.url, if c.public { 1 } else { 0 }, c.bitrate
        );
        s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
        let mut buf = [0u8; 512];
        let n = s.read(&mut buf).map_err(|e| e.to_string())?;
        let reply = String::from_utf8_lossy(&buf[..n]).to_string();
        let first = reply.lines().next().unwrap_or("").to_string();
        if first.contains(" 100") || first.contains(" 200") {
            Ok(s)
        } else {
            Err(format!("server answered: {first}"))
        }
    }
}

fn update_title(c: &StreamCfg, title: &str) {
    let (path, port) = if c.kind == "shoutcast2" {
        (format!("/admin.cgi?sid={}&pass={}&mode=updinfo&song={}", c.sid.max(1), urlenc(&c.password), urlenc(title)), c.port)
    } else if c.kind == "shoutcast" {
        (format!("/admin.cgi?pass={}&mode=updinfo&song={}", urlenc(&c.password), urlenc(title)), c.port)
    } else {
        let mount = if c.mount.starts_with('/') { c.mount.clone() } else { format!("/{}", c.mount) };
        (format!("/admin/metadata?mode=updinfo&mount={}&song={}", urlenc(&mount), urlenc(title)), c.port)
    };
    if let Ok(mut s) = connect(&c.host, port) {
        let auth = B64.encode(format!("{}:{}", c.user, c.password));
        let req = format!("GET {path} HTTP/1.0\r\nHost: {}:{}\r\nUser-Agent: Mozilla/5.0 (RadioMoon)\r\nAuthorization: Basic {auth}\r\n\r\n", c.host, port);
        let _ = s.write_all(req.as_bytes());
        let mut b = [0u8; 128];
        let _ = s.read(&mut b);
    }
}

fn run(cfg: StreamCfg, rx: Receiver<Vec<f32>>, stop: Arc<AtomicBool>, status: Arc<Mutex<String>>, meta_rx: Receiver<String>) {
    let mut last_title = String::new();
    'outer: while !stop.load(Ordering::Relaxed) {
        set_status(&status, "Connecting...");
        let mut sock = match handshake(&cfg) {
            Ok(s) => s,
            Err(e) => {
                set_status(&status, &format!("Connection failed: {e}. Retrying in 10 s"));
                for _ in 0..100 {
                    if stop.load(Ordering::Relaxed) {
                        break 'outer;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                continue;
            }
        };
        let Some(mut b) = Builder::new() else {
            set_status(&status, "MP3 encoder could not be created");
            return;
        };
        let ok = b.set_num_channels(2).is_ok() && b.set_sample_rate(44_100).is_ok() && b.set_brate(bitrate(cfg.bitrate)).is_ok() && b.set_quality(Quality::Good).is_ok();
        let enc = if ok { b.build().ok() } else { None };
        let Some(mut enc) = enc else {
            set_status(&status, "MP3 encoder could not be started");
            return;
        };
        set_status(&status, &format!("On air: {} kbps to {}:{}", cfg.bitrate, cfg.host, cfg.port));
        if !last_title.is_empty() {
            update_title(&cfg, &last_title);
        }
        let mut out: Vec<u8> = Vec::new();
        loop {
            if stop.load(Ordering::Relaxed) {
                break 'outer;
            }
            while let Ok(t) = meta_rx.try_recv() {
                last_title = t;
                update_title(&cfg, &last_title);
            }
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(chunk) => {
                    let pcm: Vec<i16> = chunk.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
                    out.clear();
                    out.reserve(mp3lame_encoder::max_required_buffer_size(pcm.len()));
                    match enc.encode(InterleavedPcm(&pcm), out.spare_capacity_mut()) {
                        Ok(n) => unsafe { out.set_len(out.len() + n) },
                        Err(_) => continue,
                    }
                    if !out.is_empty() && sock.write_all(&out).is_err() {
                        set_status(&status, "Connection lost. Reconnecting...");
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break 'outer,
            }
        }
        let mut tail: Vec<u8> = Vec::new();
        tail.reserve(7200);
        if let Ok(n) = enc.flush::<FlushNoGap>(tail.spare_capacity_mut()) {
            unsafe { tail.set_len(n) };
            let _ = sock.write_all(&tail);
        }
    }
    set_status(&status, "Stopped");
}

impl Streamer {
    pub fn start(cfg: StreamCfg, rx: Receiver<Vec<f32>>) -> Streamer {
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(String::from("Starting...")));
        let (meta_tx, meta_rx) = std::sync::mpsc::channel();
        let (s2, st2) = (stop.clone(), status.clone());
        let listeners = Arc::new(AtomicI64::new(-1));
        {
            // the number of listeners is asked from the server of the station every 20 seconds
            let (c, stop, l) = (cfg.clone(), stop.clone(), listeners.clone());
            std::thread::spawn(move || {
                let mut first = true;
                while !stop.load(Ordering::Relaxed) {
                    for _ in 0..(if first { 25 } else { 200 }) {
                        if stop.load(Ordering::Relaxed) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    first = false;
                    l.store(fetch_listeners(&c).unwrap_or(-1), Ordering::Relaxed);
                }
            });
        }
        std::thread::spawn(move || run(cfg, rx, s2, st2, meta_rx));
        Streamer { stop, status, meta_tx, listeners }
    }

    pub fn set_title(&self, title: &str) {
        let _ = self.meta_tx.send(title.to_string());
    }

    pub fn status_text(&self) -> String {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Listeners that are connected to the server of the station now (-1 = the server did not tell).
    pub fn listeners(&self) -> i64 {
        self.listeners.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// GET of a page of the station server; returns the body.
fn http_body(host: &str, port: u16, path: &str) -> Option<String> {
    let mut s = connect(host, port).ok()?;
    let req = format!("GET {path} HTTP/1.0\r\nHost: {host}:{port}\r\nUser-Agent: Mozilla/5.0 (RadioMoon)\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).ok()?;
    let mut buf = Vec::new();
    let _ = s.take(256 * 1024).read_to_end(&mut buf);
    let t = String::from_utf8_lossy(&buf).to_string();
    if !t.starts_with("HTTP/1.") || t.lines().next().map(|l| !l.contains(" 200")).unwrap_or(true) {
        return None;
    }
    Some(t.split_once("\r\n\r\n").map(|x| x.1.to_string()).unwrap_or_default())
}

/// All the whole numbers that follow `key` (for example `"listeners":`).
fn nums_after(body: &str, key: &str) -> Vec<i64> {
    let mut v = Vec::new();
    let mut rest = body;
    while let Some(i) = rest.find(key) {
        rest = &rest[i + key.len()..];
        let d: String = rest.trim_start().chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = d.parse::<i64>() {
            v.push(n);
        }
    }
    v
}

/// The number of listeners that are connected now: Icecast (status-json.xsl), Shoutcast v2 (stats) or Shoutcast v1 (7.html).
fn fetch_listeners(c: &StreamCfg) -> Option<i64> {
    match c.kind.as_str() {
        "shoutcast2" => {
            let sid = c.sid.max(1);
            if let Some(b) = http_body(&c.host, c.port, &format!("/stats?sid={sid}&json=1")) {
                if let Some(n) = nums_after(&b, "\"currentlisteners\":").first() {
                    return Some(*n);
                }
            }
            let b = http_body(&c.host, c.port, &format!("/7.html?sid={sid}"))?;
            seven_html(&b)
        }
        "shoutcast" => seven_html(&http_body(&c.host, c.port, "/7.html")?),
        _ => {
            let b = http_body(&c.host, c.port, "/status-json.xsl")?;
            let mount = if c.mount.starts_with('/') { c.mount.clone() } else { format!("/{}", c.mount) };
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&b) {
                let src = &v["icestats"]["source"];
                let list: Vec<&serde_json::Value> = match src {
                    serde_json::Value::Array(a) => a.iter().collect(),
                    serde_json::Value::Null => Vec::new(),
                    o => vec![o],
                };
                let mine: Vec<&serde_json::Value> = list.iter().copied().filter(|x| x["listenurl"].as_str().map(|u| u.ends_with(&mount)).unwrap_or(false)).collect();
                let pick = if mine.is_empty() { list } else { mine };
                return Some(pick.iter().map(|x| x["listeners"].as_i64().unwrap_or(0)).sum());
            }
            // the page of some Icecast versions is not valid JSON: the numbers are read from the text
            let n = nums_after(&b, "\"listeners\":");
            if n.is_empty() {
                None
            } else {
                Some(n.iter().sum())
            }
        }
    }
}

/// `<body>listeners,status,peak,max,unique,bitrate,title</body>`
fn seven_html(b: &str) -> Option<i64> {
    let i = b.find("<body>")? + 6;
    let j = b[i..].find("</body>").map(|x| x + i).unwrap_or(b.len());
    b[i..j].split(',').next()?.trim().parse().ok()
}
