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

// ------------------------------------------------------------------ encoders
/// Bytes that an encoder produced (the sink of Ogg and FLAC writers).
#[derive(Clone, Default)]
struct SharedBuf(Arc<Mutex<Vec<u8>>>);

impl Write for SharedBuf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut g) = self.0.lock() {
            g.extend_from_slice(b);
        }
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl SharedBuf {
    fn take(&self) -> Vec<u8> {
        self.0.lock().map(|mut g| std::mem::take(&mut *g)).unwrap_or_default()
    }
}

/// An audio encoder of the broadcast: 44.1 kHz stereo 16 bit in, the bytes of the stream out.
trait Enc {
    fn encode(&mut self, pcm: &[i16]) -> Vec<u8>;
    /// A new song title for the stream itself (Ogg streams); the result is sent to the server.
    fn set_title(&mut self, _title: &str) -> Vec<u8> {
        Vec::new()
    }
    fn finish(&mut self) -> Vec<u8>;
}

struct Mp3Enc {
    enc: mp3lame_encoder::Encoder,
}

impl Enc for Mp3Enc {
    fn encode(&mut self, pcm: &[i16]) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        out.reserve(mp3lame_encoder::max_required_buffer_size(pcm.len()));
        match self.enc.encode(InterleavedPcm(pcm), out.spare_capacity_mut()) {
            Ok(n) => unsafe { out.set_len(out.len() + n) },
            Err(_) => out.clear(),
        }
        out
    }
    fn finish(&mut self) -> Vec<u8> {
        let mut tail: Vec<u8> = Vec::new();
        tail.reserve(7200);
        if let Ok(n) = self.enc.flush::<FlushNoGap>(tail.spare_capacity_mut()) {
            unsafe { tail.set_len(n) };
        }
        tail
    }
}

/// 44.1 kHz stereo to 48 kHz (cubic interpolation); Opus does not take 44.1 kHz.
struct Resampler {
    buf: Vec<[f32; 2]>,
    pos: f64,
}

impl Resampler {
    fn new() -> Resampler {
        Resampler { buf: vec![[0.0, 0.0]], pos: 1.0 }
    }
    fn process(&mut self, pcm: &[i16], out: &mut Vec<i16>) {
        for f in pcm.chunks_exact(2) {
            self.buf.push([f[0] as f32, f[1] as f32]);
        }
        let step = 44100.0 / 48000.0;
        while self.pos + 2.0 < self.buf.len() as f64 {
            let i = self.pos.floor() as usize;
            let t = (self.pos - i as f64) as f32;
            for ch in 0..2 {
                let (p0, p1, p2, p3) = (self.buf[i - 1][ch], self.buf[i][ch], self.buf[i + 1][ch], self.buf[i + 2][ch]);
                let v = p1 + 0.5 * t * (p2 - p0 + t * (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3 + t * (3.0 * (p1 - p2) + p3 - p0)));
                out.push(v.clamp(-32768.0, 32767.0) as i16);
            }
            self.pos += step;
        }
        let drop = (self.pos.floor() as usize).saturating_sub(1);
        if drop > 0 {
            self.buf.drain(..drop);
            self.pos -= drop as f64;
        }
    }
}

/// Ogg Opus: the Opus encoder and an Ogg writer (a new title starts a new logical stream, as Icecast expects).
struct OpusEnc {
    enc: opus::Encoder,
    rs: Resampler,
    acc: Vec<i16>,
    ogg: ogg::writing::PacketWriter<'static, SharedBuf>,
    sink: SharedBuf,
    serial: u32,
    granule: u64,
    pre_skip: u16,
    in_page: u32,
    held: Option<(Vec<u8>, u64)>,
    started: bool,
    title: String,
}

fn opus_head(pre_skip: u16) -> Vec<u8> {
    let mut h = b"OpusHead".to_vec();
    h.push(1);
    h.push(2);
    h.extend_from_slice(&pre_skip.to_le_bytes());
    h.extend_from_slice(&44100u32.to_le_bytes());
    h.extend_from_slice(&0i16.to_le_bytes());
    h.push(0);
    h
}

fn opus_tags(title: &str) -> Vec<u8> {
    let vendor = format!("RadioMoon {}", env!("CARGO_PKG_VERSION"));
    let mut t = b"OpusTags".to_vec();
    t.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    t.extend_from_slice(vendor.as_bytes());
    if title.is_empty() {
        t.extend_from_slice(&0u32.to_le_bytes());
    } else {
        let c = format!("TITLE={title}");
        t.extend_from_slice(&1u32.to_le_bytes());
        t.extend_from_slice(&(c.len() as u32).to_le_bytes());
        t.extend_from_slice(c.as_bytes());
    }
    t
}

impl OpusEnc {
    fn new(kbps: u32, title: &str) -> Result<OpusEnc, String> {
        let mut enc = opus::Encoder::new(48000, opus::Channels::Stereo, opus::Application::Audio).map_err(|e| format!("Opus encoder: {e}"))?;
        let _ = enc.set_bitrate(opus::Bitrate::Bits((kbps.clamp(16, 510) * 1000) as i32));
        let pre_skip = enc.get_lookahead().unwrap_or(312).clamp(0, 65535) as u16;
        let sink = SharedBuf::default();
        let serial = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() ^ d.as_secs() as u32).unwrap_or(1);
        Ok(OpusEnc { enc, rs: Resampler::new(), acc: Vec::new(), ogg: ogg::writing::PacketWriter::new(sink.clone()), sink, serial, granule: 0, pre_skip, in_page: 0, held: None, started: false, title: title.to_string() })
    }

    /// The two header packets of a (new) logical stream.
    fn start(&mut self) {
        let pre = if self.started { 0 } else { self.pre_skip };
        let _ = self.ogg.write_packet(opus_head(pre), self.serial, ogg::writing::PacketWriteEndInfo::EndPage, 0);
        let _ = self.ogg.write_packet(opus_tags(&self.title), self.serial, ogg::writing::PacketWriteEndInfo::EndPage, 0);
        self.granule = pre as u64;
        self.in_page = 0;
        self.started = true;
    }

    /// One packet is held back so that the last packet of a stream can be marked as the end.
    fn push_packet(&mut self, data: Vec<u8>) {
        self.granule += 960;
        if let Some((prev, g)) = self.held.replace((data, self.granule)) {
            self.in_page += 1;
            let info = if self.in_page >= 5 { ogg::writing::PacketWriteEndInfo::EndPage } else { ogg::writing::PacketWriteEndInfo::NormalPacket };
            if self.in_page >= 5 {
                self.in_page = 0;
            }
            let _ = self.ogg.write_packet(prev, self.serial, info, g);
        }
    }

    fn end_stream(&mut self) {
        if let Some((prev, g)) = self.held.take() {
            let _ = self.ogg.write_packet(prev, self.serial, ogg::writing::PacketWriteEndInfo::EndStream, g);
        }
    }
}

impl Enc for OpusEnc {
    fn encode(&mut self, pcm: &[i16]) -> Vec<u8> {
        if !self.started {
            self.start();
        }
        let mut r = Vec::new();
        self.rs.process(pcm, &mut r);
        self.acc.extend_from_slice(&r);
        let mut buf = vec![0u8; 4000];
        while self.acc.len() >= 1920 {
            let frame: Vec<i16> = self.acc.drain(..1920).collect();
            if let Ok(n) = self.enc.encode(&frame, &mut buf) {
                self.push_packet(buf[..n].to_vec());
            }
        }
        self.sink.take()
    }

    fn set_title(&mut self, title: &str) -> Vec<u8> {
        if !self.started || title == self.title {
            self.title = title.to_string();
            return Vec::new();
        }
        self.end_stream();
        self.title = title.to_string();
        self.serial = self.serial.wrapping_add(1);
        self.start();
        self.sink.take()
    }

    fn finish(&mut self) -> Vec<u8> {
        self.end_stream();
        self.sink.take()
    }
}

/// Ogg FLAC (lossless) with libFLAC.
struct FlacEnc {
    enc: Option<flac_bound::FlacEncoder<'static>>,
    sink: SharedBuf,
}

impl FlacEnc {
    fn new() -> Result<FlacEnc, String> {
        let sink = SharedBuf::default();
        // the encoder keeps a reference to its writer for as long as it lives: the two small boxes stay allocated
        let w: &'static mut SharedBuf = Box::leak(Box::new(sink.clone()));
        let ww: &'static mut flac_bound::WriteWrapper<'static> = Box::leak(Box::new(flac_bound::WriteWrapper(w)));
        let serial = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| (d.subsec_nanos() ^ d.as_secs() as u32) & 0x7fff_ffff).unwrap_or(1) as std::os::raw::c_long;
        let cfg = flac_bound::FlacEncoder::new().ok_or("FLAC encoder could not be created")?;
        let enc = cfg
            .channels(2)
            .sample_rate(44100)
            .bits_per_sample(16)
            .compression_level(5)
            .blocksize(4096)
            .ogg_serial_number(serial)
            .init_write_ogg(ww)
            .map_err(|e| format!("FLAC encoder: {e:?}"))?;
        Ok(FlacEnc { enc: Some(enc), sink })
    }
}

impl Enc for FlacEnc {
    fn encode(&mut self, pcm: &[i16]) -> Vec<u8> {
        if let Some(e) = self.enc.as_mut() {
            let v: Vec<i32> = pcm.iter().map(|s| *s as i32).collect();
            let _ = e.process_interleaved(&v, (v.len() / 2) as u32);
        }
        self.sink.take()
    }
    fn finish(&mut self) -> Vec<u8> {
        if let Some(e) = self.enc.take() {
            let _ = e.finish();
        }
        self.sink.take()
    }
}

fn make_encoder(c: &StreamCfg, title: &str) -> Result<Box<dyn Enc>, String> {
    match c.codec.as_str() {
        "opus" => Ok(Box::new(OpusEnc::new(c.bitrate, title)?)),
        "flac" => Ok(Box::new(FlacEnc::new()?)),
        _ => {
            let mut b = Builder::new().ok_or("MP3 encoder could not be created")?;
            let ok = b.set_num_channels(2).is_ok() && b.set_sample_rate(44_100).is_ok() && b.set_brate(bitrate(c.bitrate)).is_ok() && b.set_quality(Quality::Good).is_ok();
            let enc = if ok { b.build().ok() } else { None }.ok_or("MP3 encoder could not be started")?;
            Ok(Box::new(Mp3Enc { enc }))
        }
    }
}

/// Ogg streams (Opus, FLAC) are only for Icecast; Shoutcast takes MP3.
fn effective_codec(c: &StreamCfg) -> &str {
    if c.kind == "icecast" && (c.codec == "opus" || c.codec == "flac") {
        c.codec.as_str()
    } else {
        "mp3"
    }
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
        let ctype = match effective_codec(c) {
            "opus" => "audio/ogg",
            "flac" => "application/ogg",
            _ => "audio/mpeg",
        };
        let req = format!(
            "PUT {mount} HTTP/1.1\r\nHost: {}:{}\r\nAuthorization: Basic {auth}\r\nUser-Agent: RadioMoon/{ver}\r\nContent-Type: {ctype}\r\nice-name: {}\r\nice-genre: {}\r\nice-description: {}\r\nice-url: {}\r\nice-public: {}\r\nice-audio-info: ice-samplerate=44100;ice-bitrate={};ice-channels=2\r\nExpect: 100-continue\r\n\r\n",
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
    if effective_codec(c) != "mp3" {
        return; // the title of an Ogg stream travels inside the stream
    }
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
        let mut enc = match make_encoder(&cfg, &last_title) {
            Ok(e) => e,
            Err(e) => {
                set_status(&status, &e);
                return;
            }
        };
        let label = match effective_codec(&cfg) {
            "opus" => format!("Opus {} kbps", cfg.bitrate),
            "flac" => "FLAC lossless".to_string(),
            _ => format!("MP3 {} kbps", cfg.bitrate),
        };
        set_status(&status, &format!("On air: {label} to {}:{}", cfg.host, cfg.port));
        if !last_title.is_empty() {
            update_title(&cfg, &last_title);
        }
        loop {
            if stop.load(Ordering::Relaxed) {
                break 'outer;
            }
            while let Ok(t) = meta_rx.try_recv() {
                last_title = t;
                update_title(&cfg, &last_title);
                let b = enc.set_title(&last_title);
                if !b.is_empty() && sock.write_all(&b).is_err() {
                    set_status(&status, "Connection lost. Reconnecting...");
                    continue 'outer;
                }
            }
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(chunk) => {
                    let pcm: Vec<i16> = chunk.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
                    let out = enc.encode(&pcm);
                    if !out.is_empty() && sock.write_all(&out).is_err() {
                        set_status(&status, "Connection lost. Reconnecting...");
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break 'outer,
            }
        }
        let tail = enc.finish();
        if !tail.is_empty() {
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
