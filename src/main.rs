#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod backup;
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
mod mpx;
mod mpxdsp;
mod mpxio;
mod mpxscope;
mod player;
mod schema;
mod stream;
mod studio;
mod waveform;

use app::App;
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
            let _server = if self.cfg.is_local() { dbserver::DbServer::start(&self.exe_dir, &self.data, self.cfg.port).ok() } else { None };
            let _db = Db::connect(&self.cfg)?;
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
            ui.label(egui::RichText::new(tr("For the database server of the installer (Windows service RadioMoonDB) use address 127.0.0.1, port 3307, user root, empty password.")).small().color(egui::Color32::GRAY));
        });
    }
}

/// Portable mode (a `portable.flag` file next to the program): everything stays in the program folder.
/// Installed mode: data goes to %PROGRAMDATA%\RadioMoon (Windows) or the per-user data folder.
fn data_dir(exe_dir: &std::path::Path) -> PathBuf {
    if exe_dir.join("portable.flag").exists() {
        return exe_dir.to_path_buf();
    }
    let base: Option<PathBuf> = if cfg!(windows) {
        // %PROGRAMDATA%\RadioMoon; an older per-user folder (%LOCALAPPDATA%\RadioMoon) is still used if it exists and the new one does not
        let pd = std::env::var_os("PROGRAMDATA").map(PathBuf::from);
        let old = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        match (pd, old) {
            (Some(p), Some(o)) if !p.join("RadioMoon").exists() && o.join("RadioMoon").exists() => Some(o),
            (Some(p), _) => Some(p),
            (None, o) => o,
        }
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    let p = base.map(|b| b.join("RadioMoon")).unwrap_or_else(|| exe_dir.to_path_buf());
    let _ = std::fs::create_dir_all(&p);
    p
}

fn main() -> eframe::Result<()> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|x| x.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let data = data_dir(&exe_dir);
    let cfg = config::load(&data);

    let startup = (|| -> anyhow::Result<(dbserver::DbServer, Db, String)> {
        let server = if cfg.is_local() { dbserver::DbServer::start(&exe_dir, &data, cfg.port)? } else { dbserver::DbServer::none() };
        let db = Db::connect(&cfg)?;
        let msg = db.ensure_schema(app::VERSION, &data.join("backups"))?;
        Ok((server, db, msg))
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
        Ok((server, db, msg)) => eframe::run_native(
            "RadioMoon",
            options,
            Box::new(move |cc| {
                kit::apply_style(&cc.egui_ctx);
                Ok(Box::new(App::new(server, db, cfg, data, exe_dir, msg)))
            }),
        ),
        Err(e) => {
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
