//! MPX receiver (transmitter site): IP -> 192 kHz sound card. Used by the separate receiver program.
use crate::mpxdsp::from_l24;
use crate::mpxio::{card_thread, get, set, track_peak, Queue, Text};
use std::io::Read;
use std::net::{TcpListener, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct RxCfg {
    pub proto: String,
    pub port: u16,
    pub device: String,
    pub buffer_ms: u32,
}

pub struct RxHandle {
    stop: Arc<AtomicBool>,
    status: Text,
    peak: Arc<AtomicU32>,
}

impl RxHandle {
    pub fn start(cfg: &RxCfg) -> RxHandle {
        let stop = Arc::new(AtomicBool::new(false));
        let status: Text = Arc::new(Mutex::new("Starting...".into()));
        let peak = Arc::new(AtomicU32::new(0));
        let q = Queue::new(cfg.buffer_ms.clamp(20, 2000));
        let card: Text = Arc::new(Mutex::new(String::new()));
        card_thread(cfg.device.clone(), q.clone(), stop.clone(), card.clone(), "Sound card");
        let (st, stt, pk, port, tcp) = (stop.clone(), status.clone(), peak.clone(), cfg.port, cfg.proto == "tcp");
        std::thread::spawn(move || {
            let mut samples: Vec<f32> = Vec::with_capacity(4096);
            let mut shown = Instant::now();
            let show = |stt: &Text, from: &str, q: &Queue, card: &Text| {
                set(stt, &format!("Receiving from {from}, buffer {} ms; {}", q.ms(), get(card)));
            };
            if tcp {
                let l = match TcpListener::bind(("0.0.0.0", port)) {
                    Ok(l) => l,
                    Err(e) => {
                        set(&stt, &format!("Cannot listen on port {port}: {e}"));
                        return;
                    }
                };
                let _ = l.set_nonblocking(true);
                'accept: while !st.load(Ordering::Relaxed) {
                    set(&stt, &format!("Waiting for a sender on TCP port {port} ..."));
                    let (mut s, who) = loop {
                        if st.load(Ordering::Relaxed) {
                            break 'accept;
                        }
                        match l.accept() {
                            Ok(x) => break x,
                            Err(_) => std::thread::sleep(Duration::from_millis(100)),
                        }
                    };
                    let _ = s.set_nonblocking(false);
                    let _ = s.set_read_timeout(Some(Duration::from_millis(300)));
                    let mut buf = vec![0u8; 8192];
                    let mut rest: Vec<u8> = Vec::new();
                    while !st.load(Ordering::Relaxed) {
                        match s.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => {
                                rest.extend_from_slice(&buf[..n]);
                                let usable = rest.len() / 3 * 3;
                                samples.clear();
                                from_l24(&rest[..usable], &mut samples);
                                rest.drain(..usable);
                                track_peak(&samples, &pk);
                                q.push(&samples);
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {}
                            Err(_) => break,
                        }
                        if shown.elapsed() > Duration::from_millis(500) {
                            shown = Instant::now();
                            show(&stt, &who.to_string(), &q, &card);
                        }
                    }
                }
            } else {
                let sock = match UdpSocket::bind(("0.0.0.0", port)) {
                    Ok(s) => s,
                    Err(e) => {
                        set(&stt, &format!("Cannot listen on UDP port {port}: {e}"));
                        return;
                    }
                };
                let _ = sock.set_read_timeout(Some(Duration::from_millis(300)));
                let mut buf = vec![0u8; 4096];
                let mut from = String::from("-");
                while !st.load(Ordering::Relaxed) {
                    match sock.recv_from(&mut buf) {
                        Ok((n, a)) if n > 12 && buf[0] >> 6 == 2 => {
                            from = a.to_string();
                            samples.clear();
                            from_l24(&buf[12..n], &mut samples);
                            track_peak(&samples, &pk);
                            q.push(&samples);
                        }
                        _ => {
                            if shown.elapsed() > Duration::from_secs(2) && q.len() == 0 {
                                set(&stt, &format!("Waiting for packets on UDP port {port} ..."));
                            }
                        }
                    }
                    if shown.elapsed() > Duration::from_millis(500) && q.len() > 0 {
                        shown = Instant::now();
                        show(&stt, &from, &q, &card);
                    }
                }
            }
        });
        RxHandle { stop, status, peak }
    }

    pub fn status_text(&self) -> String {
        get(&self.status)
    }

    pub fn peak(&self) -> f32 {
        f32::from_bits(self.peak.load(Ordering::Relaxed))
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

