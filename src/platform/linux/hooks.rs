use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};
use anyhow::{Context, Result};

const LXQT_SHORTCUT_TOKENS: &[&str] = &[
    "Print",
    "Shift%2BPrint",
    "Control%2BPrint",
    "Shift%2BControl%2BPrint",
    "Alt%2BPrint",
    "Meta%2BPrint",
    "Super_L",
];

const LXQT_SCREENSHOT_COMMENTS: &[&str] = &[
    "screenshot",
    "screen shot",
    "screen-shot",
    "capture",
    "snip",
];

const LXQT_SCREENSHOT_EXEC: &[&str] = &[
    "screengrab",
    "flameshot",
    "gnome-screenshot",
    "spectacle",
    "xfce4-screenshooter",
    "grim",
    "slurp",
];

const OPENBOX_KEYS: &[&str] = &[
    "A-Tab",
    "A-S-Tab",
    "A-ISO_Left_Tab",
    "C-t",
    "C-w",
    "C-n",
    "C-S-t",
    "C-S-n",
    "C-l",
    "C-S-i",
    "C-S-j",
    "F11",
    "F12",
];

struct SavedFile {
    path: PathBuf,
    original: String,
}

pub struct HookManager {
    saved_files: Mutex<Vec<SavedFile>>,
    pointer_button_count: Mutex<Option<usize>>,
    print_key_mapping: Mutex<Option<String>>,
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            saved_files: Mutex::new(Vec::new()),
            pointer_button_count: Mutex::new(None),
            print_key_mapping: Mutex::new(None),
        }
    }

    pub fn install(&self) -> Result<()> {
        if !Self::is_x11() {
            let actual = env::var("XDG_SESSION_TYPE").unwrap_or_default();
            anyhow::bail!("Only X11 sessions are supported (detected: {actual})");
        }
        self.disable_lxqt_shortcuts()?;
        self.neutralize_openbox_keybinds()?;
        self.disable_print_key();
        self.disable_right_click();
        Ok(())
    }

    pub fn uninstall(&self) -> Result<()> {
        self.restore_saved_files();
        self.restore_print_key();
        Self::reload_lxqt_globalkeys();
        Self::reload_openbox();
        self.restore_right_click();
        Ok(())
    }

    fn is_x11() -> bool {
        env::var("XDG_SESSION_TYPE")
            .unwrap_or_default()
            .eq_ignore_ascii_case("x11")
    }

    // ── LXQt global shortcuts ──────────────────────────────────────────

    fn lxqt_config_path() -> Option<PathBuf> {
        env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(".config/lxqt/globalkeyshortcuts.conf"))
    }

    fn disable_lxqt_shortcuts(&self) -> Result<()> {
        let path = match Self::lxqt_config_path() {
            Some(p) if p.exists() => p,
            _ => return Ok(()),
        };

        let original =
            fs::read_to_string(&path).with_context(|| format!("Reading {}", path.display()))?;

        self.save_file_backup(&path, &original);

        let patched = Self::patch_lxqt_config(&original);
        if patched != original {
            fs::write(&path, &patched)
                .with_context(|| format!("Writing {}", path.display()))?;
            Self::reload_lxqt_globalkeys();
        }

        Ok(())
    }

    fn patch_lxqt_config(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut section_name = String::new();
        let mut section_lines: Vec<String> = Vec::new();
        let mut in_section = false;

        for line in input.lines() {
            if let Some(name) = Self::parse_ini_section(line) {
                if in_section {
                    Self::write_section(&mut out, &section_name, &mut section_lines);
                }
                section_name = name;
                section_lines.clear();
                in_section = true;
            } else if in_section {
                section_lines.push(line.to_string());
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }

        if in_section {
            Self::write_section(&mut out, &section_name, &mut section_lines);
        }

        if !input.ends_with('\n') && out.ends_with('\n') {
            out.pop();
        }

        out
    }

    fn write_section(out: &mut String, name: &str, lines: &mut Vec<String>) {
        out.push_str(&format!("[{name}]\n"));

        if !Self::should_disable_section(name, lines) {
            for l in lines.drain(..) {
                out.push_str(&l);
                out.push('\n');
            }
            return;
        }

        let mut has_enabled = false;
        for l in lines.drain(..) {
            let trimmed = l.trim();
            if trimmed.starts_with("Enabled=") {
                out.push_str("Enabled=false\n");
                has_enabled = true;
            } else if trimmed.starts_with("Exec=")
                || trimmed.starts_with("path=")
                || trimmed.starts_with("DBus=")
                || trimmed.starts_with("Command=")
            {
                let key = trimmed.split('=').next().unwrap_or("");
                out.push_str(&format!("{key}=\n"));
            } else {
                out.push_str(&l);
                out.push('\n');
            }
        }

        if !has_enabled {
            out.push_str("Enabled=false\n");
        }
    }

    fn parse_ini_section(line: &str) -> Option<String> {
        line.trim()
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    fn should_disable_section(name: &str, lines: &[String]) -> bool {
        let token = name.split('.').next().unwrap_or(name);
        if LXQT_SHORTCUT_TOKENS
            .iter()
            .any(|t| t.eq_ignore_ascii_case(token))
        {
            return true;
        }

        for line in lines {
            let low = line.to_lowercase();
            if low.starts_with("comment=")
                && LXQT_SCREENSHOT_COMMENTS.iter().any(|k| low.contains(k))
            {
                return true;
            }
            if low.starts_with("exec=")
                && LXQT_SCREENSHOT_EXEC.iter().any(|k| low.contains(k))
            {
                return true;
            }
        }

        false
    }

    fn reload_lxqt_globalkeys() {
        let _ = Command::new("pkill")
            .args(["-HUP", "lxqt-globalkeysd"])
            .output();
    }

    // ── Openbox keybinds ───────────────────────────────────────────────

    fn openbox_rc_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(home) = env::var_os("HOME") {
            let base = PathBuf::from(home).join(".config/openbox");
            paths.push(base.join("lxqt-rc.xml"));
            paths.push(base.join("rc.xml"));
        }
        paths
    }

    fn neutralize_openbox_keybinds(&self) -> Result<()> {
        for path in Self::openbox_rc_paths() {
            if !path.exists() {
                continue;
            }

            let original = fs::read_to_string(&path)
                .with_context(|| format!("Reading {}", path.display()))?;

            self.save_file_backup(&path, &original);

            let patched = Self::patch_openbox_xml(&original);
            if patched != original {
                fs::write(&path, &patched)
                    .with_context(|| format!("Writing {}", path.display()))?;
                Self::reload_openbox();
            }

            return Ok(());
        }
        Ok(())
    }

    fn patch_openbox_xml(xml: &str) -> String {
        let mut result = xml.to_string();

        for key in OPENBOX_KEYS {
            result = Self::replace_keybind_with_noop(&result, key);
        }

        if !OPENBOX_KEYS
            .iter()
            .any(|k| result.contains(&format!("key=\"{k}\"")))
        {
            result = Self::insert_noop_keybinds(&result);
        }

        result
    }

    fn replace_keybind_with_noop(xml: &str, key: &str) -> String {
        let open_tag = format!("<keybind key=\"{key}\">");
        let Some(start) = xml.find(&open_tag) else {
            return xml.to_string();
        };

        // Depth-aware search: skip nested <keybind>...</keybind> pairs
        let rest = &xml[start + open_tag.len()..];
        let mut depth = 1i32;
        let mut search_pos = 0;
        while depth > 0 && search_pos < rest.len() {
            let next_open = rest[search_pos..].find("<keybind ").map(|i| i + search_pos);
            let next_close = rest[search_pos..].find("</keybind>").map(|i| i + search_pos);

            match (next_open, next_close) {
                (_, Some(c)) if next_open.map_or(true, |o| c < o) => {
                    depth -= 1;
                    search_pos = c + "</keybind>".len();
                }
                (Some(o), _) => {
                    depth += 1;
                    search_pos = o + "<keybind ".len();
                }
                _ => break,
            }
        }

        if depth != 0 {
            return xml.to_string();
        }

        let end = start + open_tag.len() + search_pos;

        format!(
            "{}{open_tag}\n    <action name=\"Execute\"><command>true</command></action>\n  </keybind>{}",
            &xml[..start],
            &xml[end..]
        )
    }

    fn insert_noop_keybinds(xml: &str) -> String {
        let Some(pos) = xml.find("<keyboard>") else {
            return xml.to_string();
        };
        let after = pos + "<keyboard>".len();

        let keybinds: String = OPENBOX_KEYS
            .iter()
            .map(|k| {
                format!(
                    "\n  <keybind key=\"{k}\">\n    <action name=\"Execute\"><command>true</command></action>\n  </keybind>"
                )
            })
            .collect();

        format!("{}{keybinds}{}", &xml[..after], &xml[after..])
    }

    fn reload_openbox() {
        let _ = Command::new("openbox").arg("--reconfigure").output();
    }

    // ── X11 Print key ──────────────────────────────────────────────────

    fn disable_print_key(&self) {
        let output = match Command::new("xmodmap").args(["-pke"]).output() {
            Ok(o) if o.status.success() => o,
            _ => return,
        };

        let text = String::from_utf8_lossy(&output.stdout);

        // Find the keycode that maps to Print keysym (typically 107, but not guaranteed)
        let print_line = text.lines().find(|l| {
            l.split('=')
                .nth(1)
                .map(|rhs| rhs.split_whitespace().any(|sym| sym == "Print"))
                .unwrap_or(false)
        });

        let Some(line) = print_line else { return };
        *self.print_key_mapping.lock().unwrap() = Some(line.to_string());

        if let Some(keycode) = line.split_whitespace().nth(1) {
            let _ = Command::new("xmodmap")
                .args(["-e", &format!("keycode {keycode} =")])
                .output();
        }
    }

    fn restore_print_key(&self) {
        if let Some(line) = self.print_key_mapping.lock().unwrap().as_ref() {
            let _ = Command::new("xmodmap").args(["-e", line]).output();
        }
    }

    // ── Right click ────────────────────────────────────────────────────

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
            // button 3 = right click
            let map: String = (1..=count)
                .map(|i| if i == 3 { "0".to_string() } else { i.to_string() })
                .collect::<Vec<_>>()
                .join(" ");
            let _ = Command::new("xmodmap")
                .args(["-e", &format!("pointer = {map}")])
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
                .args(["-e", &format!("pointer = {map}")])
                .output();
        }
    }

    // ── File backup/restore ────────────────────────────────────────────

    fn save_file_backup(&self, path: &Path, content: &str) {
        let mut saved = self.saved_files.lock().unwrap();
        if !saved.iter().any(|f| f.path == path) {
            saved.push(SavedFile {
                path: path.to_path_buf(),
                original: content.to_string(),
            });
        }
    }

    fn restore_saved_files(&self) {
        let saved = self.saved_files.lock().unwrap();
        for f in saved.iter() {
            let _ = fs::write(&f.path, &f.original);
        }
    }
}

impl Drop for HookManager {
    fn drop(&mut self) {
        let _ = self.uninstall();
    }
}
