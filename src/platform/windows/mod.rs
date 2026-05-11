mod devtools_policy;
mod guard;
mod hooks;
mod registry_restore;

use super::ProctorPlatform;
use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::process::{Child, Command};

use hooks::HookManager;

pub struct WindowsPlatform {
    hook_manager: HookManager,
}

impl WindowsPlatform {
    pub fn new() -> Self {
        Self {
            hook_manager: HookManager::new(),
        }
    }
}

pub fn maybe_run_lockdown_guard() -> Result<bool> {
    guard::maybe_run_from_args()
}

#[async_trait]
impl ProctorPlatform for WindowsPlatform {
    async fn setup(&self) -> Result<()> {
        registry_restore::restore_all()
            .context("Failed to restore stale Windows lockdown state")?;
        devtools_policy::restore_legacy_lockdown_defaults()
            .context("Failed to restore legacy Windows devtools policy values")?;
        self.hook_manager
            .restore_legacy_lockdown_defaults()
            .context("Failed to restore legacy Windows input lockdown values")?;
        guard::spawn_for_current_process().context("Failed to start Windows lockdown guard")?;

        devtools_policy::disable_devtools().context("Failed to disable devtools")?;
        println!("Disabled devtools");

        if let Err(err) = self.hook_manager.install() {
            let _ = devtools_policy::enable_devtools();
            return Err(err).context("Failed to install hooks");
        }

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
        let hooks_result = self
            .hook_manager
            .uninstall()
            .context("Failed to uninstall hooks");
        if hooks_result.is_ok() {
            println!("Uninstalled input hooks");
        }

        let devtools_result =
            devtools_policy::enable_devtools().context("Failed to re-enable devtools");
        if devtools_result.is_ok() {
            println!("Re-enabled devtools");
        }

        hooks_result?;
        devtools_result?;
        Ok(())
    }
}
