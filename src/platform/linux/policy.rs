use anyhow::{Result, bail};
use std::fs;
use std::path::Path;

const POLICY_CONTENT: &str = r#"{"DeveloperToolsAvailability": 2}"#;
const POLICY_FILE: &str = "evalify_kiosk.json";

const POLICY_DIRS: &[&str] = &[
    "/etc/opt/chrome/policies/managed",
    "/etc/chromium/policies/managed",
    "/etc/opt/edge/policies/managed",
];

fn check_root() -> Result<()> {
    if unsafe {     libc::geteuid() } != 0 {
        bail!("Must run as root to write browser policies to /etc (try: sudo)");
    }
    Ok(())
}

pub fn disable_devtools() -> Result<()> {
    check_root()?;
    for dir in POLICY_DIRS {
        let path = Path::new(dir);
        fs::create_dir_all(path)?;
        fs::write(path.join(POLICY_FILE), POLICY_CONTENT)?;
    }
    Ok(())
}

pub fn enable_devtools() -> Result<()> {
    for dir in POLICY_DIRS {
        let path = Path::new(dir).join(POLICY_FILE);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
