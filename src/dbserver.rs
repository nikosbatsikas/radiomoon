//! Starts the bundled MariaDB/MySQL server that lives next to the program (portable mode).
use anyhow::{bail, Result};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub struct DbServer {
    child: Option<Child>,
    graceful: bool,
}

fn quiet(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
}

fn port_open(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
}

impl DbServer {
    pub fn start(app_dir: &Path, data_dir: &Path, port: u16) -> Result<Self> {
        if port_open(port) {
            // A server is already running on this port (for example your own MySQL).
            return Ok(Self { child: None, graceful: false });
        }
        let root = app_dir.join("mariadb");
        let bin = root.join("bin");
        let win = cfg!(windows);
        let mysqld = bin.join(if win { "mysqld.exe" } else { "mysqld" });
        if !mysqld.exists() {
            bail!(
                "Database server not found:\n{}\n\nCopy the MariaDB files into the 'mariadb' folder next to the program (see README.txt), or start your own MySQL on port {}.",
                mysqld.display(),
                port
            );
        }
        let data = data_dir.join("data");
        if !data.join("mysql").exists() {
            std::fs::create_dir_all(&data)?;
            let install = bin.join(if win { "mysql_install_db.exe" } else { "mysql_install_db" });
            let st = quiet(Command::new(&install).arg(format!("--datadir={}", data.display()))).status()?;
            if !st.success() {
                bail!("Could not initialise the database folder ({}).", data.display());
            }
        }
        let mut child = quiet(
            Command::new(&mysqld)
                .arg("--no-defaults")
                .arg(format!("--basedir={}", root.display()))
                .arg(format!("--datadir={}", data.display()))
                .arg(format!("--port={port}"))
                .arg("--bind-address=127.0.0.1")
                .arg("--character-set-server=utf8mb4")
                .arg("--collation-server=utf8mb4_unicode_ci")
                .arg("--innodb-buffer-pool-size=64M"),
        )
        .spawn()?;

        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(60) {
            if port_open(port) {
                return Ok(Self { child: Some(child), graceful: false });
            }
            if let Ok(Some(st)) = child.try_wait() {
                bail!("The database server stopped right after starting ({st}).");
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        let _ = child.kill();
        bail!("The database server did not start within 60 seconds.");
    }

    /// No bundled server (an external MySQL is used).
    pub fn none() -> Self {
        Self { child: None, graceful: false }
    }

    pub fn owned(&self) -> bool {
        self.child.is_some()
    }

    /// Call after sending SHUTDOWN so Drop waits for a clean exit instead of killing.
    pub fn mark_shutdown(&mut self) {
        self.graceful = true;
    }
}

impl Drop for DbServer {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            if self.graceful {
                let t0 = Instant::now();
                while t0.elapsed() < Duration::from_secs(15) {
                    if let Ok(Some(_)) = c.try_wait() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            let _ = c.kill();
        }
    }
}
