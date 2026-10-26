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
            pass: String::new(),
            msg: String::new(),
            app: None,
            data,
            exe_dir,
        }
    }

    fn connect(&mut self, ctx: &egui::Context) {
        self.msg.clear();
        let _ = std::fs::write(
            self.data.join("client.json"),
            json!({ "url": self.url, "user": self.user }).to_string(),
        );
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
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.heading(format!("RadioMoon client {VERSION}"));
                ui.label("The studio of a RadioMoon server");
                ui.add_space(14.0);
                let mut go = false;
                egui::Grid::new("login").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label("Server");
                    ui.add(egui::TextEdit::singleline(&mut self.url).desired_width(300.0).hint_text("http://server:8080"));
                    ui.end_row();
                    ui.label("User name");
                    ui.add(egui::TextEdit::singleline(&mut self.user).desired_width(300.0));
                    ui.end_row();
                    ui.label("Password");
                    let r = ui.add(egui::TextEdit::singleline(&mut self.pass).password(true).desired_width(300.0));
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        go = true;
                    }
                    ui.end_row();
                });
                ui.add_space(6.0);
                ui.add_space(12.0);
                if ui.add_sized([180.0, 34.0], egui::Button::new("Connect")).clicked() {
                    go = true;
                }
                if go {
                    self.connect(ctx);
                }
                if !self.msg.is_empty() {
                    ui.add_space(10.0);
                    ui.colored_label(egui::Color32::from_rgb(229, 83, 75), &self.msg);
                }
            });
        });
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
