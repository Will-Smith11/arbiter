use crate::{
    bindings::liquid_exchange::LiquidExchange, middleware::RevmMiddleware,
};
use anyhow::Result;
use artemis_core::types::Executor;
use artemis_core::types::{Collector, Strategy};
use crossbeam_channel::TryRecvError;
use ethers::{prelude::FunctionCall, providers::Middleware, types::Transaction};
use ethers_core::types::U256;
use futures::Stream;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

/// type aliases for contract calls
pub type ContractFunctionCall = FunctionCall<Arc<RevmMiddleware>, RevmMiddleware, ()>;
/// type alias for contract calls that has a bool
pub type ContractHackCall = FunctionCall<Arc<RevmMiddleware>, RevmMiddleware, bool>;
/// Idea here is to have a collector that can be used to collect events from the revm middleware.
///
/// The actions that the [`Environment`] can take
#[derive(Clone, Debug)]
pub enum ArbiterActions {
    /// raw transaction
    SendTx(Transaction),
    /// contract call
    ContractCall(ContractFunctionCall),
    /// contract call that has a bool
    ContractHackCall(ContractHackCall),
}

/// Arbiter Events
#[derive(Clone, Debug)]
pub enum ArbiterEvents {
    /// eth logs event
    Event(Vec<ethers::types::Log>),
    /// Price update event to stocastic process
    UpdatePrice(bool),
    /// Signal that the price is updated
    PriceUpdated(f64),
    /// for testing counter
    Increment,
    /// for testing counter
    SetNumber(U256),
}

/// We present a collector that can be used to collect events from a chennel shared with other Agents(Straegies)
/// Notice this collector doesn't get events from the revm middleware directly, but rather from a channel shared with the strategies.
/// There is a nice LogCollector in the Artemis core crate that can be used nicely for eth logs as well.
pub struct AgentCollector {
    reciever_stream: crossbeam_channel::Receiver<ArbiterEvents>,

}

impl Stream for AgentCollector {
    type Item = ArbiterEvents;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Try to receive from the channel
        match self.reciever_stream.try_recv() {
            Ok(event) => Poll::Ready(Some(event)), // If there's an event, return it
            Err(TryRecvError::Empty) => Poll::Pending, // If the channel is empty, return Pending
            Err(TryRecvError::Disconnected) => Poll::Ready(None), // If the channel is disconnected, return None indicating the stream is finished
        }
    }
}

impl AgentCollector {
    /// Constructor for the [`AgentCollector`].
    pub fn new(event_channel: crossbeam_channel::Receiver<ArbiterEvents>) -> Self {
        // figure out how to make a channel between the strategies and the collector
        Self {
            reciever_stream: event_channel,
        }
    }
}

#[async_trait::async_trait]
impl Collector<ArbiterEvents> for AgentCollector {
    async fn get_event_stream(
        &self,
    ) -> anyhow::Result<artemis_core::types::CollectorStream<'_, ArbiterEvents>> {
        // some stream we make out of a channel between strategies and collector
        let stream = AgentCollector {
            reciever_stream: self.reciever_stream.clone(),
        };
        Ok(Box::pin(stream))
    }
}

/// These are revm strategies that can be used to interact with the revm middleware.
/// We present two strategies, one for the price updater and one for the arbitraguer.

pub struct PriceUpdaterStrategy {
    liquid_exchange: LiquidExchange<RevmMiddleware>,
    price_path: Vec<f64>,
    index: usize,
}

impl PriceUpdaterStrategy {
    /// Constructor for the [`PriceUpdaterStrategy`].
    /// The strategy is responsible for updating the stocastic price process on the liquid exchange.
    /// The strategy receives a price path and updates the price on the liquid exchange accordingly.
    /// The strategy gets events from the [`AgentCollector`], which gets them from a channel from the ArbitraguerStrategy
    pub fn new(liquid_exchange: LiquidExchange<RevmMiddleware>, price_path: Vec<f64>) -> Self {
        Self {
            liquid_exchange,
            price_path,
            index: 0,
        }
    }
    /// This function builds a function call to update the price on the liquid exchange.
    pub fn build_price_update_call(&self, price: f64) -> ContractFunctionCall {
        let wad_price = U256::from((price * 1e18) as u128);
        self.liquid_exchange.set_price(wad_price)
    }
}

#[async_trait::async_trait]
impl Strategy<ArbiterEvents, ArbiterActions> for PriceUpdaterStrategy {
    async fn sync_state(&mut self) -> Result<()> {
        todo!()
    }

    async fn process_event(&mut self, event: ArbiterEvents) -> Vec<ArbiterActions> {
        match event {
            ArbiterEvents::UpdatePrice(to_update) => {
                if to_update {
                    let next_price = self.price_path[self.index];
                    let tx = self.build_price_update_call(next_price);
                    self.index += 1;
                    vec![ArbiterActions::ContractCall(tx)]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }
}



/// this is a trait for the arbitraguer
/// it is intended to be used to detect arbitrage opportunities for different markets and tradding functions
/// implement it on a strategy
pub trait Arbitraguer {
    /// detect arbitrage opportunities
    fn detect_arbitrage(&self, new_price: f64) -> Option<usize>;
}

/// Executors for revm
/// This is a bespoke executor for the revm middleware
pub struct RevmExecutor {
    /// for evm communication
    client: Arc<RevmMiddleware>,
}

impl RevmExecutor {
    /// Constructor for the [`RevmExecutor`].
    pub fn new(client: Arc<RevmMiddleware>) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl Executor<ArbiterActions> for RevmExecutor {
    async fn execute(&self, action: ArbiterActions) -> Result<()> {
        match action {
            ArbiterActions::SendTx(tx) => {
                if self.client.send_transaction(&tx, None).await.is_err() {
                    return Err(anyhow::anyhow!("Failed to send transaction."));
                }
            }
            ArbiterActions::ContractCall(tx) => {
                if tx.call().await.is_err() {
                    return Err(anyhow::anyhow!("Failed to call contract."));
                }
            }
            _ => return Err(anyhow::anyhow!("Action not supported.")),
        }
        Ok(())
    }
}
