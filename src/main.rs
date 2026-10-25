#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod backup;
mod boot;
mod config;
mod control;
mod db;
mod dbserver;
mod engine;
mod export;
mod i18n;
mod jzimport;
mod kit;
mod library;
mod model;
mod mdb;
mod player;
mod schema;
mod stream;
mod studio;
mod waveform;

#[cfg(feature = "desktop")]
pub use ::rfd;
use app::App;
use boot::{data_dir, open_db, tcp_open, COMMON_PORTS};
use db::Db;
use eframe::egui;
use std::path::PathBuf;

const WHITE: egui::Color32 = egui::Color32::WHITE;

/// Shown when the database cannot be reached at start: the connection can be entered and tested here.
struct ConnectApp {
    cfg: config::Config,
    error: String,
    status: String,
    data: PathBuf,
    exe_dir: PathBuf,
}

impl ConnectApp {
    fn test(&mut self) -> bool {
        let r = (|| -> anyhow::Result<()> {
            let (_s, _d, used) = open_db(&self.cfg, &self.exe_dir, &self.data)?;
            self.cfg = used;
            Ok(())
        })();
        match r {
            Ok(()) => {
                self.status = i18n::tr("Connection works.");
                true
            }
            Err(e) => {
                self.status = format!("{e:#}");
                false
            }
        }
    }
}

impl eframe::App for ConnectApp {
    fn update(&mut self, ctx: &egui::Context, _f: &mut eframe::Frame) {
        let tr = i18n::tr;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(egui::RichText::new(tr("RadioMoon could not connect to the database")).color(WHITE));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(&self.error).color(egui::Color32::from_rgb(255, 140, 120)));
            ui.add_space(10.0);
            ui.label(tr("Enter the connection of your MariaDB / MySQL server:"));
            ui.add_space(4.0);
            egui::Grid::new("conn").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(tr("Server address"));
                ui.add(egui::TextEdit::singleline(&mut self.cfg.host).desired_width(260.0));
                ui.end_row();
                ui.label(tr("Port"));
                ui.add(egui::DragValue::new(&mut self.cfg.port).range(1..=65535));
                ui.end_row();
                ui.label(tr("User"));
                ui.add(egui::TextEdit::singleline(&mut self.cfg.user).desired_width(260.0));
                ui.end_row();
                ui.label(tr("Password"));
                ui.add(egui::TextEdit::singleline(&mut self.cfg.password).password(true).desired_width(260.0));
                ui.end_row();
                ui.label(tr("Database name"));
                ui.add(egui::TextEdit::singleline(&mut self.cfg.database).desired_width(260.0));
                ui.end_row();
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Test connection")).clicked() {
                    self.test();
                }
                if ui.button(tr("Save and start RadioMoon")).clicked() {
                    if self.test() {
                        config::save(&self.data, &self.cfg);
                        if let Ok(exe) = std::env::current_exe() {
                            let _ = std::process::Command::new(exe).spawn();
                        }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                if ui.button(tr("Save only")).clicked() {
                    config::save(&self.data, &self.cfg);
                    self.status = tr("Saved.");
                }
            });
            ui.add_space(8.0);
            if !self.status.is_empty() {
                ui.label(&self.status);
            }
            ui.add_space(10.0);
            if !cfg!(windows) {
                ui.label(egui::RichText::new(tr("System MariaDB / MySQL (Linux, macOS): the port is usually 3306. The root user of the system server often cannot log in with a password; create a user, for example: sudo mariadb -e \"CREATE USER 'radiomoon'@'localhost' IDENTIFIED BY 'password'; GRANT ALL ON *.* TO 'radiomoon'@'localhost'; FLUSH PRIVILEGES;\"")).small().color(egui::Color32::GRAY));
                ui.add_space(4.0);
            }
            ui.label(egui::RichText::new(tr("For the database server of the installer (Windows service RadioMoonDB) use address 127.0.0.1, port 3307, user root, empty password.")).small().color(egui::Color32::GRAY));
        });
    }
}

fn main() -> eframe::Result<()> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|x| x.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let data = data_dir(&exe_dir);
    let cfg = config::load(&data);

    let orig_port = cfg.port;
    let startup = (|| -> anyhow::Result<(dbserver::DbServer, Db, String, config::Config)> {
        let (server, db, used) = open_db(&cfg, &exe_dir, &data)?;
        let msg = db.ensure_schema(app::VERSION, &data.join("backups"))?;
        Ok((server, db, msg, used))
    })();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 900.0])
            .with_min_inner_size([800.0, 480.0])
            .with_maximized(true)
            .with_title(format!("RadioMoon {}", app::VERSION))
            .with_icon(std::sync::Arc::new(egui::IconData {
                rgba: image_rgba(),
                width: 256,
                height: 256,
            })),
        ..Default::default()
    };

    match startup {
        Ok((server, db, msg, cfg)) => {
            if cfg.port != orig_port {
                config::save(&data, &cfg); // the port that answered is remembered
            }
            eframe::run_native(
            "RadioMoon",
            options,
            Box::new(move |cc| {
                kit::apply_style(&cc.egui_ctx);
                Ok(Box::new(App::new(server, db, cfg, data, exe_dir, msg)))
            }),
            )
        }
        Err(e) => {
            // suggest the port of a database that answers on this computer
            let mut cfg = cfg;
            if cfg.is_local() && !tcp_open("127.0.0.1", cfg.port) {
                if let Some(p) = COMMON_PORTS.iter().find(|p| tcp_open("127.0.0.1", **p)) {
                    cfg.port = *p;
                }
            }
            // the interface language is stored in the database, which is not reachable here: Greek is used when the system language is Greek
            if std::env::var("LANG").map(|l| l.starts_with("el")).unwrap_or(false) || std::env::var("LANGUAGE").map(|l| l.starts_with("el")).unwrap_or(false) {
                i18n::set_language("el", &[exe_dir.join("lang")]);
            }
            eframe::run_native(
                "RadioMoon",
                options,
                Box::new(move |cc| {
                    kit::apply_style(&cc.egui_ctx);
                    Ok(Box::new(ConnectApp { cfg, error: format!("{e:#}"), status: String::new(), data, exe_dir }))
                }),
            )
        }
    }
}

/// Window icon (256x256 RGBA) decoded from the embedded PNG.
fn image_rgba() -> Vec<u8> {
    let bytes = include_bytes!("../packaging/icon.png");
    image::load_from_memory(bytes).map(|i| i.to_rgba8().into_raw()).unwrap_or_else(|_| vec![0; 256 * 256 * 4])
}
