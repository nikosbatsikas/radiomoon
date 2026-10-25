//! radiomoon-server: RadioMoon without a window, for headless Linux servers. The studio is used from a web browser.
#![allow(dead_code)]

#[path = "../app.rs"]
mod app;
#[path = "../backup.rs"]
mod backup;
#[path = "../boot.rs"]
mod boot;
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
#[path = "../schema.rs"]
mod schema;
#[path = "../stream.rs"]
mod stream;
#[path = "../studio.rs"]
mod studio;
#[path = "../waveform.rs"]
mod waveform;
#[path = "../web.rs"]
mod web;

/// There are no file dialogs on a server: the paths are typed in the web studio or in the configuration.
mod rfd {
    use std::path::PathBuf;
    pub struct FileDialog;
    impl FileDialog {
        pub fn new() -> Self {
            FileDialog
        }
        pub fn add_filter(self, _name: &str, _ext: &[&str]) -> Self {
            self
        }
        pub fn set_directory<P: AsRef<std::path::Path>>(self, _p: P) -> Self {
            self
        }
        pub fn pick_file(self) -> Option<PathBuf> {
            None
        }
        pub fn pick_folder(self) -> Option<PathBuf> {
            None
        }
        pub fn pick_files(self) -> Option<Vec<PathBuf>> {
            None
        }
    }
}

use std::path::PathBuf;
use std::time::Duration;

fn usage() {
    println!(
        "radiomoon-server {}\n\n\
         Usage: radiomoon-server [options]\n\n\
         --bind ADDR:PORT      address of the web studio (default 0.0.0.0:8080, or RADIOMOON_BIND)\n\
         --data DIR            data folder (default RADIOMOON_DATA or the per-user data folder)\n\
         --db-host HOST        database host (default 127.0.0.1)\n\
         --db-port PORT        database port\n\
         --db-user USER        database user\n\
         --db-password PASS    database password\n\
         --db-name NAME        database name (default radiomoon)\n\
         --help                this text\n\n\
         The web studio login is stored in webserver.json inside the data folder; it is created at the first start.",
        app::VERSION
    );
}

fn main() {
    let mut bind = std::env::var("RADIOMOON_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let mut db_over: Vec<(String, String)> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let (k, inline) = match a.split_once('=') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (a.clone(), None),
        };
        match k.as_str() {
            "--help" | "-h" => {
                usage();
                return;
            }
            "--version" | "-V" => {
                println!("radiomoon-server {}", app::VERSION);
                return;
            }
            "--bind" | "--data" | "--db-host" | "--db-port" | "--db-user" | "--db-password" | "--db-name" => {
                let v = inline.or_else(|| args.next()).unwrap_or_default();
                if k == "--bind" {
                    bind = v;
                } else if k == "--data" {
                    std::env::set_var("RADIOMOON_DATA", v);
                } else {
                    db_over.push((k, v));
                }
            }
            _ => {
                eprintln!("unknown option {a}");
                usage();
                std::process::exit(2);
            }
        }
    }

    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|x| x.to_path_buf())).unwrap_or_else(|| PathBuf::from("."));
    let data = boot::data_dir(&exe_dir);
    let mut cfg = config::load(&data);
    let before = (cfg.host.clone(), cfg.port, cfg.user.clone(), cfg.password.clone(), cfg.database.clone());
    for (k, v) in &db_over {
        match k.as_str() {
            "--db-host" => cfg.host = v.clone(),
            "--db-port" => cfg.port = v.parse().unwrap_or(cfg.port),
            "--db-user" => cfg.user = v.clone(),
            "--db-password" => cfg.password = v.clone(),
            "--db-name" => cfg.database = v.clone(),
            _ => {}
        }
    }
    let changed = before != (cfg.host.clone(), cfg.port, cfg.user.clone(), cfg.password.clone(), cfg.database.clone());

    player::HEADLESS.store(true, std::sync::atomic::Ordering::SeqCst);
    println!("RadioMoon server {} - data folder {}", app::VERSION, data.display());

    let (server, db, used) = match boot::open_db(&cfg, &exe_dir, &data) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Cannot connect to the database: {e:#}");
            eprintln!("Give the connection with --db-host / --db-port / --db-user / --db-password / --db-name (it is remembered).");
            std::process::exit(1);
        }
    };
    if changed || used.port != cfg.port {
        config::save(&data, &used);
    }
    let msg = match db.ensure_schema(app::VERSION, &data.join("backups")) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Database error: {e:#}");
            std::process::exit(1);
        }
    };

    let mut app = app::App::new(server, db, used, data.clone(), exe_dir, msg);
    app.user = Some(model::User { admin: true, ..Default::default() });

    let (user, pass, fresh) = web::credentials(&data);
    let creds = (user.clone(), pass.clone());
    let rx = match web::spawn(&bind) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
    };
    println!("Web studio: http://{bind}/  (user: {user})");
    if fresh {
        println!("First start: the password is {pass}  (kept in {})", data.join("webserver.json").display());
    }

    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(req) => {
                let resp = web::handle(&mut app, &req, &creds);
                let _ = req.reply.send(resp);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
        app.playout_tick();
    }
}
