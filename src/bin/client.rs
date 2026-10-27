//! radiomoon-client: the RadioMoon studio for a radiomoon-server. It is the same window as the desktop program, but the server plays
//! (sound card, schedule, streams): this window shows the studio, sends the buttons to the server and plays nothing, so closing it
//! never stops the broadcast. Songs are uploaded to the server and can be listened to from it.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![allow(dead_code)]

#[path = "../app.rs"]
mod app;
#[path = "../backup.rs"]
mod backup;
#[path = "../clap_host.rs"]
mod clap_host;
#[path = "../config.rs"]
mod config;
#[path = "../control.rs"]
mod control;
#[path = "../db.rs"]
mod db;
#[path = "../dbserver.rs"]
mod dbserver;
#[path = "../engine.rs"]
mod engine;
#[path = "../export.rs"]
mod export;
#[path = "../i18n.rs"]
mod i18n;
#[path = "../jzimport.rs"]
mod jzimport;
#[path = "../kit.rs"]
mod kit;
#[path = "../library.rs"]
mod library;
#[path = "../mdb.rs"]
mod mdb;
#[path = "../model.rs"]
mod model;
#[path = "../player.rs"]
mod player;
#[path = "../remote.rs"]
mod remote;
#[path = "../schema.rs"]
mod schema;
#[path = "../stream.rs"]
mod stream;
#[path = "../studio.rs"]
mod studio;
#[path = "../waveform.rs"]
mod waveform;

pub use ::rfd;

use eframe::egui;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn settings_dir() -> PathBuf {
    let dir = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("RadioMoon-Client");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

struct Shell {
    url: String,
    user: String,
    pass: String,
    remember: bool,
    bw: u8,
    /// servers used before: (address, user name, encrypted password or empty)
    history: Vec<(String, String, String)>,
    first: bool,
    msg: String,
    app: Option<app::App>,
    data: PathBuf,
    exe_dir: PathBuf,
}

impl Shell {
    fn new() -> Self {
        let data = settings_dir();
        let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|x| x.to_path_buf())).unwrap_or_else(|| PathBuf::from("."));
        let saved: Value = std::fs::read_to_string(data.join("client.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        let g = |k: &str, d: &str| saved[k].as_str().unwrap_or(d).to_string();
        Shell {
            url: g("url", "http://127.0.0.1:8080"),
            user: g("user", "admin"),
            pass: saved["pass"].as_str().and_then(config::decrypt).unwrap_or_default(),
            remember: saved["remember"].as_bool().unwrap_or(false),
            bw: saved["bw"].as_u64().unwrap_or(0).min(2) as u8,
            history: saved["servers"]
                .as_array()
                .map(|a| a.iter().map(|x| (x["url"].as_str().unwrap_or("").to_string(), x["user"].as_str().unwrap_or("").to_string(), x["pass"].as_str().unwrap_or("").to_string())).filter(|x| !x.0.is_empty()).collect())
                .unwrap_or_default(),
            first: true,
            msg: String::new(),
            app: None,
            data,
            exe_dir,
        }
    }

    fn connect(&mut self, ctx: &egui::Context) {
        self.msg.clear();
        // the list of the servers that were used (the last one first); the password only when it is remembered
        let enc = if self.remember { config::encrypt(&self.pass) } else { String::new() };
        self.history.retain(|h| !(h.0 == self.url && h.1 == self.user));
        self.history.insert(0, (self.url.clone(), self.user.clone(), enc.clone()));
        self.history.truncate(12);
        let servers: Vec<Value> = self.history.iter().map(|h| json!({ "url": h.0, "user": h.1, "pass": h.2 })).collect();
        let _ = std::fs::write(
            self.data.join("client.json"),
            (if self.remember { json!({ "url": self.url, "user": self.user, "remember": true, "bw": self.bw, "pass": enc, "servers": servers }) } else { json!({ "url": self.url, "user": self.user, "remember": false, "bw": self.bw, "servers": servers }) }).to_string(),
        );
        remote::LOW_BW.store(self.bw, std::sync::atomic::Ordering::Relaxed);
        let link = remote::connect(self.url.clone(), self.user.clone(), self.pass.clone(), ctx.clone());
        let t0 = Instant::now();
        let (user, admin) = loop {
            {
                let g = link.sh.lock().unwrap();
                if g.denied {
                    self.msg = tr_msg("Wrong user name or password");
                    return;
                }
                if g.online {
                    break (g.user.clone(), g.admin);
                }
                if !g.err.is_empty() || t0.elapsed() > Duration::from_secs(8) {
                    self.msg = format!("Cannot reach the server: {}", if g.err.is_empty() { "no answer".to_string() } else { g.err.clone() });
                    drop(g);
                    let _ = link.tx.send(remote::Cmd::Quit);
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        };

        // the database of the server is reached through the server itself (only the address, user name and password are needed)
        let info = match remote::start_db_proxy(&link.base, &link.auth) {
            Ok(i) => i,
            Err(e) => {
                self.msg = format!("Connected to the server, but its database cannot be opened:\n{e}");
                let _ = link.tx.send(remote::Cmd::Quit);
                return;
            }
        };
        let cfg = config::Config { host: "127.0.0.1".into(), port: info.port, user: info.user, password: info.password, database: info.database };
        let db = match db::Db::connect(&cfg) {
            Ok(d) => d,
            Err(e) => {
                self.msg = format!("The server's database did not answer:\n{e:#}");
                let _ = link.tx.send(remote::Cmd::Quit);
                return;
            }
        };
        // every server has its own folder (settings, cache of songs): the numbers of the songs are the same in different servers
        let slug: String = format!("{}_{}", remote::host_of(&self.url), self.url.rsplit(':').next().filter(|p| p.chars().all(|c| c.is_ascii_digit())).unwrap_or("80")).chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
        let sdata = self.data.join("servers").join(slug);
        let _ = std::fs::create_dir_all(&sdata);
        app::REMOTE_MODE.store(true, std::sync::atomic::Ordering::SeqCst);
        let mut a = app::App::new(dbserver::DbServer::none(), db, cfg, sdata.clone(), self.exe_dir.clone(), String::new());
        // a sound output of this computer only for listening to songs; the broadcast is played by the server
        if let Ok(p) = player::Player::new("", "", "") {
            a.player = Some(p);
        }
        a.user = Some(match a.data.users.iter().find(|u| u.name.eq_ignore_ascii_case(&user)) {
            Some(u) => u.clone(),
            None => model::User { name: user, admin, ..Default::default() },
        });
        a.status = "Connected to the server".into();
        a.remote = Some(remote::Remote::new(link));
        self.app = Some(a);
    }
}

fn tr_msg(s: &str) -> String {
    s.to_string()
}

impl eframe::App for Shell {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(a) = self.app.as_mut() {
            eframe::App::update(a, ctx, frame);
            return;
        }
        if self.first {
            self.first = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
        let mut go = false;
        egui::CentralPanel::default().frame(egui::Frame::none().fill(kit::BG)).show(ctx, |ui| {
            let full = ui.max_rect();
            let painter = ui.painter();
            // background: a dark blue gradient, a big faint moon and slow equalizer bars
            let mut mesh = egui::Mesh::default();
            let (top, bottom) = (egui::Color32::from_rgb(0x08, 0x0b, 0x10), egui::Color32::from_rgb(0x12, 0x26, 0x40));
            mesh.colored_vertex(full.left_top(), top);
            mesh.colored_vertex(full.right_top(), top);
            mesh.colored_vertex(full.right_bottom(), bottom);
            mesh.colored_vertex(full.left_bottom(), bottom);
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            painter.add(egui::Shape::mesh(mesh));
            let t = ui.input(|i| i.time) as f32;
            let r = full.height().min(full.width()) * 0.55;
            kit::draw_moon(painter, egui::pos2(full.right() - r * 0.55, full.top() + r * 0.7), r, egui::Color32::from_rgba_unmultiplied(56, 182, 255, 22), egui::Color32::from_rgb(0x0c, 0x14, 0x20));
            let n = (full.width() / 22.0) as i32;
            for i in 0..n {
                let h = (0.5 + 0.5 * (t * 0.9 + i as f32 * 0.55).sin() * (t * 0.37 + i as f32 * 0.21).cos()).abs() * full.height() * 0.22 + 6.0;
                let x = full.left() + 11.0 + i as f32 * 22.0;
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(x - 6.0, full.bottom() - h), egui::pos2(x + 6.0, full.bottom())), 3.0, egui::Color32::from_rgba_unmultiplied(56, 182, 255, 26));
            }
            ui.ctx().request_repaint_after(Duration::from_millis(60));

            // the login card in the centre
            let card = egui::Rect::from_center_size(full.center(), egui::vec2(400.0_f32.min(full.width() - 24.0), 470.0_f32.min(full.height() - 16.0)));
            ui.painter().rect_filled(card.translate(egui::vec2(0.0, 6.0)).expand(2.0), 18.0, egui::Color32::from_black_alpha(90));
            ui.painter().rect(card, 16.0, kit::CARD, egui::Stroke::new(1.0_f32, kit::BORDER));
            let inner = card.shrink2(egui::vec2(30.0, 24.0));
            let mut cui = ui.child_ui(inner, egui::Layout::top_down(egui::Align::Min), None);
            cui.vertical_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(60.0, 60.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 29.0, egui::Color32::from_rgb(0x0e, 0x2a, 0x44));
                kit::draw_moon(ui.painter(), r.center(), 20.0, kit::ACCENT, egui::Color32::from_rgb(0x0e, 0x2a, 0x44));
                ui.add_space(6.0);
                ui.label(egui::RichText::new("RadioMoon").size(26.0).strong().color(egui::Color32::WHITE));
                ui.label(egui::RichText::new(format!("Studio client {VERSION}")).color(kit::TEXT_DIM));
            });
            cui.add_space(18.0);
            let field = |ui: &mut egui::Ui, label: &str, text: &mut String, pw: bool, hint: &str| -> bool {
                ui.label(egui::RichText::new(label).small().color(kit::TEXT_DIM));
                let r = ui.add(egui::TextEdit::singleline(text).password(pw).hint_text(hint).desired_width(f32::INFINITY).margin(egui::vec2(10.0, 8.0)));
                ui.add_space(8.0);
                r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
            };
            cui.label(egui::RichText::new("SERVER").small().color(kit::TEXT_DIM));
            cui.horizontal(|ui| {
                let w = ui.available_width() - 44.0;
                let r = ui.add(egui::TextEdit::singleline(&mut self.url).hint_text("http://server:8080").desired_width(w).margin(egui::vec2(10.0, 8.0)));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    go = true;
                }
                // the servers that were used before
                let mut pick: Option<usize> = None;
                let mut clear = false;
                egui::ComboBox::from_id_salt("server_history").width(30.0).selected_text("\u{25BC}").show_ui(ui, |ui| {
                    if self.history.is_empty() {
                        ui.label(egui::RichText::new("No server yet").color(kit::TEXT_DIM));
                    }
                    for (i, h) in self.history.iter().enumerate() {
                        if ui.selectable_label(false, format!("{}  ({})", h.0, h.1)).clicked() {
                            pick = Some(i);
                        }
                    }
                    if !self.history.is_empty() {
                        ui.separator();
                        if ui.selectable_label(false, egui::RichText::new("Clear the list").color(kit::TEXT_DIM)).clicked() {
                            clear = true;
                        }
                    }
                });
                if let Some(i) = pick {
                    let h = self.history[i].clone();
                    self.url = h.0;
                    self.user = h.1;
                    match config::decrypt(&h.2) {
                        Some(p) if !h.2.is_empty() => {
                            self.pass = p;
                            self.remember = true;
                        }
                        _ => self.pass.clear(),
                    }
                }
                if clear {
                    self.history.clear();
                }
            });
            cui.add_space(8.0);
            go |= field(&mut cui, "USER NAME", &mut self.user, false, "");
            go |= field(&mut cui, "PASSWORD", &mut self.pass, true, "");
            cui.label(egui::RichText::new("CONNECTION").small().color(kit::TEXT_DIM));
            egui::ComboBox::from_id_salt("login_bw").width(cui.available_width()).selected_text(remote::bw_name(self.bw)).show_ui(&mut cui, |ui| {
                for k in 0..3u8 {
                    ui.selectable_value(&mut self.bw, k, remote::bw_name(k));
                }
            });
            cui.add_space(8.0);
            cui.checkbox(&mut self.remember, "Remember the password on this computer");
            cui.add_space(12.0);
            let w = cui.available_width();
            let b = cui.add_sized([w, 40.0], egui::Button::new(egui::RichText::new("Connect").size(16.0).strong().color(egui::Color32::WHITE)).fill(kit::ACCENT.gamma_multiply(0.85)).rounding(8.0));
            if b.clicked() {
                go = true;
            }
            if !self.msg.is_empty() {
                cui.add_space(10.0);
                cui.colored_label(egui::Color32::from_rgb(255, 120, 110), &self.msg);
            }
        });
        if go {
            self.connect(ctx);
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1250.0, 760.0]).with_min_inner_size([800.0, 480.0]).with_maximized(true).with_title(format!("RadioMoon client {VERSION}")),
        ..Default::default()
    };
    eframe::run_native("RadioMoon client", options, Box::new(|cc| {
        kit::apply_style(&cc.egui_ctx);
        Ok(Box::new(Shell::new()))
    }))
}
