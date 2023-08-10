use std::sync::Arc;
use artemis_core::types::Collector;
use crate::{middleware::RevmMiddleware, environment::ArbiterEvents};

/// Idea here is to have a collector that can be used to collect events from the revm middleware.
pub struct RevmCollector {
    client: Arc<RevmMiddleware>,
}

impl RevmCollector {
    pub fn new(client: Arc<RevmMiddleware>) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl Collector<ArbiterEvents> for RevmCollector {
    async fn get_event_stream(&self) -> anyhow::Result<artemis_core::types::CollectorStream<'_, ArbiterEvents>> {
        todo!()
    }
}


use anyhow::{Result, anyhow};
use artemis_core::types::Executor;
use ethers::providers::Middleware;


use crate::environment::ArbiterActions;

pub struct RevmExecutor {
    client: Arc<RevmMiddleware>,
}

impl RevmExecutor {
    pub fn new(client: Arc<RevmMiddleware>) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl Executor<ArbiterActions> for RevmExecutor {
    async fn execute(&self, action: ArbiterActions) -> Result<()> {
        match action {
            ArbiterActions::SendTx(tx, thing) => {
                self.client.send_transaction(&tx, None).await;
                Ok(())
            }
            _ => Err(anyhow!("Action not supported.")),
        }
    }
}