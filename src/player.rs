//! Audio engine. Everything on air goes through one mixer ("bus") so that it can be heard on the
//! main output and, at the same time, sent to the internet stream encoder.
use anyhow::Result;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::dynamic_mixer::{mixer, DynamicMixerController};
use rodio::source::Zero;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Src = Box<dyn Source<Item = f32> + Send>;

struct Out {
    _stream: OutputStream,
    handle: OutputStreamHandle,
}

fn open_out(name: &str) -> Option<Out> {
    if !name.is_empty() {
        if let Ok(devs) = rodio::cpal::default_host().output_devices() {
            for d in devs {
                if d.name().map(|n| n == name).unwrap_or(false) {
                    if let Ok((s, h)) = OutputStream::try_from_device(&d) {
                        return Some(Out { _stream: s, handle: h });
                    }
                }
            }
        }
    }
    OutputStream::try_default().ok().map(|(s, h)| Out { _stream: s, handle: h })
}

/// Names of the sound outputs of this computer.
pub fn list_outputs() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(devs) = rodio::cpal::default_host().output_devices() {
        for d in devs {
            if let Ok(n) = d.name() {
                v.push(n);
            }
        }
    }
    v
}

fn open(path: &str) -> Result<Decoder<BufReader<File>>> {
    Ok(Decoder::new(BufReader::new(File::open(path)?))?)
}

fn lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

fn file_ms(path: &str) -> u64 {
    lofty::read_from_path(path).map(|t| lofty::prelude::AudioFile::properties(&t).duration().as_millis() as u64).unwrap_or(0)
}

// ---------------------------------------------------------------- controllable source
#[derive(Clone)]
struct Ctl {
    vol: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    live: Arc<AtomicUsize>,
}

impl Ctl {
    fn new(v: f32, parts: usize) -> Self {
        Ctl { vol: Arc::new(AtomicU32::new(v.to_bits())), stop: Arc::new(AtomicBool::new(false)), pause: Arc::new(AtomicBool::new(false)), live: Arc::new(AtomicUsize::new(parts)) }
    }
    fn set_vol(&self, v: f32) {
        self.vol.store(v.to_bits(), Ordering::Relaxed);
    }
    fn get_vol(&self) -> f32 {
        f32::from_bits(self.vol.load(Ordering::Relaxed))
    }
    fn done(&self) -> bool {
        self.live.load(Ordering::Relaxed) == 0
    }
}

struct Ctrl<S> {
    inner: S,
    c: Ctl,
    ended: bool,
}

impl<S> Ctrl<S> {
    fn finish(&mut self) {
        if !self.ended {
            self.ended = true;
            self.c.live.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

impl<S> Drop for Ctrl<S> {
    fn drop(&mut self) {
        self.finish();
    }
}

impl<S: Source<Item = f32>> Iterator for Ctrl<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.ended {
            return None;
        }
        if self.c.stop.load(Ordering::Relaxed) {
            self.finish();
            return None;
        }
        if self.c.pause.load(Ordering::Relaxed) {
            return Some(0.0);
        }
        match self.inner.next() {
            Some(s) => Some(if DUCK.load(Ordering::Relaxed) { 0.0 } else { s * self.c.get_vol() }),
            None => {
                self.finish();
                None
            }
        }
    }
}

impl<S: Source<Item = f32>> Source for Ctrl<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

// ---------------------------------------------------------------- tap for the stream encoder
struct TapShared {
    enabled: AtomicBool,
    subs: Mutex<Vec<(u32, SyncSender<Vec<f32>>)>>,
}

struct Tap<S> {
    inner: S,
    sh: Arc<TapShared>,
    buf: Vec<f32>,
}

impl<S: Source<Item = f32>> Iterator for Tap<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let s = self.inner.next()?;
        if self.sh.enabled.load(Ordering::Relaxed) {
            self.buf.push(s);
            if self.buf.len() >= 4096 {
                let chunk = std::mem::replace(&mut self.buf, Vec::with_capacity(4096));
                if let Ok(g) = self.sh.subs.lock() {
                    for (_, tx) in g.iter() {
                        let _ = tx.try_send(chunk.clone());
                    }
                }
            }
        }
        Some(s)
    }
}

impl<S: Source<Item = f32>> Source for Tap<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}


// ---------------------------------------------------------------- live input (sound card or internet stream)
/// While a live source is on the air with "mute automation", everything the automation plays is silent
/// (it keeps running underneath, so the schedule stays in time).
static DUCK: AtomicBool = AtomicBool::new(false);

struct LiveShared {
    q: Mutex<VecDeque<f32>>,
    cap: usize,
    stop: AtomicBool,
    /// false = fade out and end
    on: AtomicBool,
    fade_ms: AtomicU32,
    peak: AtomicU32,
    gain: AtomicU32,
    status: Mutex<String>,
}

impl LiveShared {
    fn set_status(&self, s: &str) {
        if let Ok(mut g) = self.status.lock() {
            *g = s.to_string();
        }
    }
    /// Adds interleaved stereo 44.1 kHz samples. A stream (`block`) waits when the queue is full, a sound card drops the oldest.
    fn push(&self, buf: &mut Vec<f32>, block: bool) {
        let g = f32::from_bits(self.gain.load(Ordering::Relaxed));
        let mut pk = 0f32;
        for x in buf.iter_mut() {
            *x *= g;
            pk = pk.max(x.abs());
        }
        let old = f32::from_bits(self.peak.load(Ordering::Relaxed));
        self.peak.store(pk.max(old * 0.9).to_bits(), Ordering::Relaxed);
        if block {
            while !self.stop.load(Ordering::Relaxed) && self.q.lock().map(|q| q.len() > self.cap).unwrap_or(false) {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        if let Ok(mut q) = self.q.lock() {
            q.extend(buf.iter().copied());
            while q.len() > self.cap * 2 {
                q.pop_front();
            }
        }
        buf.clear();
    }
}

struct LiveSrc {
    sh: Arc<LiveShared>,
    /// fade gain 0..1: rises when the source starts, falls to 0 when it ends
    cur: f32,
}

impl Iterator for LiveSrc {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.sh.stop.load(Ordering::Relaxed) {
            return None;
        }
        // one step per interleaved sample: 44.1 frames per ms, 2 samples per frame
        let step = 1.0 / (self.sh.fade_ms.load(Ordering::Relaxed).max(1) as f32 * 88.2);
        if self.sh.on.load(Ordering::Relaxed) {
            if self.cur < 1.0 {
                self.cur = (self.cur + step).min(1.0);
            }
        } else {
            self.cur -= step;
            if self.cur <= 0.0 {
                self.sh.stop.store(true, Ordering::Relaxed); // also ends the capture / stream thread
                return None;
            }
        }
        Some(self.sh.q.lock().ok().and_then(|mut q| q.pop_front()).unwrap_or(0.0) * self.cur)
    }
}

impl Source for LiveSrc {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        44100
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Linear resampler to 44.1 kHz.
struct Rs {
    step: f64,
    pos: f64,
    prev: (f32, f32),
}

impl Rs {
    fn new(rate: u32) -> Self {
        Rs { step: rate.max(1) as f64 / 44100.0, pos: 0.0, prev: (0.0, 0.0) }
    }
    fn frame(&mut self, l: f32, r: f32, out: &mut Vec<f32>) {
        while self.pos < 1.0 {
            let a = self.pos as f32;
            out.push(self.prev.0 + (l - self.prev.0) * a);
            out.push(self.prev.1 + (r - self.prev.1) * a);
            self.pos += self.step;
        }
        self.pos -= 1.0;
        self.prev = (l, r);
    }
}

/// Names of the sound inputs of this computer.
pub fn list_inputs() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(devs) = rodio::cpal::default_host().input_devices() {
        for d in devs {
            if let Ok(n) = d.name() {
                v.push(n);
            }
        }
    }
    v
}

fn capture_thread(device: String, sh: Arc<LiveShared>, ready: std::sync::mpsc::Sender<Result<(), String>>) {
    use rodio::cpal::SampleFormat;
    let host = rodio::cpal::default_host();
    let dev = host.input_devices().ok().and_then(|mut it| it.find(|d| d.name().map(|n| n == device).unwrap_or(false))).or_else(|| host.default_input_device());
    let dev = match dev {
        Some(d) => d,
        None => {
            let _ = ready.send(Err("no audio input device".into()));
            return;
        }
    };
    let cfg = match dev.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    let ch = cfg.channels().max(1) as usize;
    let mut rs = Rs::new(cfg.sample_rate().0);
    let sh2 = sh.clone();
    let mut out: Vec<f32> = Vec::new();
    let mut handle = move |d: &[f32]| {
        for f in d.chunks_exact(ch) {
            let (l, r) = if ch == 1 { (f[0], f[0]) } else { (f[0], f[1]) };
            rs.frame(l, r, &mut out);
        }
        sh2.push(&mut out, false);
    };
    let err = |e| eprintln!("live input: {e}");
    let stream = match cfg.sample_format() {
        SampleFormat::F32 => dev.build_input_stream(&cfg.clone().into(), move |d: &[f32], _| handle(d), err, None),
        SampleFormat::I16 => {
            let mut tmp: Vec<f32> = Vec::new();
            dev.build_input_stream(
                &cfg.clone().into(),
                move |d: &[i16], _| {
                    tmp.clear();
                    tmp.extend(d.iter().map(|x| *x as f32 / 32768.0));
                    handle(&tmp)
                },
                err,
                None,
            )
        }
        SampleFormat::U16 => {
            let mut tmp: Vec<f32> = Vec::new();
            dev.build_input_stream(
                &cfg.clone().into(),
                move |d: &[u16], _| {
                    tmp.clear();
                    tmp.extend(d.iter().map(|x| (*x as f32 - 32768.0) / 32768.0));
                    handle(&tmp)
                },
                err,
                None,
            )
        }
        f => {
            let _ = ready.send(Err(format!("unsupported input format {f:?}")));
            return;
        }
    };
    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    use rodio::cpal::traits::StreamTrait;
    if let Err(e) = stream.play() {
        let _ = ready.send(Err(e.to_string()));
        return;
    }
    sh.set_status("Sound input on air");
    let _ = ready.send(Ok(()));
    while !sh.stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(100));
    }
    drop(stream);
}

/// A network stream has no seeking; the decoder only needs to be told so.
struct NoSeek<R>(R);

impl<R: Read> Read for NoSeek<R> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(b)
    }
}

impl<R> Seek for NoSeek<R> {
    fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
        match p {
            SeekFrom::Current(0) => Ok(0),
            _ => Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "live stream")),
        }
    }
}

/// Plays an internet radio stream (HTTP/HTTPS, MP3 or AAC) into the queue; reconnects when it drops.
fn stream_thread(url: String, sh: Arc<LiveShared>) {
    while !sh.stop.load(Ordering::Relaxed) {
        sh.set_status("Connecting to the stream ...");
        let res = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(8)).timeout_read(Duration::from_secs(15)).build().get(&url).set("User-Agent", "RadioMoon").call();
        match res {
            Ok(resp) => {
                let reader = resp.into_reader();
                match Decoder::new(NoSeek(BufReader::with_capacity(64 * 1024, reader))) {
                    Ok(dec) => {
                        let ch = dec.channels().max(1) as usize;
                        let mut rs = Rs::new(dec.sample_rate());
                        let mut it = dec.convert_samples::<f32>();
                        let mut out: Vec<f32> = Vec::with_capacity(8192);
                        sh.set_status("Stream on air");
                        'play: loop {
                            let mut f = [0f32; 8];
                            for k in 0..ch {
                                match it.next() {
                                    Some(x) => {
                                        if k < 8 {
                                            f[k] = x;
                                        }
                                    }
                                    None => break 'play,
                                }
                            }
                            let (l, r) = if ch == 1 { (f[0], f[0]) } else { (f[0], f[1]) };
                            rs.frame(l, r, &mut out);
                            if out.len() >= 4096 {
                                sh.push(&mut out, true);
                                if sh.stop.load(Ordering::Relaxed) {
                                    return;
                                }
                            }
                        }
                        sh.set_status("Stream ended, reconnecting ...");
                    }
                    Err(e) => sh.set_status(&format!("Stream format not supported: {e}")),
                }
            }
            Err(e) => sh.set_status(&format!("Stream connection failed: {e}")),
        }
        for _ in 0..30 {
            if sh.stop.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

// ---------------------------------------------------------------- player
struct Deck {
    ctl: Ctl,
    fade: Option<(Instant, Duration)>,
    fade_in: Option<(Instant, Duration)>,
}

pub struct Player {
    main: Out,
    monitor: Option<Out>,
    instants: Option<Out>,
    bus: Arc<DynamicMixerController<f32>>,
    _bus_sink: Sink,
    tap: Arc<TapShared>,
    live: Option<Arc<LiveShared>>,
    pending_fade_in: Option<Duration>,
    decks: Vec<Deck>,
    jingles: Vec<Ctl>,
    jingle_sinks: Vec<Sink>,
    overlays: Vec<Ctl>,
    preview: Option<Sink>,
    events: Vec<(u64, String, f32)>,
    pub volume: f32,
    started: Option<Instant>,
    skip_ms: u64,
    preview_started: Option<Instant>,
    preview_skip: u64,
    paused_at: Option<Instant>,
    paused_total: Duration,
}

impl Player {
    pub fn new(main: &str, monitor: &str, instants: &str) -> Result<Self> {
        let m = open_out(main).ok_or_else(|| anyhow::anyhow!("no audio output device"))?;
        let (bus, mix) = mixer::<f32>(2, 44100);
        bus.add(Zero::<f32>::new(2, 44100));
        let tap = Arc::new(TapShared { enabled: AtomicBool::new(false), subs: Mutex::new(Vec::new()) });
        let sink = Sink::try_new(&m.handle)?;
        sink.append(Tap { inner: mix, sh: tap.clone(), buf: Vec::with_capacity(4096) });
        Ok(Self {
            main: m,
            monitor: if monitor.is_empty() { None } else { open_out(monitor) },
            instants: if instants.is_empty() { None } else { open_out(instants) },
            bus,
            _bus_sink: sink,
            tap,
            live: None,
            pending_fade_in: None,
            decks: Vec::new(),
            jingles: Vec::new(),
            jingle_sinks: Vec::new(),
            overlays: Vec::new(),
            preview: None,
            events: Vec::new(),
            volume: 1.0,
            started: None,
            skip_ms: 0,
            preview_started: None,
            preview_skip: 0,
            paused_at: None,
            paused_total: Duration::ZERO,
        })
    }

    // ----- taps: any number of consumers of the on-air mix (internet streams, MPX) -----
    pub fn add_tap(&mut self) -> (u32, Receiver<Vec<f32>>) {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = sync_channel(64);
        if let Ok(mut g) = self.tap.subs.lock() {
            g.push((id, tx));
        }
        self.tap.enabled.store(true, Ordering::Relaxed);
        (id, rx)
    }

    pub fn remove_tap(&mut self, id: u32) {
        let any = if let Ok(mut g) = self.tap.subs.lock() {
            g.retain(|x| x.0 != id);
            !g.is_empty()
        } else {
            false
        };
        self.tap.enabled.store(any, Ordering::Relaxed);
    }

    // ----- live source: sound card input or internet stream, mixed into the on-air bus -----
    /// `url` empty = sound card `device` (empty = default input); otherwise the internet stream at `url`.
    pub fn start_live(&mut self, url: &str, device: &str, gain_db: f32, mute_automation: bool, fade_ms: u32) -> Result<()> {
        self.stop_live();
        let stream = !url.trim().is_empty();
        let sh = Arc::new(LiveShared {
            q: Mutex::new(VecDeque::new()),
            cap: if stream { 44100 * 4 } else { 44100 / 2 },
            stop: AtomicBool::new(false),
            on: AtomicBool::new(true),
            fade_ms: AtomicU32::new(fade_ms),
            peak: AtomicU32::new(0),
            gain: AtomicU32::new(lin(gain_db).to_bits()),
            status: Mutex::new(String::new()),
        });
        if stream {
            let (u, s2) = (url.trim().to_string(), sh.clone());
            std::thread::spawn(move || stream_thread(u, s2));
        } else {
            let (tx, rx) = std::sync::mpsc::channel();
            let (d, s2) = (device.to_string(), sh.clone());
            std::thread::spawn(move || capture_thread(d, s2, tx));
            match rx.recv_timeout(Duration::from_secs(5)) {
                Ok(Ok(())) => {}
                Ok(Err(e)) => anyhow::bail!(e),
                Err(_) => {
                    sh.stop.store(true, Ordering::Relaxed);
                    anyhow::bail!("the sound input did not start");
                }
            }
        }
        self.bus.add(LiveSrc { sh: sh.clone(), cur: if fade_ms == 0 { 1.0 } else { 0.0 } });
        DUCK.store(mute_automation, Ordering::Relaxed);
        self.live = Some(sh);
        Ok(())
    }

    /// Fades the live source out (the sound of the automation can come back meanwhile) and ends it.
    pub fn end_live(&mut self, fade_ms: u32) {
        if let Some(sh) = self.live.take() {
            sh.fade_ms.store(fade_ms, Ordering::Relaxed);
            sh.on.store(false, Ordering::Relaxed);
        }
        DUCK.store(false, Ordering::Relaxed);
    }

    /// Fades out everything the automation is playing (the decks).
    pub fn fade_out_all(&mut self, ms: u64) {
        for d in self.decks.iter_mut() {
            if d.fade.is_none() {
                d.fade = Some((Instant::now(), Duration::from_millis(ms)));
            }
        }
    }

    /// The next track that starts fades in over `ms`.
    pub fn fade_in_next(&mut self, ms: u64) {
        self.pending_fade_in = if ms > 0 { Some(Duration::from_millis(ms)) } else { None };
    }

    /// Ends the live source at once.
    pub fn stop_live(&mut self) {
        if let Some(sh) = self.live.take() {
            sh.stop.store(true, Ordering::Relaxed);
        }
        DUCK.store(false, Ordering::Relaxed);
    }

    pub fn live_active(&self) -> bool {
        self.live.is_some()
    }

    pub fn set_live_gain(&self, db: f32) {
        if let Some(sh) = &self.live {
            sh.gain.store(lin(db).to_bits(), Ordering::Relaxed);
        }
    }

    pub fn set_live_mute_automation(&self, on: bool) {
        if self.live.is_some() {
            DUCK.store(on, Ordering::Relaxed);
        }
    }

    /// (peak 0..1, status text) of the live source.
    pub fn live_info(&self) -> (f32, String) {
        match &self.live {
            Some(sh) => (f32::from_bits(sh.peak.load(Ordering::Relaxed)), sh.status.lock().map(|g| g.clone()).unwrap_or_default()),
            None => (0.0, String::new()),
        }
    }

    // ----- decks -----
    fn start_deck(&mut self, parts: Vec<Src>, fade_prev_ms: u64, skip_ms: u64) {
        if fade_prev_ms > 0 {
            for d in self.decks.iter_mut() {
                if d.fade.is_none() {
                    d.fade = Some((Instant::now(), Duration::from_millis(fade_prev_ms)));
                }
            }
        }
        let c = Ctl::new(self.volume, parts.len());
        for p in parts {
            self.bus.add(Ctrl { inner: p, c: c.clone(), ended: false });
        }
        let fade_in = self.pending_fade_in.take().map(|d| (Instant::now(), d));
        self.decks.push(Deck { ctl: c, fade: None, fade_in });
        self.started = Some(Instant::now());
        self.skip_ms = skip_ms;
        self.paused_at = None;
        self.paused_total = Duration::ZERO;
        self.events.clear();
    }

    /// Starts a track on a new deck. The previous deck plays on (mix) unless `fade_prev_ms` > 0.
    pub fn play_main(&mut self, path: &str, skip_ms: u64, gain_db: f32, fade_prev_ms: u64) -> Result<()> {
        let dec = open(path)?;
        let src: Src = Box::new(dec.skip_duration(Duration::from_millis(skip_ms)).amplify(lin(gain_db)).convert_samples::<f32>());
        self.start_deck(vec![src], fade_prev_ms, skip_ms);
        Ok(())
    }

    /// Plays several audio pieces one after another: (path, start_ms, length_ms; 0 = until the end).
    pub fn play_sequence(&mut self, parts: &[(String, u64, u64)], gain_db: f32, fade_prev_ms: u64) -> Result<()> {
        let mut srcs: Vec<Src> = Vec::new();
        let mut offset = 0u64;
        for (path, start, len) in parts {
            let Ok(dec) = open(path) else { continue };
            let total = if *len > 0 { *len } else { file_ms(path).saturating_sub(*start) };
            let s: Src = if *len > 0 {
                Box::new(
                    dec.skip_duration(Duration::from_millis(*start))
                        .take_duration(Duration::from_millis(*len))
                        .amplify(lin(gain_db))
                        .convert_samples::<f32>()
                        .delay(Duration::from_millis(offset)),
                )
            } else {
                Box::new(dec.skip_duration(Duration::from_millis(*start)).amplify(lin(gain_db)).convert_samples::<f32>().delay(Duration::from_millis(offset)))
            };
            srcs.push(s);
            offset += total;
        }
        if srcs.is_empty() {
            anyhow::bail!("none of the audio parts could be opened");
        }
        self.start_deck(srcs, fade_prev_ms, 0);
        Ok(())
    }

    /// Sweepers / voice tracks to start while the current track plays: (position_ms, path, gain_db).
    pub fn set_events(&mut self, ev: Vec<(u64, String, f32)>) {
        self.events = ev;
    }

    fn play_extra(&mut self, path: &str, gain_db: f32, instants_out: bool) -> Result<Option<Ctl>> {
        let dec = open(path)?;
        if instants_out {
            if let Some(o) = &self.instants {
                let sink = Sink::try_new(&o.handle)?;
                sink.append(dec.amplify(lin(gain_db)));
                self.jingle_sinks.push(sink);
                return Ok(None);
            }
        }
        let src: Src = Box::new(dec.amplify(lin(gain_db)).convert_samples::<f32>());
        let c = Ctl::new(1.0, 1);
        self.bus.add(Ctrl { inner: src, c: c.clone(), ended: false });
        Ok(Some(c))
    }

    pub fn play_jingle(&mut self, path: &str, gain_db: f32) -> Result<()> {
        if let Some(c) = self.play_extra(path, gain_db, true)? {
            self.jingles.push(c);
        }
        Ok(())
    }

    pub fn stop_jingles(&mut self) {
        for c in self.jingles.drain(..) {
            c.stop.store(true, Ordering::Relaxed);
        }
        for s in self.jingle_sinks.drain(..) {
            s.stop();
        }
    }

    // ----- preview (monitor output, never broadcast) -----
    pub fn play_preview(&mut self, path: &str, skip_ms: u64, len_ms: u64) -> Result<()> {
        self.stop_preview();
        let dec = open(path)?;
        let handle = match &self.monitor {
            Some(o) => &o.handle,
            None => &self.main.handle,
        };
        let sink = Sink::try_new(handle)?;
        let src = dec.skip_duration(Duration::from_millis(skip_ms));
        if len_ms > 0 {
            sink.append(src.take_duration(Duration::from_millis(len_ms)));
        } else {
            sink.append(src);
        }
        self.preview = Some(sink);
        self.preview_started = Some(Instant::now());
        self.preview_skip = skip_ms;
        Ok(())
    }

    pub fn stop_preview(&mut self) {
        if let Some(s) = self.preview.take() {
            s.stop();
        }
        self.preview_started = None;
    }

    pub fn preview_active(&self) -> bool {
        self.preview.as_ref().map(|s| !s.empty()).unwrap_or(false)
    }

    /// Position in the previewed file (ms).
    pub fn preview_pos_ms(&self) -> u64 {
        self.preview_started.map(|s| self.preview_skip + s.elapsed().as_millis() as u64).unwrap_or(0)
    }

    pub fn preview_elapsed_ms(&self) -> u64 {
        self.preview_started.map(|s| s.elapsed().as_millis() as u64).unwrap_or(0)
    }

    // ----- control -----
    pub fn stop(&mut self) {
        for d in self.decks.drain(..) {
            d.ctl.stop.store(true, Ordering::Relaxed);
        }
        for c in self.overlays.drain(..) {
            c.stop.store(true, Ordering::Relaxed);
        }
        self.events.clear();
        self.started = None;
        self.paused_at = None;
        self.paused_total = Duration::ZERO;
    }

    pub fn toggle_pause(&mut self) {
        if self.decks.is_empty() {
            return;
        }
        if let Some(p) = self.paused_at.take() {
            self.paused_total += p.elapsed();
            for d in &self.decks {
                d.ctl.pause.store(false, Ordering::Relaxed);
            }
        } else {
            for d in &self.decks {
                d.ctl.pause.store(true, Ordering::Relaxed);
            }
            self.paused_at = Some(Instant::now());
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v;
    }

    /// Position (ms) in the audio file of the newest deck.
    pub fn pos_ms(&self) -> u64 {
        match self.started {
            Some(s) => {
                let paused = self.paused_total + self.paused_at.map(|p| p.elapsed()).unwrap_or_default();
                self.skip_ms + s.elapsed().saturating_sub(paused).as_millis() as u64
            }
            None => 0,
        }
    }

    /// Time played since the track started (without the skipped intro part).
    pub fn elapsed_ms(&self) -> u64 {
        self.pos_ms().saturating_sub(self.skip_ms)
    }

    pub fn newest_empty(&self) -> bool {
        self.decks.last().map(|d| d.ctl.done()).unwrap_or(false)
    }

    pub fn tick(&mut self) {
        self.jingles.retain(|c| !c.done());
        self.overlays.retain(|c| !c.done());
        self.jingle_sinks.retain(|s| !s.empty());
        let busy = !self.jingles.is_empty() || !self.overlays.is_empty() || !self.jingle_sinks.is_empty();
        let vol = self.volume * if busy { 0.35 } else { 1.0 };
        let newest = self.decks.len().saturating_sub(1);
        let mut i = 0;
        self.decks.retain(|d| {
            let faded = d.fade.map(|(t, l)| t.elapsed() >= l).unwrap_or(false);
            let keep = !(d.ctl.done() && i != newest) && !faded;
            if !keep {
                d.ctl.stop.store(true, Ordering::Relaxed);
            }
            i += 1;
            keep
        });
        for d in &self.decks {
            let mut f = match d.fade {
                Some((t, l)) => (1.0 - t.elapsed().as_secs_f32() / l.as_secs_f32().max(0.01)).clamp(0.0, 1.0),
                None => 1.0,
            };
            if let Some((t, l)) = d.fade_in {
                f *= (t.elapsed().as_secs_f32() / l.as_secs_f32().max(0.01)).clamp(0.0, 1.0);
            }
            d.ctl.set_vol(vol * f);
        }
        let pos = self.pos_ms();
        let mut due = Vec::new();
        self.events.retain(|e| {
            if e.0 <= pos {
                due.push(e.clone());
                false
            } else {
                true
            }
        });
        for (_, path, gain) in due {
            if let Ok(Some(c)) = self.play_extra(&path, gain, false) {
                self.overlays.push(c);
            }
        }
    }
}
