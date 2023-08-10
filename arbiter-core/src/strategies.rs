use std::{sync::Arc, task::{Context, Poll}, pin::Pin};
use artemis_core::types::{Collector, Strategy};
use crossbeam_channel::TryRecvError;
use ethers_core::types::U256;
use anyhow::{Result, anyhow};
use artemis_core::types::Executor;
use ethers::{providers::Middleware, prelude::{ContractDeploymentTx, FunctionCall}, types::Transaction};
use futures::Stream;
use serde::__private::de;
use crate::{middleware::RevmMiddleware, environment::{ResultSender, RevmResult}, bindings::{arbiter_token::ArbiterToken, liquid_exchange::{LiquidExchange, self}}};


type ContractFunctionCall = FunctionCall<Arc<RevmMiddleware>, RevmMiddleware, ()>;
/// Idea here is to have a collector that can be used to collect events from the revm middleware.
/// 
/// The actions that the [`Environment`] can take
#[derive(Clone, Debug)]
pub enum ArbiterActions {
    SendTx(Transaction, ResultSender),
    ContractCall(ContractFunctionCall),
    NextTransaction,
}

#[derive(Clone, Debug)]
pub enum ArbiterEvents {
    TxResult(RevmResult),
    Event(Vec<ethers::types::Log>),
    UpdatePrice(bool),
    PriceUpdated(f64),
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
            Ok(event) => Poll::Ready(Some(event)),  // If there's an event, return it
            Err(TryRecvError::Empty) => Poll::Pending, // If the channel is empty, return Pending
            Err(TryRecvError::Disconnected) => Poll::Ready(None) // If the channel is disconnected, return None indicating the stream is finished
        }
    }
}

impl AgentCollector {
    /// Constructor for the [`AgentCollector`].
    pub fn new( event_channel: crossbeam_channel::Receiver<ArbiterEvents>) -> Self {
        // figure out how to make a channel between the strategies and the collector
        Self {reciever_stream: event_channel }
    }
}

#[async_trait::async_trait]
impl Collector<ArbiterEvents> for AgentCollector {
    async fn get_event_stream(&self) -> anyhow::Result<artemis_core::types::CollectorStream<'_, ArbiterEvents>> {
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
    pub fn new( liquid_exchange: LiquidExchange<RevmMiddleware>, price_path: Vec<f64>) -> Self {
        Self { liquid_exchange, price_path, index: 0 }
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


/// This is the arbitraguer strategy.
pub struct ArbitraguerStrategy {
    client: Arc<RevmMiddleware>,
    // I am not sure the best way to make the contracts generic
    // What i would like to do is have the contstructor take in two exchange contracts and then we don't neeed the client
    // There might be a way to just use the client and maybe the exchange addresses, but I am not sure if it will be clean.
    // exchanges: (LiquidExchange<RevmMiddleware>, <RevmMiddleware>),
    exchange_prices: (f64, f64),
    event_sender: crossbeam_channel::Sender<ArbiterEvents>,
}

impl ArbitraguerStrategy {
    pub fn new(client: Arc<RevmMiddleware>, event_sender: crossbeam_channel::Sender<ArbiterEvents>) -> Self {
        Self { client, exchange_prices: (0.0, 0.0), event_sender }
    }

    pub fn build_arbitrage_call(&self, arb_size: usize) -> (ContractFunctionCall, ContractFunctionCall) {
        // one call for each leg
        todo!()
    }
}

impl Arbitraguer for ArbitraguerStrategy {
    /// check bounds, if in bounds return the size of the arbitrage
    /// else return None
    fn detect_arbitrage(&self, new_price: f64) -> Option<usize>{ 
        todo!()
    }
}

#[async_trait::async_trait]
impl Strategy<ArbiterEvents, ArbiterActions> for ArbitraguerStrategy {
    async fn sync_state(&mut self) -> Result<()> {
        todo!()
    }

    async fn process_event(&mut self, event: ArbiterEvents) -> Vec<ArbiterActions> {

        match event {
            ArbiterEvents::PriceUpdated(new_price) => {
                if let arb_size = self.detect_arbitrage(new_price) {
                    let (tx1, tx2) = self.build_arbitrage_call(arb_size.unwrap());
                    vec![ArbiterActions::ContractCall(tx1), ArbiterActions::ContractCall(tx2)]
                } else {
                    let _ = self.event_sender.send(ArbiterEvents::UpdatePrice(true));
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
trait Arbitraguer {
    fn detect_arbitrage(&self, new_price: f64) -> Option<usize>;
}

/// Executors for revm
/// This is a bespoke executor for the revm middleware
pub struct RevmExecutor {
    /// for evm communication
    client: Arc<RevmMiddleware>,
}

impl RevmExecutor {
    pub fn new(client: Arc<RevmMiddleware>, event_sender: crossbeam_channel::Sender<ArbiterEvents>) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl Executor<ArbiterActions> for RevmExecutor {
    async fn execute(&self, action: ArbiterActions) -> Result<()> {
        match action {
            ArbiterActions::SendTx(tx, thing) => {
                self.client.send_transaction(&tx, None).await.is_ok();
                Ok(());
            }
            ArbiterActions::ContractCall(tx) => {
                let result = tx.call().await;
                Ok(());
            }
            _ => (),
        }
        Ok(())
    }
}