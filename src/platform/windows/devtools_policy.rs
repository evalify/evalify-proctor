use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;
use anyhow::Result;

pub fn disable_devtools() -> Result<()> {
    disable_chrome_devtools()?;
    disable_edge_devtools()?;
    Ok(())
}

pub fn enable_devtools() -> Result<()> {
    enable_chrome_devtools()?;
    enable_edge_devtools()?;
    Ok(())
}

fn disable_edge_devtools() -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (key, _) = hklm.create_subkey("SOFTWARE\\Policies\\Microsoft\\Edge")?;
    key.set_value("DeveloperToolsAvailability", &2u32)?;
    Ok(())
}

fn disable_chrome_devtools() -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (key, _) = hklm.create_subkey("SOFTWARE\\Policies\\Google\\Chrome")?;
    key.set_value("DeveloperToolsAvailability", &2u32)?;
    Ok(())
}

fn enable_edge_devtools() -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (key, _) = hklm.create_subkey("SOFTWARE\\Policies\\Microsoft\\Edge")?;
    key.set_value("DeveloperToolsAvailability", &0u32)?;
    Ok(())
}

fn enable_chrome_devtools() -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (key, _) = hklm.create_subkey("SOFTWARE\\Policies\\Google\\Chrome")?;
    key.set_value("DeveloperToolsAvailability", &0u32)?;
    Ok(())
}