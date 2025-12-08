use async_trait::async_trait;
use anyhow::{Result, Context};
use tokio::process::Command;
use tokio::process::Child;
use super::ProctorPlatform;

pub struct MacPlatform;

impl MacPlatform {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProctorPlatform for MacPlatform {
    async fn setup(&self) -> Result<()> {
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
        Ok(())
    }
}
