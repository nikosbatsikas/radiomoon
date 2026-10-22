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
mod player;
mod stream;
mod studio;
mod waveform;

use app::App;
use db::Db;
use eframe::egui;
use std::path::PathBuf;

const WHITE: egui::Color32 = egui::Color32::WHITE;

struct ErrorApp(String);

impl eframe::App for ErrorApp {
    fn update(&mut self, ctx: &egui::Context, _f: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(egui::RichText::new("RadioMoon could not start").color(WHITE));
            ui.add_space(8.0);
            ui.label(&self.0);
        });
    }
}

/// Portable mode (a `portable.flag` file next to the program): everything stays in the program folder.
/// Installed mode: data goes to the per-user application data folder.
fn data_dir(exe_dir: &std::path::Path) -> PathBuf {
    if exe_dir.join("portable.flag").exists() {
        return exe_dir.to_path_buf();
    }
    let base: Option<PathBuf> = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
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
            .with_min_inner_size([1250.0, 700.0])
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
        Err(e) => eframe::run_native("RadioMoon", options, Box::new(move |_cc| Ok(Box::new(ErrorApp(format!("{e:#}")))))),
    }
}

/// Window icon (256x256 RGBA) decoded from the embedded PNG.
fn image_rgba() -> Vec<u8> {
    let bytes = include_bytes!("../packaging/icon.png");
    image::load_from_memory(bytes).map(|i| i.to_rgba8().into_raw()).unwrap_or_else(|_| vec![0; 256 * 256 * 4])
}
