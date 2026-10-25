//! radiomoon-client: the RadioMoon studio window for a radiomoon-server. The server plays (sound card, schedule, streams);
//! this window only shows and controls it, so closing it never stops the broadcast.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use base64::Engine;
use eframe::egui;
use serde_json::{json, Value};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[allow(dead_code)]
enum Cmd {
    Act(Value),
    Lib(String, String),
    Pal,
    Quit,
}

#[derive(Default)]
struct Shared {
    state: Option<Value>,
    lib: Option<Value>,
    pal: Option<Value>,
    user: String,
    admin: bool,
    err: String,
    denied: bool,
    online: bool,
}

struct Link {
    tx: mpsc::Sender<Cmd>,
    sh: Arc<Mutex<Shared>>,
}

fn enc(s: &str) -> String {
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

fn connect(url: String, user: String, pass: String, ctx: egui::Context) -> Link {
    let (tx, rx) = mpsc::channel::<Cmd>();
    let sh = Arc::new(Mutex::new(Shared::default()));
    let sh2 = sh.clone();
    std::thread::spawn(move || {
        let base = url.trim().trim_end_matches('/').to_string();
        let base = if base.starts_with("http://") || base.starts_with("https://") { base } else { format!("http://{base}") };
        let auth = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}")));
        let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(4)).timeout_read(Duration::from_secs(8)).build();
        let call = |post: Option<&Value>, path: &str| -> Result<Value, String> {
            let u = format!("{base}{path}");
            let r = match post {
                Some(b) => agent.post(&u).set("Authorization", &auth).set("Content-Type", "application/json").send_string(&b.to_string()),
                None => agent.get(&u).set("Authorization", &auth).call(),
            };
            match r {
                Ok(r) => {
                    let t = r.into_string().map_err(|e| e.to_string())?;
                    serde_json::from_str::<Value>(&t).map_err(|e| e.to_string())
                }
                Err(ureq::Error::Status(401, _)) => Err("401".into()),
                Err(ureq::Error::Status(c, _)) => Err(format!("server answered {c}")),
                Err(e) => Err(e.to_string()),
            }
        };
        // login first: a wrong password is reported at once
        match call(None, "/api/login") {
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
            Err(e) => {
                sh2.lock().unwrap().err = e;
            }
        }
        let mut first = true;
        loop {
            // commands from the window
            loop {
                match rx.try_recv() {
                    Ok(Cmd::Quit) => return,
                    Ok(Cmd::Act(b)) => match call(Some(&b), "/api/action") {
                        Ok(v) => sh2.lock().unwrap().state = Some(v),
                        Err(e) => sh2.lock().unwrap().err = e,
                    },
                    Ok(Cmd::Lib(q, kind)) => {
                        if let Ok(v) = call(None, &format!("/api/library?kind={}&q={}&limit=150", enc(&kind), enc(&q))) {
                            sh2.lock().unwrap().lib = Some(v);
                        }
                    }
                    Ok(Cmd::Pal) => {
                        if let Ok(v) = call(None, "/api/pallets") {
                            sh2.lock().unwrap().pal = Some(v);
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => return,
                }
            }
            match call(None, "/api/state") {
                Ok(v) => {
                    let mut g = sh2.lock().unwrap();
                    g.state = Some(v);
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
            if first {
                first = false;
                if let Ok(v) = call(None, "/api/pallets") {
                    sh2.lock().unwrap().pal = Some(v);
                }
                if let Ok(v) = call(None, "/api/library?kind=music&limit=150") {
                    sh2.lock().unwrap().lib = Some(v);
                }
            }
            ctx.request_repaint();
            std::thread::sleep(Duration::from_millis(500));
        }
    });
    Link { tx, sh }
}

fn settings_file() -> std::path::PathBuf {
    let dir = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    dir.join("radiomoon-client.json")
}

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}
fn n(v: &Value, k: &str) -> u64 {
    v[k].as_u64().unwrap_or(0)
}
fn fm(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

struct ClientApp {
    url: String,
    user: String,
    pass: String,
    link: Option<Link>,
    vol: f32,
    q: String,
    kind: String,
    pidx: usize,
    msg: String,
}

impl ClientApp {
    fn new() -> Self {
        let saved: Value = std::fs::read_to_string(settings_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        ClientApp {
            url: saved["url"].as_str().unwrap_or("http://127.0.0.1:8080").to_string(),
            user: saved["user"].as_str().unwrap_or("admin").to_string(),
            pass: String::new(),
            link: None,
            vol: 1.0,
            q: String::new(),
            kind: "music".into(),
            pidx: 0,
            msg: String::new(),
        }
    }

    fn act(&self, v: Value) {
        if let Some(l) = &self.link {
            let _ = l.tx.send(Cmd::Act(v));
        }
    }

    fn login_screen(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.heading(format!("RadioMoon client {VERSION}"));
                ui.label("Connect to a RadioMoon server");
                ui.add_space(16.0);
                egui::Grid::new("login").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label("Server");
                    ui.add(egui::TextEdit::singleline(&mut self.url).desired_width(280.0).hint_text("http://server:8080"));
                    ui.end_row();
                    ui.label("User name");
                    ui.add(egui::TextEdit::singleline(&mut self.user).desired_width(280.0));
                    ui.end_row();
                    ui.label("Password");
                    let r = ui.add(egui::TextEdit::singleline(&mut self.pass).password(true).desired_width(280.0));
                    ui.end_row();
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.start(ctx);
                    }
                });
                ui.add_space(12.0);
                if ui.add_sized([160.0, 32.0], egui::Button::new("Connect")).clicked() {
                    self.start(ctx);
                }
                if !self.msg.is_empty() {
                    ui.add_space(10.0);
                    ui.colored_label(egui::Color32::from_rgb(229, 83, 75), &self.msg);
                }
            });
        });
    }

    fn start(&mut self, ctx: &egui::Context) {
        let _ = std::fs::write(settings_file(), json!({ "url": self.url, "user": self.user }).to_string());
        self.msg.clear();
        self.link = Some(connect(self.url.clone(), self.user.clone(), self.pass.clone(), ctx.clone()));
    }

    fn studio(&mut self, ctx: &egui::Context, st: Value, lib: Option<Value>, pal: Option<Value>, online: bool, err: String, user: String) {
        let acc = egui::Color32::from_rgb(240, 162, 58);
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(if s(&st, "station").is_empty() { "RadioMoon".to_string() } else { s(&st, "station") });
                ui.weak(format!("server {}  -  {}", s(&st, "version"), user));
                ui.separator();
                let mode = s(&st, "mode");
                for (k, label) in [("auto", "Auto"), ("manual", "Manual"), ("offline", "Offline")] {
                    if ui.selectable_label(mode == k, label).clicked() {
                        self.act(json!({ "do": "mode", "mode": k }));
                    }
                }
                if st["live"].as_bool().unwrap_or(false) {
                    ui.colored_label(egui::Color32::from_rgb(229, 83, 75), "LIVE");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Disconnect").clicked() {
                        if let Some(l) = self.link.take() {
                            let _ = l.tx.send(Cmd::Quit);
                        }
                    }
                    if online {
                        ui.colored_label(egui::Color32::from_rgb(70, 192, 122), "connected");
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(229, 83, 75), format!("no connection: {err}"));
                    }
                });
            });
        });

        egui::SidePanel::right("lib").min_width(320.0).default_width(380.0).show(ctx, |ui| {
            ui.heading("Library");
            let mut changed = false;
            ui.horizontal(|ui| {
                changed |= ui.add(egui::TextEdit::singleline(&mut self.q).hint_text("Search").desired_width(180.0)).changed();
                let kinds: Vec<String> = lib.as_ref().and_then(|l| l["kinds"].as_array()).map(|a| a.iter().filter_map(|k| k.as_str().map(|x| x.to_string())).collect()).unwrap_or_else(|| vec!["music".into()]);
                egui::ComboBox::from_id_salt("kind").selected_text(self.kind.clone()).show_ui(ui, |ui| {
                    for k in kinds {
                        if ui.selectable_value(&mut self.kind, k.clone(), k).changed() {
                            changed = true;
                        }
                    }
                });
            });
            if changed {
                if let Some(l) = &self.link {
                    let _ = l.tx.send(Cmd::Lib(self.q.clone(), self.kind.clone()));
                }
            }
            egui::ScrollArea::vertical().id_salt("libscroll").max_height(ui.available_height() * 0.62).show(ui, |ui| {
                if let Some(items) = lib.as_ref().and_then(|l| l["items"].as_array()) {
                    for t in items {
                        ui.horizontal(|ui| {
                            if ui.small_button("Add").clicked() {
                                self.act(json!({ "do": "add", "id": n(t, "id"), "where": "end" }));
                            }
                            if ui.small_button("Next").clicked() {
                                self.act(json!({ "do": "add", "id": n(t, "id"), "where": "next" }));
                            }
                            ui.weak(s(t, "durs"));
                            ui.add(egui::Label::new(s(t, "label")).truncate());
                        });
                    }
                }
            });
            ui.separator();
            ui.heading("Played");
            egui::ScrollArea::vertical().id_salt("histscroll").show(ui, |ui| {
                if let Some(h) = st["history"].as_array() {
                    for x in h {
                        ui.horizontal(|ui| {
                            ui.weak(s(x, "time"));
                            ui.add(egui::Label::new(s(x, "label")).truncate());
                        });
                    }
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // on air
            let now = &st["now"];
            ui.label(egui::RichText::new(if now.is_null() { "Nothing is playing".to_string() } else { s(now, "label") }).size(22.0).strong());
            let (el, du) = (n(&st, "elapsed"), if now.is_null() { 0 } else { n(now, "dur") });
            ui.horizontal(|ui| {
                ui.label(format!("{} / {}", fm(el), fm(du)));
                ui.add(egui::ProgressBar::new(if du > 0 { (el as f32 / du as f32).clamp(0.0, 1.0) } else { 0.0 }).fill(acc).desired_width(ui.available_width()));
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Restart").clicked() {
                    self.act(json!({ "do": "restart" }));
                }
                if ui.selectable_label(st["paused"].as_bool().unwrap_or(false), "Pause").clicked() {
                    self.act(json!({ "do": "pause" }));
                }
                if ui.button("Next").clicked() {
                    self.act(json!({ "do": "next" }));
                }
                if ui.button("Stop").clicked() {
                    self.act(json!({ "do": "stop" }));
                }
                if ui.selectable_label(st["loop"].as_bool().unwrap_or(false), "Loop").clicked() {
                    self.act(json!({ "do": "loop" }));
                }
                if ui.selectable_label(st["stopnext"].as_bool().unwrap_or(false), "Stop after track").clicked() {
                    self.act(json!({ "do": "stopnext" }));
                }
                if ui.selectable_label(st["live"].as_bool().unwrap_or(false), "Live").clicked() {
                    self.act(json!({ "do": "live" }));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Volume");
                let r = ui.add(egui::Slider::new(&mut self.vol, 0.0..=1.5).show_value(true));
                if r.drag_stopped() || (r.changed() && !r.dragged()) {
                    self.act(json!({ "do": "volume", "value": self.vol }));
                } else if !r.dragged() && !r.has_focus() {
                    self.vol = st["volume"].as_f64().unwrap_or(1.0) as f32;
                }
            });
            let status = s(&st, "status");
            if !status.is_empty() {
                ui.weak(status);
            }
            ui.separator();

            // playlist
            ui.horizontal(|ui| {
                ui.heading("Playlist");
                if ui.button("Rebuild automatic").clicked() {
                    self.act(json!({ "do": "regen" }));
                }
                if ui.button("Clear").clicked() {
                    self.act(json!({ "do": "clear" }));
                }
            });
            let cur = st["current"].as_u64();
            let h = (ui.available_height() - 150.0).max(120.0);
            egui::ScrollArea::vertical().id_salt("logscroll").max_height(h).auto_shrink([false, false]).show(ui, |ui| {
                if let Some(log) = st["log"].as_array() {
                    for t in log {
                        let i = n(t, "i");
                        ui.horizontal(|ui| {
                            if ui.small_button("▶").clicked() {
                                self.act(json!({ "do": "play", "index": i }));
                            }
                            if ui.small_button("↑").clicked() {
                                self.act(json!({ "do": "up", "index": i }));
                            }
                            if ui.small_button("↓").clicked() {
                                self.act(json!({ "do": "down", "index": i }));
                            }
                            if ui.small_button("✕").clicked() {
                                self.act(json!({ "do": "remove", "index": i }));
                            }
                            ui.weak(s(t, "start"));
                            let txt = egui::RichText::new(s(t, "label"));
                            ui.label(if cur == Some(i) { txt.color(acc).strong() } else { txt });
                            ui.weak(format!("{}  {}", s(t, "kind"), s(t, "durs")));
                        });
                    }
                }
            });
            ui.separator();

            // instant jingles
            ui.heading("Instant jingles");
            if let Some(pl) = pal.as_ref().and_then(|p| p["pallets"].as_array()) {
                ui.horizontal_wrapped(|ui| {
                    for (i, p) in pl.iter().enumerate() {
                        if ui.selectable_label(i == self.pidx, s(p, "name")).clicked() {
                            self.pidx = i;
                        }
                    }
                    if ui.button("Stop jingles").clicked() {
                        self.act(json!({ "do": "stopjingles" }));
                    }
                });
                if let Some(btns) = pl.get(self.pidx).and_then(|p| p["buttons"].as_array()) {
                    ui.horizontal_wrapped(|ui| {
                        for b in btns {
                            let c = b["color"].as_array().map(|a| a.iter().map(|x| x.as_u64().unwrap_or(60) as u8).collect::<Vec<u8>>()).unwrap_or_default();
                            let col = if c.len() == 3 { egui::Color32::from_rgb(c[0], c[1], c[2]) } else { egui::Color32::from_rgb(70, 74, 92) };
                            if ui.add(egui::Button::new(s(b, "label")).fill(col).min_size(egui::vec2(110.0, 34.0))).clicked() {
                                self.act(json!({ "do": "jingle", "id": n(b, "id") }));
                            }
                        }
                    });
                }
            }
        });
    }
}

impl eframe::App for ClientApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(500));
        let snap = self.link.as_ref().map(|l| {
            let g = l.sh.lock().unwrap();
            (g.state.clone(), g.lib.clone(), g.pal.clone(), g.online, g.err.clone(), g.denied, g.user.clone())
        });
        match snap {
            None => self.login_screen(ctx),
            Some((_, _, _, _, err, true, _)) => {
                self.link = None;
                self.msg = err;
                self.login_screen(ctx);
            }
            Some((None, _, _, _, err, _, _)) => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.heading("Connecting...");
                        if !err.is_empty() {
                            ui.colored_label(egui::Color32::from_rgb(229, 83, 75), err);
                        }
                        if ui.button("Cancel").clicked() {
                            if let Some(l) = self.link.take() {
                                let _ = l.tx.send(Cmd::Quit);
                            }
                        }
                    });
                });
            }
            Some((Some(st), lib, pal, online, err, _, user)) => self.studio(ctx, st, lib, pal, online, err, user),
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1250.0, 760.0]).with_min_inner_size([800.0, 520.0]).with_title(format!("RadioMoon client {VERSION}")),
        ..Default::default()
    };
    eframe::run_native("RadioMoon client", options, Box::new(|_cc| Ok(Box::new(ClientApp::new()))))
}
