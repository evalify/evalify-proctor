use anyhow::Result;

use super::registry_restore::{self, RegistryDwordOverride, RegistryHive};

const SCOPE: &str = "devtools-policy";
const DEVTOOLS_AVAILABILITY: &str = "DeveloperToolsAvailability";
const CHROME_POLICY_PATH: &str = r"SOFTWARE\Policies\Google\Chrome";
const EDGE_POLICY_PATH: &str = r"SOFTWARE\Policies\Microsoft\Edge";

const DEVTOOLS_OVERRIDES: &[RegistryDwordOverride] = &[
    RegistryDwordOverride {
        hive: RegistryHive::LocalMachine,
        path: CHROME_POLICY_PATH,
        name: DEVTOOLS_AVAILABILITY,
        value: 2,
    },
    RegistryDwordOverride {
        hive: RegistryHive::LocalMachine,
        path: EDGE_POLICY_PATH,
        name: DEVTOOLS_AVAILABILITY,
        value: 2,
    },
];

pub fn disable_devtools() -> Result<()> {
    for entry in DEVTOOLS_OVERRIDES {
        registry_restore::set_dword(SCOPE, *entry)?;
    }
    Ok(())
}

pub fn enable_devtools() -> Result<()> {
    registry_restore::restore_scope(SCOPE)?;
    Ok(())
}

pub fn restore_legacy_lockdown_defaults() -> Result<()> {
    for path in [CHROME_POLICY_PATH, EDGE_POLICY_PATH] {
        registry_restore::set_dword_if_current(
            RegistryHive::LocalMachine,
            path,
            DEVTOOLS_AVAILABILITY,
            2,
            0,
        )?;
    }
    Ok(())
}
