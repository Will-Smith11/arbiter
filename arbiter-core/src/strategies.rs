use crate::middleware::RevmMiddleware;
use anyhow::Result;
use artemis_core::types::{Collector, Executor};
use crossbeam_channel::TryRecvError;
use ethers::{prelude::FunctionCall, providers::Middleware, types::Transaction};
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
pub enum SimulationActions {
    /// raw transaction
    SendTx(Transaction),
    /// contract call
    ContractCall(ContractFunctionCall),
    /// contract call that has a bool
    ContractHackCall(ContractHackCall),
    Reply(String),
}

/// Arbiter Events
#[derive(Clone, Debug)]
pub enum SimulationEvents {
    /// eth logs event
    Event(Vec<ethers::types::Log>),
    Message(String),
}

/// We present a collector that can be used to collect events from a chennel shared with other Agents(Straegies)
/// Notice this collector doesn't get events from the revm middleware directly, but rather from a channel shared with the strategies.
/// There is a nice LogCollector in the Artemis core crate that can be used nicely for eth logs as well.
pub struct SimulationCollector {
    reciever_stream: crossbeam_channel::Receiver<SimulationEvents>,
}

impl Stream for SimulationCollector {
    type Item = SimulationEvents;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Try to receive from the channel
        println!("polling");
        match self.reciever_stream.try_recv() {
            Ok(event) => Poll::Ready(Some(event)), // If there's an event, return it
            Err(TryRecvError::Empty) => Poll::Pending, // If the channel is empty, return Pending
            Err(TryRecvError::Disconnected) => Poll::Ready(None), // If the channel is disconnected, return None indicating the stream is finished
        }
    }
}

impl SimulationCollector {
    /// Constructor for the [`AgentCollector`].
    pub fn new(event_channel: crossbeam_channel::Receiver<SimulationEvents>) -> Self {
        // figure out how to make a channel between the strategies and the collector
        Self {
            reciever_stream: event_channel,
        }
    }
}

#[async_trait::async_trait]
impl Collector<SimulationEvents> for SimulationCollector {
    async fn get_event_stream(
        &self,
    ) -> anyhow::Result<artemis_core::types::CollectorStream<'_, SimulationEvents>> {
        // some stream we make out of a channel between strategies and collector
        println!("getting stream");
        let stream = SimulationCollector {
            reciever_stream: self.reciever_stream.clone(),
        };
        Ok(Box::pin(stream))
    }
}

/// Executors for revm
/// This is a bespoke executor for the revm middleware
pub struct SimulationExecutor {
    /// for evm communication
    client: Arc<RevmMiddleware>,
}

impl SimulationExecutor {
    /// Constructor for the [`RevmExecutor`].
    pub fn new(client: Arc<RevmMiddleware>) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl Executor<SimulationActions> for SimulationExecutor {
    async fn execute(&self, action: SimulationActions) -> Result<()> {
        match action {
            SimulationActions::SendTx(tx) => {
                if self.client.send_transaction(&tx, None).await.is_err() {
                    return Err(anyhow::anyhow!("Failed to send transaction."));
                }
            }
            SimulationActions::ContractCall(tx) => {
                if tx.call().await.is_err() {
                    return Err(anyhow::anyhow!("Failed to call contract."));
                }
            }
            SimulationActions::Reply(msg) => {
                println!("{}", msg);
            }
            _ => return Err(anyhow::anyhow!("Action not supported.")),
        }
        Ok(())
    }
}
