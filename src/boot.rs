//! Start-up helpers shared by the desktop program and the headless server: data folder and database connection.
use crate::config;
use crate::db::Db;
use crate::dbserver;
use std::path::PathBuf;

pub fn tcp_open(host: &str, port: u16) -> bool {
    use std::net::ToSocketAddrs;
    (host, port).to_socket_addrs().ok().and_then(|mut a| a.next()).map(|a| std::net::TcpStream::connect_timeout(&a, std::time::Duration::from_millis(300)).is_ok()).unwrap_or(false)
}

/// The usual ports of a database on this computer: the RadioMoon service / bundled server, then the system MariaDB / MySQL.
pub const COMMON_PORTS: [u16; 2] = [3307, 3306];

/// Starts the bundled database server when there is one, then connects. A missing bundled server is not an error by itself:
/// the database may be a MariaDB / MySQL service of the system or of another computer. When the configured port does not
/// answer, the usual ports of this computer are tried too; the configuration that worked is returned (it is then saved).
pub fn open_db(cfg: &config::Config, exe_dir: &std::path::Path, data: &std::path::Path) -> anyhow::Result<(dbserver::DbServer, Db, config::Config)> {
    let mut start_err: Option<anyhow::Error> = None;
    let server = if cfg.is_local() {
        match dbserver::DbServer::start(exe_dir, data, cfg.port) {
            Ok(s) => s,
            Err(e) => {
                start_err = Some(e);
                dbserver::DbServer::none()
            }
        }
    } else {
        dbserver::DbServer::none()
    };
    let first = Db::connect(cfg);
    let err = match first {
        Ok(db) => return Ok((server, db, cfg.clone())),
        Err(c) => c,
    };
    if cfg.is_local() {
        for p in COMMON_PORTS {
            if p != cfg.port && tcp_open("127.0.0.1", p) {
                let mut c2 = cfg.clone();
                c2.port = p;
                if let Ok(db) = Db::connect(&c2) {
                    return Ok((server, db, c2));
                }
            }
        }
    }
    match start_err {
        Some(e) => Err(anyhow::anyhow!("{e:#}\n\n{err:#}")),
        None => Err(err),
    }
}

/// Portable mode (a `portable.flag` file next to the program): everything stays in the program folder.
/// Installed mode: data goes to %PROGRAMDATA%\RadioMoon (Windows) or the per-user data folder.
pub fn data_dir(exe_dir: &std::path::Path) -> PathBuf {
    // a service sets the data folder itself
    if let Some(d) = std::env::var_os("RADIOMOON_DATA").filter(|d| !d.is_empty()) {
        let p = PathBuf::from(d);
        let _ = std::fs::create_dir_all(&p);
        return p;
    }
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

