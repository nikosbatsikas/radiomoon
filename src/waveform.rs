//! Decodes a track in the background and stores its peak levels (used for the waveform and VU meter).
use rodio::Decoder;
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};

pub type WaveStore = Arc<Mutex<(u64, Arc<Vec<f32>>)>>;

const CHUNK: usize = 4096;

pub fn new_store() -> WaveStore {
    Arc::new(Mutex::new((0, Arc::new(Vec::new()))))
}

/// The peak levels of a whole file (one value per 4096 samples).
pub fn peaks_of(path: &str) -> Option<Vec<f32>> {
    let f = File::open(path).ok()?;
    let dec = Decoder::new(BufReader::new(f)).ok()?;
    let mut peaks: Vec<f32> = Vec::new();
    let (mut m, mut n) = (0i32, 0usize);
    for s in dec {
        let a = (s as i32).abs();
        if a > m {
            m = a;
        }
        n += 1;
        if n == CHUNK {
            peaks.push(m as f32 / 32768.0);
            m = 0;
            n = 0;
        }
    }
    if n > 0 {
        peaks.push(m as f32 / 32768.0);
    }
    Some(peaks)
}

pub fn spawn(path: String, id: u64, store: WaveStore) {
    if let Ok(mut g) = store.lock() {
        *g = (id, Arc::new(Vec::new()));
    }
    std::thread::spawn(move || {
        let Some(peaks) = peaks_of(&path) else { return };
        if let Ok(mut g) = store.lock() {
            if g.0 == id {
                g.1 = Arc::new(peaks);
            }
        }
    });
}

/// Finds the mix-in / mix-out points (first and last sample above `thr_db`) and the duration.
pub fn autocue(path: &str, thr_db: f32) -> Option<(u64, u64, u64)> {
    use rodio::Source;
    let f = File::open(path).ok()?;
    let dec = Decoder::new(BufReader::new(f)).ok()?;
    let ch = dec.channels().max(1) as u64;
    let rate = dec.sample_rate().max(1) as u64;
    let thr = (10f32.powf(thr_db / 20.0) * 32768.0) as i32;
    let (mut first, mut last, mut n) = (None::<u64>, 0u64, 0u64);
    for s in dec {
        if (s as i32).abs() >= thr {
            if first.is_none() {
                first = Some(n);
            }
            last = n;
        }
        n += 1;
    }
    let to_ms = |i: u64| i * 1000 / (ch * rate);
    Some((first.map(to_ms).unwrap_or(0), to_ms(last), to_ms(n)))
}

/// Peak level of a file in dB (used to normalize the playback volume).
pub fn peak_db(path: &str) -> Option<f32> {
    let f = File::open(path).ok()?;
    let dec = Decoder::new(BufReader::new(f)).ok()?;
    let mut m = 0i32;
    for s in dec {
        let a = (s as i32).abs();
        if a > m {
            m = a;
        }
    }
    if m == 0 {
        return None;
    }
    Some(20.0 * (m as f32 / 32768.0).log10())
}
