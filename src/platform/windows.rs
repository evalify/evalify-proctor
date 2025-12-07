use async_trait::async_trait;
use anyhow::Result;
use tokio::process::Command;
use tokio::process::Child;
use super::ProctorPlatform;
use browsers::Browser;

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProctorPlatform for WindowsPlatform {

    async fn setup(&self) -> Result<()> {
        Ok(())
    }

    async fn launch_kioski(&self, url: &str) ->Result<Child> {
        let browser = Browser::find().context("Failed to find browser")?;
        let flags = browser.get_flags(url, "http://127.0.0.1:8080");

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
