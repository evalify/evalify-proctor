use anyhow::{Context, Result};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};

use crate::platform::windows::registry_restore::{self, RegistryDwordOverride, RegistryHive};

use super::touchpad_cache;

const PRECISION_TOUCHPAD_PATH: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\PrecisionTouchPad";
const TOUCH_GESTURE_PATH: &str = r"Control Panel\Desktop";
const EXPLORER_POLICY_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\Explorer";
const WINDOWS_EXPLORER_POLICY_PATH: &str = r"SOFTWARE\Policies\Microsoft\Windows\Explorer";
const SCOPE: &str = "windows-input-lockdown";

const TOUCHPAD_DWORD_OVERRIDES: &[RegistryDwordOverride] = &[
    touchpad_override("ThreeFingerTapEnabled", 0),
    touchpad_override("ThreeFingerSlideEnabled", 0),
    touchpad_override("FourFingerTapEnabled", 0),
    touchpad_override("FourFingerSlideEnabled", 0),
    touchpad_override("TwoFingerTapEnabled", 0),
    touchpad_override("RightClickZoneEnabled", 0),
    touchpad_override("ZoomEnabled", 0),
];

const SHELL_POLICY_DWORD_OVERRIDES: &[RegistryDwordOverride] = &[
    hkcu_override(TOUCH_GESTURE_PATH, "TouchGestureSetting", 0),
    hkcu_override(EXPLORER_POLICY_PATH, "NoWinKeys", 1),
    hkcu_override(WINDOWS_EXPLORER_POLICY_PATH, "HideTaskViewButton", 1),
];

const LEGACY_TOUCHPAD_DEFAULTS: &[(&str, u32)] = &[
    ("ThreeFingerTapEnabled", 1),
    ("ThreeFingerSlideEnabled", 1),
    ("FourFingerTapEnabled", 1),
    ("FourFingerSlideEnabled", 1),
    ("TwoFingerTapEnabled", 1),
    ("RightClickZoneEnabled", 1),
    ("ZoomEnabled", 1),
];

pub(super) struct TouchpadPolicy;

impl TouchpadPolicy {
    pub(super) fn new() -> Self {
        Self
    }

    pub(super) fn install(&self) -> Result<()> {
        for entry in TOUCHPAD_DWORD_OVERRIDES {
            registry_restore::set_dword(SCOPE, *entry)?;
        }

        for entry in SHELL_POLICY_DWORD_OVERRIDES {
            registry_restore::set_dword(SCOPE, *entry)?;
        }

        broadcast_settings_changed();
        touchpad_cache::apply_touchpad_setting_changes()
            .context("Failed to apply touchpad gesture settings immediately")?;
        Ok(())
    }

    pub(super) fn uninstall(&self) -> Result<()> {
        registry_restore::restore_scope(SCOPE)
            .context("Failed to restore Windows input lockdown journal")?;
        broadcast_settings_changed();
        touchpad_cache::apply_touchpad_setting_changes()
            .context("Failed to apply restored touchpad gesture settings immediately")?;
        Ok(())
    }

    pub(super) fn restore_legacy_lockdown_defaults(&self) -> Result<()> {
        for (name, default_value) in LEGACY_TOUCHPAD_DEFAULTS {
            registry_restore::set_dword_if_current(
                RegistryHive::CurrentUser,
                PRECISION_TOUCHPAD_PATH,
                name,
                0,
                *default_value,
            )?;
        }

        registry_restore::set_dword_if_current(
            RegistryHive::CurrentUser,
            TOUCH_GESTURE_PATH,
            "TouchGestureSetting",
            0,
            1,
        )?;
        registry_restore::delete_value_if_current_dword(
            RegistryHive::CurrentUser,
            EXPLORER_POLICY_PATH,
            "NoWinKeys",
            1,
        )?;
        registry_restore::delete_value_if_current_dword(
            RegistryHive::CurrentUser,
            WINDOWS_EXPLORER_POLICY_PATH,
            "HideTaskViewButton",
            1,
        )?;

        broadcast_settings_changed();
        touchpad_cache::apply_touchpad_setting_changes()
            .context("Failed to apply legacy touchpad gesture defaults immediately")?;
        Ok(())
    }
}

fn broadcast_settings_changed() {
    broadcast_setting_change(w!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\PrecisionTouchPad"
    ));
    broadcast_setting_change(w!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer"
    ));
    broadcast_setting_change(w!("Software\\Policies\\Microsoft\\Windows\\Explorer"));
    broadcast_setting_change(w!("Control Panel\\Desktop"));
}

fn broadcast_setting_change(area: windows::core::PCWSTR) {
    let mut result = 0usize;
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND(HWND_BROADCAST.0),
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(area.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            200,
            Some(&mut result as *mut usize),
        );
    }
}

const fn touchpad_override(name: &'static str, value: u32) -> RegistryDwordOverride {
    hkcu_override(PRECISION_TOUCHPAD_PATH, name, value)
}

const fn hkcu_override(
    path: &'static str,
    name: &'static str,
    value: u32,
) -> RegistryDwordOverride {
    RegistryDwordOverride {
        hive: RegistryHive::CurrentUser,
        path,
        name,
        value,
    }
}
