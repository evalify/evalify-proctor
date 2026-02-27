use std::sync::Mutex;
use std::process::Command;
use anyhow::Result;

struct SavedGsetting {
    schema: String,
    key: String,
    original: String,
}

const GNOME_SHORTCUTS: &[(&str, &str)] = &[
    ("org.gnome.desktop.wm.keybindings", "switch-applications"),
    ("org.gnome.desktop.wm.keybindings", "switch-applications-backward"),
    ("org.gnome.desktop.wm.keybindings", "switch-windows"),
    ("org.gnome.desktop.wm.keybindings", "switch-windows-backward"),
    ("org.gnome.settings-daemon.plugins.media-keys", "screenshot"),
    ("org.gnome.settings-daemon.plugins.media-keys", "screenshot-clip"),
    ("org.gnome.settings-daemon.plugins.media-keys", "window-screenshot"),
    ("org.gnome.settings-daemon.plugins.media-keys", "window-screenshot-clip"),
    ("org.gnome.settings-daemon.plugins.media-keys", "area-screenshot"),
    ("org.gnome.settings-daemon.plugins.media-keys", "area-screenshot-clip"),
    ("org.gnome.shell.keybindings", "show-screenshot-ui"),
    ("org.gnome.shell.keybindings", "screenshot"),
    ("org.gnome.shell.keybindings", "screenshot-window"),
];

const GNOME_OVERLAY: (&str, &str) = ("org.gnome.mutter", "overlay-key");

pub struct HookManager {
    saved_gsettings: Mutex<Vec<SavedGsetting>>,
    pointer_button_count: Mutex<Option<usize>>,
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            saved_gsettings: Mutex::new(Vec::new()),
            pointer_button_count: Mutex::new(None),
        }
    }

    pub fn install(&self) -> Result<()> {
        self.disable_shortcuts();
        self.disable_right_click();
        Ok(())
    }

    pub fn uninstall(&self) -> Result<()> {
        self.restore_shortcuts();
        self.restore_right_click();
        Ok(())
    }

    fn gsettings_get(schema: &str, key: &str) -> Option<String> {
        Command::new("gsettings")
            .args(["get", schema, key])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    }

    fn gsettings_set(schema: &str, key: &str, value: &str) -> bool {
        Command::new("gsettings")
            .args(["set", schema, key, value])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn disable_shortcuts(&self) {
        let mut saved = self.saved_gsettings.lock().unwrap();

        for &(schema, key) in GNOME_SHORTCUTS {
            if let Some(original) = Self::gsettings_get(schema, key) {
                if Self::gsettings_set(schema, key, "['']") {
                    saved.push(SavedGsetting {
                        schema: schema.to_string(),
                        key: key.to_string(),
                        original,
                    });
                }
            }
        }

        let (schema, key) = GNOME_OVERLAY;
        if let Some(original) = Self::gsettings_get(schema, key) {
            if Self::gsettings_set(schema, key, "''") {
                saved.push(SavedGsetting {
                    schema: schema.to_string(),
                    key: key.to_string(),
                    original,
                });
            }
        }
    }

    fn restore_shortcuts(&self) {
        let saved = self.saved_gsettings.lock().unwrap();
        for s in saved.iter() {
            Self::gsettings_set(&s.schema, &s.key, &s.original);
        }
    }

    fn disable_right_click(&self) {
        let output = match Command::new("xmodmap").args(["-pp"]).output() {
            Ok(o) if o.status.success() => o,
            _ => return,
        };

        let text = String::from_utf8_lossy(&output.stdout);
        let count = text
            .lines()
            .find(|l| l.contains("pointer buttons defined"))
            .and_then(|l| l.split_whitespace().find_map(|w| w.parse::<usize>().ok()));

        if let Some(count) = count {
            *self.pointer_button_count.lock().unwrap() = Some(count);
            let map: String = (1..=count)
                .map(|i| if i == 3 { "0".to_string() } else { i.to_string() })
                .collect::<Vec<_>>()
                .join(" ");
            let _ = Command::new("xmodmap")
                .args(["-e", &format!("pointer = {}", map)])
                .output();
        }
    }

    fn restore_right_click(&self) {
        if let Some(count) = *self.pointer_button_count.lock().unwrap() {
            let map: String = (1..=count)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let _ = Command::new("xmodmap")
                .args(["-e", &format!("pointer = {}", map)])
                .output();
        }
    }
}

impl Drop for HookManager {
    fn drop(&mut self) {
        let _ = self.uninstall();
    }
}
