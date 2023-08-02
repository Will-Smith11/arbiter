#![allow(missing_docs)]
use artemis_core::types::Strategy;

use anyhow::Result;
use ethers::providers::Middleware;

/// Core Event enum for the current strategy.
#[derive(Debug, Clone)]
pub enum Event {
    Deployer,
    NewEvent,
}

/// Core Action enum for the current strategy.
#[derive(Debug, Clone)]
pub enum Action {
    /// I asked frankie about making a PR with a revm_executor
    /// Seems promising
    SubmitTx,
}

pub struct BuyAndHoldStrategy<M> {
    pub amount: u64,
    pub client: M,
}

impl<M: Middleware + 'static> BuyAndHoldStrategy<M> {
    async fn new(client: M, amount: u64) -> Self {
        Self { amount, client  }
    }
}

#[async_trait::async_trait]
impl<M: Middleware + 'static> Strategy<Event, Action> for BuyAndHoldStrategy<M> {
    async fn process_event(&mut self, event: Event) -> Option<Action> {
        todo!()
    }
    async fn sync_state(&mut self) -> Result<()> {
        todo!()
    }
}
