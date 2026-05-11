use std::io::ErrorKind;

use anyhow::{Context, Result};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS};
use winreg::{RegKey, HKEY};

const JOURNAL_PATH: &str = r"SOFTWARE\Evalify\Proctor\RestoreJournal";

#[derive(Clone, Copy)]
pub(crate) enum RegistryHive {
    CurrentUser,
    LocalMachine,
}

#[derive(Clone, Copy)]
pub(crate) struct RegistryDwordOverride {
    pub(crate) hive: RegistryHive,
    pub(crate) path: &'static str,
    pub(crate) name: &'static str,
    pub(crate) value: u32,
}

pub(crate) fn set_dword(scope: &str, entry: RegistryDwordOverride) -> Result<()> {
    save_original_once(scope, entry.hive, entry.path, entry.name)?;

    let root = entry.hive.root();
    let (key, _) = root
        .create_subkey(entry.path)
        .with_context(|| format!("Opening {}\\{}", entry.hive.label(), entry.path))?;

    key.set_value(entry.name, &entry.value).with_context(|| {
        format!(
            "Writing {}\\{}\\{}",
            entry.hive.label(),
            entry.path,
            entry.name
        )
    })?;
    Ok(())
}

pub(crate) fn restore_all() -> Result<usize> {
    let mut restored = 0usize;
    let mut first_error = None;

    for backup_hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        match restore_all_from_backup_hive(backup_hive) {
            Ok(count) => restored += count,
            Err(err) if first_error.is_none() => first_error = Some(err),
            Err(_) => {}
        }
    }

    if let Some(err) = first_error {
        return Err(err);
    }

    Ok(restored)
}

pub(crate) fn restore_scope(scope: &str) -> Result<usize> {
    let mut restored = 0usize;
    let mut first_error = None;

    for backup_hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        match restore_scope_from_backup_hive(backup_hive, scope) {
            Ok(count) => restored += count,
            Err(err) if first_error.is_none() => first_error = Some(err),
            Err(_) => {}
        }
    }

    if let Some(err) = first_error {
        return Err(err);
    }

    Ok(restored)
}

pub(crate) fn set_dword_if_current(
    hive: RegistryHive,
    path: &str,
    name: &str,
    expected_current: u32,
    restored_value: u32,
) -> Result<()> {
    update_dword_if_current(hive, path, name, expected_current, Some(restored_value))
}

pub(crate) fn delete_value_if_current_dword(
    hive: RegistryHive,
    path: &str,
    name: &str,
    expected_current: u32,
) -> Result<()> {
    update_dword_if_current(hive, path, name, expected_current, None)
}

fn update_dword_if_current(
    hive: RegistryHive,
    path: &str,
    name: &str,
    expected_current: u32,
    restored_value: Option<u32>,
) -> Result<()> {
    let root = hive.root();
    let key = match root.open_subkey_with_flags(path, KEY_ALL_ACCESS) {
        Ok(key) => key,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("Opening {}\\{path}", hive.label())),
    };

    let current = match key.get_value::<u32, _>(name) {
        Ok(value) => value,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("Reading {}\\{}\\{}", hive.label(), path, name))
        }
    };

    if current != expected_current {
        return Ok(());
    }

    match restored_value {
        Some(value) => key
            .set_value(name, &value)
            .with_context(|| format!("Writing {}\\{}\\{}", hive.label(), path, name))?,
        None => {
            if let Err(err) = key.delete_value(name) {
                if err.kind() != ErrorKind::NotFound {
                    return Err(err)
                        .with_context(|| format!("Deleting {}\\{}\\{}", hive.label(), path, name));
                }
            }
        }
    }

    Ok(())
}

fn save_original_once(scope: &str, hive: RegistryHive, path: &str, name: &str) -> Result<()> {
    let journal_root = hive.root();
    let (scope_key, _) = journal_root
        .create_subkey(format!("{JOURNAL_PATH}\\{scope}"))
        .with_context(|| {
            format!(
                "Opening {}\\{}\\{}",
                hive.backup_label(),
                JOURNAL_PATH,
                scope
            )
        })?;

    let entry_name = journal_entry_name(hive, path, name);
    if let Ok(existing_entry) = scope_key.open_subkey(&entry_name) {
        if existing_entry.get_value::<u32, _>("Existed").is_ok() {
            return Ok(());
        }
        drop(existing_entry);
        let _ = scope_key.delete_subkey_all(&entry_name);
    }

    let target_root = hive.root();
    let original_value = match target_root
        .open_subkey(path)
        .and_then(|key| key.get_raw_value(name))
    {
        Ok(original) => Some(original),
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => {
            return Err(err)
                .with_context(|| format!("Reading original {}\\{}\\{}", hive.label(), path, name));
        }
    };

    let (entry_key, _) = scope_key
        .create_subkey(&entry_name)
        .with_context(|| format!("Creating restore journal entry {scope}\\{entry_name}"))?;

    entry_key.set_value("Hive", &hive.id())?;
    entry_key.set_value("Path", &path.to_string())?;
    entry_key.set_value("Name", &name.to_string())?;

    if let Some(original) = original_value {
        entry_key.set_raw_value("Original", &original)?;
        entry_key.set_value("Existed", &1u32)?;
    } else {
        entry_key.set_value("Existed", &0u32)?;
    }

    Ok(())
}

fn restore_all_from_backup_hive(backup_hive: HKEY) -> Result<usize> {
    let root = RegKey::predef(backup_hive);
    let journal = match root.open_subkey_with_flags(JOURNAL_PATH, KEY_ALL_ACCESS) {
        Ok(key) => key,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err).context("Opening Windows restore journal"),
    };

    let scopes = journal
        .enum_keys()
        .collect::<std::io::Result<Vec<_>>>()
        .context("Reading Windows restore journal scopes")?;

    let mut restored = 0usize;
    for scope in scopes {
        restored += restore_scope_from_backup_hive(backup_hive, &scope)?;
    }

    Ok(restored)
}

fn restore_scope_from_backup_hive(backup_hive: HKEY, scope: &str) -> Result<usize> {
    let root = RegKey::predef(backup_hive);
    let journal = match root.open_subkey_with_flags(JOURNAL_PATH, KEY_ALL_ACCESS) {
        Ok(key) => key,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err).context("Opening Windows restore journal"),
    };

    let scope_key = match journal.open_subkey_with_flags(scope, KEY_ALL_ACCESS) {
        Ok(key) => key,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err).with_context(|| format!("Opening restore scope {scope}")),
    };

    let entries = scope_key
        .enum_keys()
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("Reading restore entries for {scope}"))?;

    let mut restored = 0usize;
    let mut first_error = None;

    for entry in entries {
        match restore_entry(&scope_key, &entry) {
            Ok(()) => {
                restored += 1;
                if let Err(err) = scope_key.delete_subkey_all(&entry) {
                    if first_error.is_none() {
                        first_error =
                            Some(anyhow::Error::new(err).context(format!(
                                "Deleting restore journal entry {scope}\\{entry}"
                            )));
                    }
                }
            }
            Err(err) if first_error.is_none() => first_error = Some(err),
            Err(_) => {}
        }
    }

    drop(scope_key);
    let _ = journal.delete_subkey(scope);

    if let Some(err) = first_error {
        return Err(err);
    }

    Ok(restored)
}

fn restore_entry(scope_key: &RegKey, entry_name: &str) -> Result<()> {
    let entry = scope_key
        .open_subkey(entry_name)
        .with_context(|| format!("Opening restore journal entry {entry_name}"))?;

    let hive_id = entry
        .get_value::<u32, _>("Hive")
        .with_context(|| format!("Reading restore hive for {entry_name}"))?;
    let hive = RegistryHive::from_id(hive_id)
        .with_context(|| format!("Unknown restore hive {hive_id} in {entry_name}"))?;
    let path = entry
        .get_value::<String, _>("Path")
        .with_context(|| format!("Reading restore path for {entry_name}"))?;
    let name = entry
        .get_value::<String, _>("Name")
        .with_context(|| format!("Reading restore value name for {entry_name}"))?;
    let existed = entry
        .get_value::<u32, _>("Existed")
        .with_context(|| format!("Reading restore existence marker for {entry_name}"))?
        != 0;

    let root = hive.root();
    let (target, _) = root
        .create_subkey(&path)
        .with_context(|| format!("Opening {}\\{}", hive.label(), path))?;

    if existed {
        let original = entry
            .get_raw_value("Original")
            .with_context(|| format!("Reading original value for {entry_name}"))?;
        target
            .set_raw_value(&name, &original)
            .with_context(|| format!("Restoring {}\\{}\\{}", hive.label(), path, name))?;
    } else if let Err(err) = target.delete_value(&name) {
        if err.kind() != ErrorKind::NotFound {
            return Err(err)
                .with_context(|| format!("Deleting {}\\{}\\{}", hive.label(), path, name));
        }
    }

    Ok(())
}

impl RegistryHive {
    fn root(self) -> RegKey {
        RegKey::predef(match self {
            Self::CurrentUser => HKEY_CURRENT_USER,
            Self::LocalMachine => HKEY_LOCAL_MACHINE,
        })
    }

    fn id(self) -> u32 {
        match self {
            Self::CurrentUser => 1,
            Self::LocalMachine => 2,
        }
    }

    fn from_id(id: u32) -> Option<Self> {
        match id {
            1 => Some(Self::CurrentUser),
            2 => Some(Self::LocalMachine),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::CurrentUser => "HKCU",
            Self::LocalMachine => "HKLM",
        }
    }

    fn backup_label(self) -> &'static str {
        self.label()
    }
}

fn journal_entry_name(hive: RegistryHive, path: &str, name: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in hive.label().bytes().chain(path.bytes()).chain(name.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    format!("{hash:016x}-{}", sanitize_name(name))
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
