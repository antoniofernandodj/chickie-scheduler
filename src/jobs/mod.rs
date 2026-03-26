use async_trait::async_trait;
use anyhow::Result;

#[async_trait]
pub trait CronJob: Send + Sync {
    fn name(&self) -> &'static str;
    async fn execute(&self) -> Result<()>;
}

pub mod backup;
pub mod cleanup;
pub mod health_check;