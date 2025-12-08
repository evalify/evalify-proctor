use std::path::PathBuf;
use anyhow::{Result};

pub mod chrome;
pub mod edge;


#[derive(Debug,Clone)]
pub enum BrowserKind {
    Chrome,
    Chromium,
    Edge,
}

pub struct Browser{
    pub kind : BrowserKind,
    pub path : PathBuf,
}

impl Browser {
    pub fn get_flags(&self, url: &str, proxy_url: &str) -> Vec<String> {
        let mut flags = match self.kind {
            BrowserKind::Chrome | BrowserKind::Chromium => chrome::get_flags(url),
            BrowserKind::Edge => edge::get_flags(url),
        };

        flags.push(format!("--proxy-server={}", proxy_url));

        flags
    }
}

pub fn find() -> Result<Browser> {
    #[cfg(target_os = "windows")]
    return find_windows();

    #[cfg(target_os = "linux")]
    return find_linux();

    #[cfg(target_os = "macos")]
    return find_macos();
}


#[cfg(target_os = "linux")]
pub fn find_linux() -> Result<Browser> {
    use which::which;
    let candidates = [
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
    "microsoft-edge",
    "microsoft-edge-stable",
    ];

    for candidate in candidates {
        if let Ok(path) = which(candidate) {
            let kind = if candidate.contains("edge") {
                BrowserKind::Edge
            } else if candidate.contains("chromium") {
                BrowserKind::Chromium
            } else {
                BrowserKind::Chrome
            };
            return Ok(Browser {
                kind,
                path,
            });
        }
    }
    Err(anyhow::anyhow!("Browser not found"))
}


#[cfg(target_os = "macos")]
pub fn find_macos() -> Result<Browser> {
    use std::path::PathBuf;

    let candidates = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    ];

    for path_str in candidates {
        let path = PathBuf::from(path_str);
        if path.exists() {
             let kind = if path_str.contains("Chromium") {
                BrowserKind::Edge
            } else if path_str.contains("Chromium") {
                BrowserKind::Chromium
            } else {
                BrowserKind::Chrome
            };
            
            return Ok(Browser { kind, path });
        }
    }
    Err(anyhow::anyhow!("Browser not found"))
}


#[cfg(target_os = "windows")]
pub fn find_windows() -> Result<Browser> {
    use winreg::enums::*;
    use winreg::RegKey;
    use which::which;

    // Check PATH
    if let Ok(path) = which("chrome") {
        return Ok(Browser {
            kind: BrowserKind::Chrome,
            path,
        });
    }

    if let Ok(path) = which("chromium") {
        return Ok(Browser {
            kind: BrowserKind::Chromium,
            path,
        });
    }

    if let Ok(path) = which("msedge") {
        return Ok(Browser {
            kind: BrowserKind::Edge,
            path,
        });
    }

    // Check Registery
    let registery_checks = vec![
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe", BrowserKind::Chrome),
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe", BrowserKind::Chrome),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe", BrowserKind::Edge),
    ];

    for (key, path, kind) in registery_checks {
        let key = RegKey::predef(key);
        if let Ok(path) = key.get_value::<String, _>(path) {
            return Ok(Browser {
                kind,
                path: PathBuf::from(path),
            });
        }
    }

    // Check paths
    let path_candidates = [
        (r"C:\Program Files\Google\Chrome\Application\chrome.exe", BrowserKind::Chrome),
        (r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe", BrowserKind::Chrome),
        (r"C:\Users\%USERNAME%\AppData\Local\Google\Chrome\Application\chrome.exe", BrowserKind::Chrome),

        (r"C:\Program Files\Microsoft\Edge\Application\msedge.exe", BrowserKind::Edge),
        (r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe", BrowserKind::Edge),

        (r"C:\Program Files\Chromium\Application\chromium.exe", BrowserKind::Chromium),
        (r"C:\Program Files (x86)\Chromium\Application\chromium.exe", BrowserKind::Chromium),
    ];

    for (path_str, kind) in path_candidates {
        let expanded = shellexpand::full(path_str).unwrap().to_string();
        let path = PathBuf::from(expanded);
        if path.exists() {
            return Ok(Browser {
                kind: kind.clone(),
                path,
            });
        }
    }

    Err(anyhow::anyhow!("Browser not found"))
}

