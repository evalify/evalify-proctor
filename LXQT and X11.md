use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

use anyhow::{Context, Result};

struct SavedGsetting {
    schema: String,
    key: String,
    original: String,
}

struct SavedFile {
    path: PathBuf,
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

/// Targets to disable for LXQt globalkeyshortcuts.conf.
///
/// LXQt stores shortcuts as INI sections like:
/// [Print.29]
/// Comment=Screenshot
/// Enabled=true
/// Exec=screengrab, -f
///
/// Also combos are URL-encoded, e.g. Shift%2BPrint.
/// We'll disable anything whose section "shortcut token" matches Print-ish
/// OR whose Exec/Comment indicates screenshot tooling.
const LXQT_SHORTCUT_TOKENS_TO_DISABLE: &[&str] = &[
    "Print",
    "Shift%2BPrint",
    "Control%2BPrint",
    "Shift%2BControl%2BPrint",
    "Alt%2BPrint",
    "Meta%2BPrint",
    "Super_L", // mimic GNOME overlay-key disable (optional but aligned with your intent)
];

const LXQT_COMMENT_KEYWORDS: &[&str] = &[
    "screenshot",
    "screen shot",
    "screen-shot",
    "capture",
    "snip",
];

const LXQT_EXEC_KEYWORDS: &[&str] = &[
    "screengrab",        // common on LXQt/Lubuntu
    "flameshot",
    "gnome-screenshot",
    "spectacle",
    "xfce4-screenshooter",
    "grim",              // (Wayland) harmless if present
    "slurp",             // (Wayland)
];

/// Openbox keybinds to neutralize (Alt-Tab family).
/// Openbox uses key="A-Tab" and key="A-S-Tab" in rc.xml keyboard section.
/// (A=Alt, S=Shift)
const OPENBOX_KEYS_TO_NEUTRALIZE: &[&str] = &["A-Tab", "A-S-Tab", "A-ISO_Left_Tab"];

pub struct HookManager {
    saved_gsettings: Mutex<Vec<SavedGsetting>>,
    saved_files: Mutex<Vec<SavedFile>>,
    pointer_button_count: Mutex<Option<usize>>,
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            saved_gsettings: Mutex::new(Vec::new()),
            saved_files: Mutex::new(Vec::new()),
            pointer_button_count: Mutex::new(None),
        }
    }

    pub fn install(&self) -> Result<()> {
        self.disable_shortcuts()?;
        self.disable_right_click();
        Ok(())
    }

    pub fn uninstall(&self) -> Result<()> {
        self.restore_shortcuts()?;
        self.restore_right_click();
        Ok(())
    }

    // --------------------------
    // Desktop detection
    // --------------------------

    fn xdg_current_desktop() -> String {
        env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()
    }

    fn is_lxqt() -> bool {
        let v = Self::xdg_current_desktop().to_lowercase();
        v.contains("lxqt")
    }

    fn is_gnome() -> bool {
        let v = Self::xdg_current_desktop().to_lowercase();
        // covers "GNOME", "ubuntu:GNOME", etc.
        v.contains("gnome")
    }

    fn is_x11_session() -> bool {
        // Your requirement says X11; keep a guard to avoid breaking Wayland.
        let t = env::var("XDG_SESSION_TYPE").unwrap_or_default().to_lowercase();
        t == "x11"
    }

    // --------------------------
    // GNOME gsettings helpers
    // --------------------------

    fn gsettings_schema_exists(schema: &str) -> bool {
        let out = Command::new("gsettings")
            .args(["list-schemas"])
            .output();

        match out {
            Ok(o) if o.status.success() => {
                let s = String::from_utf8_lossy(&o.stdout);
                s.lines().any(|l| l.trim() == schema)
            }
            _ => false,
        }
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

    // --------------------------
    // Public shortcut control
    // --------------------------

    fn disable_shortcuts(&self) -> Result<()> {
        // X11 is where your xmodmap / openbox edits make sense. If not X11, do nothing.
        if !Self::is_x11_session() {
            return Ok(());
        }

        if Self::is_gnome() {
            self.disable_shortcuts_gnome();
        } else if Self::is_lxqt() {
            // LXQt global shortcuts + WM (Openbox) keybind neutralization
            self.disable_shortcuts_lxqt()?;
            self.disable_shortcuts_openbox_if_present()?;
        } else {
            // Unknown desktop: do safest partial lockdown:
            // - disable Print key at X11 level (prevents most screenshot shortcuts)
            // - neutralize openbox if present
            self.disable_print_key_x11()?;
            self.disable_shortcuts_openbox_if_present()?;
        }

        Ok(())
    }

    fn restore_shortcuts(&self) -> Result<()> {
        // restore gsettings
        {
            let saved = self.saved_gsettings.lock().unwrap();
            for s in saved.iter() {
                let _ = Self::gsettings_set(&s.schema, &s.key, &s.original);
            }
        }

        // restore files
        {
            let saved = self.saved_files.lock().unwrap();
            for f in saved.iter() {
                // best-effort restore
                let _ = fs::write(&f.path, &f.original);
            }
        }

        // try to re-enable Print key if we disabled it via xmodmap
        // (We restore by reloading default keymap, best-effort)
        let _ = self.restore_print_key_x11();

        // reload lxqt-globalkeysd if present
        let _ = Self::reload_lxqt_globalkeys();

        // reload openbox if present
        let _ = Self::reload_openbox();

        Ok(())
    }

    // --------------------------
    // GNOME backend (your original logic, but schema-safe)
    // --------------------------

    fn disable_shortcuts_gnome(&self) {
        let mut saved = self.saved_gsettings.lock().unwrap();

        for &(schema, key) in GNOME_SHORTCUTS {
            if !Self::gsettings_schema_exists(schema) {
                continue;
            }
            if let Some(original) = Self::gsettings_get(schema, key) {
                // GNOME expects array for most bindings
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
        if Self::gsettings_schema_exists(schema) {
            if let Some(original) = Self::gsettings_get(schema, key) {
                // overlay-key expects string
                if Self::gsettings_set(schema, key, "''") {
                    saved.push(SavedGsetting {
                        schema: schema.to_string(),
                        key: key.to_string(),
                        original,
                    });
                }
            }
        }
    }

    // --------------------------
    // LXQt backend (edit ~/.config/lxqt/globalkeyshortcuts.conf)
    // --------------------------

    fn lxqt_globalkeys_path() -> Option<PathBuf> {
        let home = env::var_os("HOME")?;
        Some(PathBuf::from(home).join(".config/lxqt/globalkeyshortcuts.conf"))
    }

    fn disable_shortcuts_lxqt(&self) -> Result<()> {
        let Some(path) = Self::lxqt_globalkeys_path() else {
            // no HOME -> nothing we can do
            return Ok(());
        };

        if !path.exists() {
            // LXQt not configured / not installed
            // fallback: disable Print key at X11 level
            self.disable_print_key_x11()?;
            return Ok(());
        }

        let original = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        // save file for restore only once
        self.save_file_once(&path, &original)?;

        let updated = Self::lxqt_disable_in_ini(&original);

        if updated != original {
            fs::write(&path, updated).with_context(|| format!("Failed to write {}", path.display()))?;
            Self::reload_lxqt_globalkeys();
        }

        // Additionally disable Print key at X11 level to cover WM/app grabs.
        // This is harmless even if LXQt keys are already removed.
        self.disable_print_key_x11()?;

        Ok(())
    }

    fn lxqt_disable_in_ini(input: &str) -> String {
        // Very small INI-ish parser:
        // - sections: [....]
        // - within a section, if it matches screenshot-related criteria,
        //   set Enabled=false and clear Exec/path/DBus.
        //
        // We keep formatting reasonably intact.
        let mut out = String::with_capacity(input.len());
        let mut cur_section: Option<String> = None;
        let mut section_lines: Vec<String> = Vec::new();

        let flush_section = |out: &mut String, section: &Option<String>, lines: &mut Vec<String>| {
            if section.is_none() {
                // preamble lines
                for l in lines.drain(..) {
                    out.push_str(&l);
                    out.push('\n');
                }
                return;
            }

            let sec_name = section.as_ref().unwrap();
            let should_disable = Self::lxqt_should_disable_section(sec_name, lines);

            out.push('[');
            out.push_str(sec_name);
            out.push_str("]\n");

            if should_disable {
                // rewrite keys inside this section: disable + clear action fields
                let mut saw_enabled = false;

                for mut l in lines.drain(..) {
                    let trimmed = l.trim();

                    if trimmed.starts_with("Enabled=") {
                        l = "Enabled=false".to_string();
                        saw_enabled = true;
                    }

                    if trimmed.starts_with("Exec=")
                        || trimmed.starts_with("path=")
                        || trimmed.starts_with("DBus=")
                        || trimmed.starts_with("Command=")
                    {
                        // clear action
                        let key = trimmed.split('=').next().unwrap_or("").trim();
                        l = format!("{key}=");
                    }

                    out.push_str(&l);
                    out.push('\n');
                }

                if !saw_enabled {
                    out.push_str("Enabled=false\n");
                }
            } else {
                for l in lines.drain(..) {
                    out.push_str(&l);
                    out.push('\n');
                }
            }
        };

        // We must keep section header lines; we’ll store only name without brackets.
        let mut preamble: Option<String> = None;
        let mut preamble_lines: Vec<String> = Vec::new();
        let mut in_any_section = false;

        for raw in input.lines() {
            let line = raw.to_string();

            if let Some(name) = Self::parse_ini_section_header(raw) {
                // flush previous
                if in_any_section {
                    flush_section(&mut out, &cur_section, &mut section_lines);
                } else {
                    // flush preamble (no section yet)
                    preamble = Some(String::new());
                    flush_section(&mut out, &preamble, &mut preamble_lines);
                }

                cur_section = Some(name);
                section_lines.clear();
                in_any_section = true;
                continue;
            }

            if in_any_section {
                section_lines.push(line);
            } else {
                preamble_lines.push(line);
            }
        }

        // final flush
        if in_any_section {
            flush_section(&mut out, &cur_section, &mut section_lines);
        } else {
            preamble = Some(String::new());
            flush_section(&mut out, &preamble, &mut preamble_lines);
        }

        // preserve trailing newline if input had it
        if input.ends_with('\n') {
            out.push('\n');
        }

        out
    }

    fn parse_ini_section_header(line: &str) -> Option<String> {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') && t.len() >= 2 {
            Some(t[1..t.len() - 1].to_string())
        } else {
            None
        }
    }

    fn lxqt_should_disable_section(section_name: &str, lines: &[String]) -> bool {
        // section_name looks like "Print.29" or "Shift%2BPrint.30"
        let shortcut_token = section_name.split('.').next().unwrap_or(section_name);

        if LXQT_SHORTCUT_TOKENS_TO_DISABLE
            .iter()
            .any(|t| t.eq_ignore_ascii_case(shortcut_token))
        {
            return true;
        }

        // Search for screenshot hints in keys
        for l in lines {
            let low = l.to_lowercase();

            // Comment=
            if low.starts_with("comment=") {
                if LXQT_COMMENT_KEYWORDS.iter().any(|k| low.contains(k)) {
                    return true;
                }
            }

            // Exec=
            if low.starts_with("exec=") {
                if LXQT_EXEC_KEYWORDS.iter().any(|k| low.contains(k)) {
                    return true;
                }
            }
        }

        false
    }

    fn reload_lxqt_globalkeys() -> bool {
        // Best-effort: send HUP to lxqt-globalkeysd if pkill exists.
        // If that fails, try restarting the module via lxqt-session is not reliable from CLI.
        // We keep it best-effort and harmless.
        let ok = Command::new("pkill")
            .args(["-HUP", "lxqt-globalkeysd"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        ok
    }

    // --------------------------
    // Openbox backend (neutralize Alt-Tab family)
    // --------------------------

    fn openbox_rc_paths() -> Vec<PathBuf> {
        // LXQt commonly uses ~/.config/openbox/lxqt-rc.xml
        // but some systems use ~/.config/openbox/rc.xml.
        let mut v = Vec::new();
        if let Some(home) = env::var_os("HOME") {
            let base = PathBuf::from(home).join(".config/openbox");
            v.push(base.join("lxqt-rc.xml"));
            v.push(base.join("rc.xml"));
        }
        v
    }

    fn disable_shortcuts_openbox_if_present(&self) -> Result<()> {
        for p in Self::openbox_rc_paths() {
            if p.exists() {
                let original = fs::read_to_string(&p)
                    .with_context(|| format!("Failed to read {}", p.display()))?;

                self.save_file_once(&p, &original)?;

                let updated = Self::openbox_neutralize_alt_tab(&original);
                if updated != original {
                    fs::write(&p, updated)
                        .with_context(|| format!("Failed to write {}", p.display()))?;
                    Self::reload_openbox();
                }
                break;
            }
        }
        Ok(())
    }

    fn openbox_neutralize_alt_tab(xml: &str) -> String {
        // We do a safe-ish textual strategy:
        // - If keybind entries for A-Tab / A-S-Tab exist, replace their <action ...> block with Execute "true".
        // - If none exist, insert new keybinds under <keyboard> ... </keyboard>.
        //
        // This avoids needing an XML parser crate.

        let mut out = xml.to_string();

        // Replace existing keybind blocks (best effort)
        for key in OPENBOX_KEYS_TO_NEUTRALIZE {
            out = Self::replace_openbox_keybind_with_noop(&out, key);
        }

        // If still no keybind for A-Tab present, insert new ones under <keyboard>
        let has_any = OPENBOX_KEYS_TO_NEUTRALIZE
            .iter()
            .any(|k| out.contains(&format!("key=\"{}\"", k)));

        if !has_any {
            out = Self::insert_openbox_noop_keybinds(&out);
        }

        out
    }

    fn replace_openbox_keybind_with_noop(xml: &str, key: &str) -> String {
        // Find `<keybind key="KEY">` ... `</keybind>` and replace inside with no-op Execute.
        // This is simplistic but works in typical Openbox rc.xml layouts.
        let start_pat = format!("<keybind key=\"{key}\">");
        let Some(start_idx) = xml.find(&start_pat) else {
            return xml.to_string();
        };

        // Find matching </keybind> after start
        let rest = &xml[start_idx..];
        let Some(end_rel) = rest.find("</keybind>") else {
            return xml.to_string();
        };
        let end_idx = start_idx + end_rel + "</keybind>".len();

        let noop_block = format!(
            "{start_pat}\n  <action name=\"Execute\">\n    <command>true</command>\n  </action>\n</keybind>"
        );

        let mut out = String::with_capacity(xml.len());
        out.push_str(&xml[..start_idx]);
        out.push_str(&noop_block);
        out.push_str(&xml[end_idx..]);
        out
    }

    fn insert_openbox_noop_keybinds(xml: &str) -> String {
        let insert_snippet = format!(
            "\n  <!-- Added by HookManager to disable task switching -->\n\
             〈KEYBINDS〉\n",
        )
        .replace(
            "〈KEYBINDS〉",
            &OPENBOX_KEYS_TO_NEUTRALIZE
                .iter()
                .map(|k| {
                    format!(
                        "  <keybind key=\"{k}\">\n\
                         \t<action name=\"Execute\">\n\
                         \t\t<command>true</command>\n\
                         \t</action>\n\
                         \t</keybind>\n"
                    )
                })
                .collect::<String>(),
        );

        // Insert right after <keyboard> tag (Openbox requires keybinds in <keyboard> section).
        if let Some(kbd_idx) = xml.find("<keyboard>") {
            let after = kbd_idx + "<keyboard>".len();
            let mut out = String::with_capacity(xml.len() + insert_snippet.len());
            out.push_str(&xml[..after]);
            out.push_str(&insert_snippet);
            out.push_str(&xml[after..]);
            out
        } else {
            // no keyboard section: do nothing
            xml.to_string()
        }
    }

    fn reload_openbox() -> bool {
        // Best-effort; common way is openbox --reconfigure
        Command::new("openbox")
            .arg("--reconfigure")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    // --------------------------
    // X11 PrintScreen disabling (universal for screenshots on X11)
    // --------------------------

    fn disable_print_key_x11(&self) -> Result<()> {
        // Store a backup of current keymap snippet for Print so we can restore.
        // Approach:
        // - xmodmap -pke
        // - keep line(s) containing "keycode 107" (Print on most layouts)
        // - then set keycode 107 = (empty) => removes Print symbols
        let out = Command::new("xmodmap").args(["-pke"]).output();
        let Ok(out) = out else { return Ok(()); };
        if !out.status.success() {
            return Ok(());
        }

        let pke = String::from_utf8_lossy(&out.stdout).to_string();
        // Save current mapping file once into temp file (per-session) for restore.
        // We'll store it as a "file" in memory to replay via `xmodmap -e`.
        // Simpler: save the exact "keycode 107 =" line if present.
        let print_line = pke
            .lines()
            .find(|l| l.trim_start().starts_with("keycode 107 ="))
            .map(|s| s.to_string());

        if let Some(line) = print_line {
            // Save as a pseudo-file in memory by using SavedFile with a sentinel path,
            // and interpret sentinel during restore.
            // However, easiest: just write an xmodmap script to ~/.cache and restore from there.
            if let Some(home) = env::var_os("HOME") {
                let cache = PathBuf::from(home).join(".cache/hookmanager_print_restore.xmodmap");
                // Save for restore only once
                let _ = self.save_file_once(&cache, &line);
                let _ = fs::write(&cache, format!("{line}\n"));
            }
        }

        // Disable Print
        let _ = Command::new("xmodmap")
            .args(["-e", "keycode 107 ="])
            .output();

        Ok(())
    }

    fn restore_print_key_x11(&self) -> bool {
        if let Some(home) = env::var_os("HOME") {
            let cache = PathBuf::from(home).join(".cache/hookmanager_print_restore.xmodmap");
            if cache.exists() {
                return Command::new("xmodmap")
                    .arg(cache)
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
            }
        }
        false
    }

    // --------------------------
    // Saved file helpers
    // --------------------------

    fn save_file_once(&self, path: &Path, original: &str) -> Result<()> {
        let mut saved = self.saved_files.lock().unwrap();
        if saved.iter().any(|f| f.path == path) {
            return Ok(());
        }
        saved.push(SavedFile {
            path: path.to_path_buf(),
            original: original.to_string(),
        });
        Ok(())
    }

    // --------------------------
    // Your original right-click logic (xmodmap pointer map)
    // --------------------------

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