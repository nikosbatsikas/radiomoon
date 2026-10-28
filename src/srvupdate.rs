//! Update of the server to the newest version: the newest release of GitHub is looked up, the package of this system (glibc or Alpine,
//! x64 or arm64) is downloaded, checked (it must start and report the new version), put in place of the running program, with its
//! `lang` and `clients` folders, and the server starts the new program in the same process (no service manager is needed).
//! The database is upgraded by the new program when it starts (it makes a backup first).
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

const REPO: &str = "nikosbatsikas/radiomoon";

/// glibc-x64, alpine-arm64 ... as in the names of the packages.
/// The server runs in a Docker container (the image is replaced to update it, the program is not).
pub fn in_docker() -> bool {
    std::env::var_os("RADIOMOON_DOCKER").is_some() || std::path::Path::new("/.dockerenv").exists()
}

pub fn flavor() -> String {
    let libc = if cfg!(target_env = "musl") { "alpine" } else { "glibc" };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{libc}-{arch}")
}

fn parts(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split('.').map(|x| x.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0)).collect()
}

/// Is `a` a newer version than `b`?
pub fn newer(a: &str, b: &str) -> bool {
    parts(a) > parts(b)
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(8)).timeout_read(Duration::from_secs(60)).build()
}

#[derive(Clone)]
pub struct Latest {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub size: u64,
}

/// The newest release that has a package for this system.
pub fn latest() -> Result<Latest, String> {
    let r = agent()
        .get(&format!("https://api.github.com/repos/{REPO}/releases?per_page=15"))
        .set("User-Agent", "radiomoon-server")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("cannot reach GitHub: {e}"))?;
    let v: Value = serde_json::from_str(&r.into_string().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let want = format!("-linux-{}.tar.gz", flavor());
    let mut best: Option<Latest> = None;
    for rel in v.as_array().ok_or("unexpected answer of GitHub")? {
        if rel["draft"].as_bool() == Some(true) || rel["prerelease"].as_bool() == Some(true) {
            continue;
        }
        let tag = rel["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
        let asset = rel["assets"].as_array().and_then(|a| a.iter().find(|x| x["name"].as_str().map(|n| n.starts_with("RadioMoon-Server-") && n.ends_with(&want)).unwrap_or(false)));
        let Some(asset) = asset else { continue };
        let l = Latest { version: tag, notes: rel["body"].as_str().unwrap_or("").to_string(), url: asset["browser_download_url"].as_str().unwrap_or("").to_string(), size: asset["size"].as_u64().unwrap_or(0) };
        if l.url.is_empty() {
            continue;
        }
        if best.as_ref().map(|b| newer(&l.version, &b.version)).unwrap_or(true) {
            best = Some(l);
        }
    }
    best.ok_or_else(|| format!("no release has a package for this server ({})", flavor()))
}

/// The answer of the last look at GitHub (made in a thread: the web page asks again every second until it is there).
static CHECK: Mutex<(Option<Result<Latest, String>>, bool)> = Mutex::new((None, false));

pub fn check_async() {
    {
        let Ok(mut g) = CHECK.lock() else { return };
        if g.1 {
            return;
        }
        g.1 = true;
    }
    std::thread::spawn(|| {
        let r = latest();
        if let Ok(mut g) = CHECK.lock() {
            g.0 = Some(r);
            g.1 = false;
        }
    });
}

/// What the web page and the client show.
pub fn state(current: &str) -> Value {
    let g = CHECK.lock().map(|g| (g.0.clone(), g.1)).unwrap_or((None, false));
    let (latest, error) = match &g.0 {
        Some(Ok(l)) => (Some(l.clone()), None),
        Some(Err(e)) => (None, Some(e.clone())),
        None => (None, None),
    };
    json!({
        "current": current,
        "flavor": flavor(),
        "checking": g.1,
        "checked": g.0.is_some(),
        "error": error,
        "latest": latest.as_ref().map(|l| json!({ "version": l.version, "notes": l.notes, "size": l.size })),
        "newer": !in_docker() && latest.as_ref().map(|l| newer(&l.version, current)).unwrap_or(false),
        "docker": in_docker(),
        "update": status(),
    })
}

/// Installs the version that was found by the last look.
pub fn install(current: &str) -> Result<(), String> {
    if in_docker() {
        return Err("This server runs in Docker: update it by replacing the image (docker compose pull, then docker compose up -d).".into());
    }
    let l = CHECK.lock().map_err(|_| "busy".to_string())?.0.clone();
    match l {
        Some(Ok(l)) if newer(&l.version, current) => start(l),
        Some(Ok(_)) => Err("This server is already the newest version".into()),
        _ => Err("Look for the new version first".into()),
    }
}

static STATE: Mutex<(String, Option<String>, bool)> = Mutex::new((String::new(), None, false));

/// {phase, error, running}
pub fn status() -> Value {
    let g = STATE.lock().map(|g| g.clone()).unwrap_or_default();
    json!({ "phase": g.0, "error": g.1, "running": g.2 })
}

fn say(m: &str) {
    if let Ok(mut g) = STATE.lock() {
        g.0 = m.to_string();
    }
}

fn fail(m: String) {
    if let Ok(mut g) = STATE.lock() {
        g.1 = Some(m);
        g.2 = false;
    }
}

/// Downloads and installs `l` and restarts the program. Runs in its own thread; `status()` tells how far it is.
pub fn start(l: Latest) -> Result<(), String> {
    {
        let mut g = STATE.lock().map_err(|_| "busy".to_string())?;
        if g.2 {
            return Err("The update is already running".into());
        }
        *g = ("Starting...".into(), None, true);
    }
    // the program folder must be writable for the user of the server
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("no program folder")?.to_path_buf();
    let probe = dir.join(".update-test");
    if std::fs::write(&probe, b"x").is_err() {
        if let Ok(mut g) = STATE.lock() {
            g.2 = false;
        }
        return Err("The program folder is not writable for the server. Run  ./install.sh upgrade  once as root from the newest package (it allows later updates from the web page), or update by hand.".into());
    }
    let _ = std::fs::remove_file(&probe);
    std::thread::spawn(move || {
        if let Err(e) = run(&l, &exe, &dir) {
            fail(e);
        }
    });
    Ok(())
}

fn run(l: &Latest, exe: &Path, dir: &Path) -> Result<(), String> {
    use std::process::Command;
    let tmp = dir.join(".update");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| format!("cannot create {}: {e}", tmp.display()))?;
    say(&format!("Downloading {} ({} MB)...", l.version, l.size / 1_048_576));
    let file = tmp.join("pack.tar.gz");
    let r = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(120))
        .build()
        .get(&l.url)
        .set("User-Agent", "radiomoon-server")
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    let mut f = std::fs::File::create(&file).map_err(|e| e.to_string())?;
    std::io::copy(&mut r.into_reader(), &mut f).map_err(|e| format!("download failed: {e}"))?;
    drop(f);
    say("Unpacking...");
    let out = Command::new("tar").arg("-xzf").arg(&file).arg("-C").arg(&tmp).output().map_err(|e| format!("cannot unpack (tar): {e}"))?;
    if !out.status.success() {
        return Err(format!("cannot unpack: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let pack = tmp.join("RadioMoon-Server");
    let new_exe = pack.join("radiomoon-server");
    if !new_exe.is_file() {
        return Err("the package has no server program".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&new_exe, std::fs::Permissions::from_mode(0o755));
    }
    say("Checking the new program...");
    let o = Command::new(&new_exe).arg("--version").output().map_err(|e| format!("the new program does not start on this system: {e}"))?;
    let said = String::from_utf8_lossy(&o.stdout).to_string();
    if !o.status.success() || !said.contains(&l.version) {
        return Err(format!("the new program did not report version {} (it said: {})", l.version, said.trim()));
    }
    say("Installing...");
    let old = dir.join("radiomoon-server.old");
    let _ = std::fs::copy(exe, &old);
    let staged = dir.join("radiomoon-server.new");
    std::fs::copy(&new_exe, &staged).map_err(|e| format!("cannot write the program: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755));
    }
    std::fs::rename(&staged, exe).map_err(|e| format!("cannot replace the program: {e}"))?;
    for sub in ["lang", "clients"] {
        let from = pack.join(sub);
        if from.is_dir() {
            if sub == "clients" {
                let _ = std::fs::remove_dir_all(dir.join("clients")); // the clients of the old version go
            }
            crate::remote::copy_dir(&from, &dir.join(sub));
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    say("Restarting the server...");
    std::thread::sleep(Duration::from_millis(1500)); // the answer of the web page goes out first
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let e = Command::new(exe).args(std::env::args().skip(1)).exec();
        return Err(format!("the new program was installed but could not be started ({e}): restart the service"));
    }
    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert!(newer("0.14.20", "0.14.19"));
        assert!(newer("v0.15.0", "0.14.99"));
        assert!(!newer("0.14.19", "0.14.19"));
        assert!(!newer("0.9.9", "0.14.0"));
    }
}
