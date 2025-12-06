use async_trait::async_trait;
use anyhow::Result;

#[async_trait]
pub trait ProctorPlatform {
    async fn setup(&self) -> Result<()>;
    async fn launch_kioski(&self, url: &str) -> Result<()>;
    async fn teardown(&self) -> Result<()>;
}