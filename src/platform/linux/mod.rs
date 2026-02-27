mod hooks;
mod policy;

use async_trait::async_trait;
use anyhow::{Result, Context};
use tokio::process::{Command, Child};
use super::ProctorPlatform;

use hooks::HookManager;

pub struct LinuxPlatform {
    hook_manager: HookManager,
}

impl LinuxPlatform {
    pub fn new() -> Self {
        Self {
            hook_manager: HookManager::new(),
        }
    }
}

#[async_trait]
impl ProctorPlatform for LinuxPlatform {
    async fn setup(&self) -> Result<()> {
        policy::disable_devtools().context("Failed to disable devtools")?;
        println!("Disabled devtools");
        self.hook_manager.install().context("Failed to install hooks")?;
        println!("Installed input hooks");
        Ok(())
    }

    async fn launch_kioski(&self, url: &str) -> Result<Child> {
        let browser = crate::browsers::find().context("Failed to find browser")?;
        let proxy_url = format!("http://127.0.0.1:{}", crate::config::CONFIG.proxy_port);
        let flags = browser.get_flags(url, &proxy_url);

        let child = Command::new(browser.path)
            .args(flags)
            .spawn()
            .context("Failed to launch browser")?;

        Ok(child)
    }

    async fn teardown(&self) -> Result<()> {
        self.hook_manager.uninstall().context("Failed to uninstall hooks")?;
        println!("Uninstalled input hooks");
        policy::enable_devtools().context("Failed to re-enable devtools")?;
        println!("Re-enabled devtools");
        Ok(())
    }
}
