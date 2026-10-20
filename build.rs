use std::process::Command;

fn main() {
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "local".to_string());
    println!("cargo:rustc-env=RM_GIT={hash}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("packaging/icon.ico");
        res.set("ProductName", "RadioMoon");
        res.set("FileDescription", "RadioMoon radio automation");
        let _ = res.compile();
    }
}
