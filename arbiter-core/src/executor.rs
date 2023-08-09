use std::sync::Arc;
use anyhow::{Result, anyhow};
use artemis_core::types::Executor;
use ethers::providers::Middleware;


use crate::{middleware::RevmMiddleware, environment::ArbiterActions};

struct RevmExecutor {
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