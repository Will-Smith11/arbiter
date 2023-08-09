#![warn(missing_docs)]
#![warn(unsafe_code)]

use artemis_core::{types::{Executor}, engine::Engine};
use async_lock::RwLock;
use ethers::{core::types::U64, prelude::ContractDeploymentTx, types::Filter, providers::Middleware};
use futures::Stream;
use tracing::info;
use revm::{
    db::{CacheDB, EmptyDB},
    primitives::{ExecutionResult, TxEnv, U256},
    EVM,
};
// use artemis_core::types::Strategy;
use anyhow::Result;

use std::{fmt::Debug, sync::Arc};


use crate::{
    math::stochastic_process::SeededPoisson,
    utils::convert_uint_to_u64, middleware::RevmMiddleware, bindings::arbiter_token::ArbiterToken,
};
use tokio::{sync::broadcast, task::JoinSet};

/// Result struct for the [`Environment`]. that wraps the [`ExecutionResult`] and the block number.
#[derive(Debug, Clone)]
pub struct RevmResult {
    /// The result of the execution.
    pub result: ExecutionResult,
    /// The block number of the execution.
    pub block_number: U64,
}

pub(crate) type ToTransact = bool;
pub(crate) type ResultSender = crossbeam_channel::Sender<RevmResult>;
pub(crate) type ResultReceiver = crossbeam_channel::Receiver<RevmResult>;
pub(crate) type TxSender = crossbeam_channel::Sender<(ToTransact, TxEnv, ResultSender)>;
pub(crate) type TxReceiver = crossbeam_channel::Receiver<(ToTransact, TxEnv, ResultSender)>;
pub(crate) type EventBroadcaster = broadcast::Sender<Vec<ethers::types::Log>>;

/// State enum for the [`Environment`].
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum State {
    /// The [`Environment`] is currently running.
    /// [`Agent`]s cannot be added if the environment is [`State::Running`].
    Running,
    /// The [`Environment`] is currently stopped.
    /// [`Agent`]s can only be added if the environment is [`State::Initialization`].
    Initialization,
}

#[derive(Clone)]
pub(crate) struct Socket {
    pub(crate) tx_sender: TxSender,
    pub(crate) tx_receiver: TxReceiver,
    pub(crate) event_sender: EventBroadcaster,
}

/// The environment struct.
pub struct Environment {
    /// The name of the environment.
    pub label: String,
    pub(crate) state: State,
    pub(crate) evm: EVM<CacheDB<EmptyDB>>,
    pub(crate) socket: Arc<Socket>,
    pub(crate) seeded_poisson: SeededPoisson,
    pub(crate) engine: Option<Engine<ArbiterEvents, ArbiterActions>>,
}

/// The actions that the [`Environment`] can take
#[derive(Clone, Debug)]
pub enum ArbiterActions {
    SendTx(TxEnv, ResultSender),
    // Alert(Address),
    Deploy(ContractDeploymentTx<Arc<RevmMiddleware>, RevmMiddleware, ArbiterToken<RevmMiddleware>>),
    SetPrice(U256),
    Mint(u128, ArbiterToken<RevmMiddleware>, Arc<RevmMiddleware>),
}


#[derive(Clone, Debug)]
pub enum ArbiterEvents {
    TxResult(RevmResult),
    Event(Vec<ethers::types::Log>),
}

// // TODO: This could be improved.
impl Debug for Socket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Socket").finish()
    }
}


impl Environment {
    /// Creates a new [`Environment`] with the given label.
    pub(crate) fn new<S: Into<String>>(label: S, block_rate: f64, seed: u64, mut engine: Engine<ArbiterEvents, ArbiterActions>) -> Self {
        let mut evm = EVM::new();
        let db = CacheDB::new(EmptyDB {});
        evm.database(db);
        let seeded_poisson = SeededPoisson::new(block_rate, seed);
        evm.env.cfg.limit_contract_code_size = Some(0x100000); // This is a large contract size limit, beware!
        evm.env.block.gas_limit = U256::MAX;

        let (tx_sender, tx_receiver) = crossbeam_channel::bounded(16);
        let (event_sender, _) = tokio::sync::broadcast::channel(16);
        let socket = Arc::new(Socket {
            tx_sender,
            tx_receiver,
            event_sender,
        });
        Self {
            label: label.into(),
            state: State::Initialization,
            evm,
            socket,
            seeded_poisson,
            engine: Some(engine),
        }
    }
    
    // TODO: Get rid of this probably
    /// returns mutable engine
    pub fn engine(&mut self) -> &mut Engine<ArbiterEvents, ArbiterActions> {
        self.engine.as_mut().unwrap()
    }


    pub(crate) async fn start_engine(&mut self) -> JoinSet<()> {
        println!("Starting engine");
        if let Some(engine) = self.engine.take() {
            if let Ok(set) = engine.run().await {
                println!("Engine has finished starting");
                set
            }
            else {
                panic!("Engine failed to start");
            }
        } else {
            panic!("Engine is missing");
        }
    }


    // TODO: Run should now run the agents as well as the evm.
    pub(crate) async fn run(&mut self) {
        let mut evm = self.evm.clone();
        let tx_receiver = self.socket.tx_receiver.clone();
        let event_broadcaster = self.socket.event_sender.clone();

        let mut seeded_poisson = self.seeded_poisson.clone();
        
        // self.state = State::Running;
        let mut counter: usize = 0;

        std::thread::spawn(move || {
            let mut expected_events_per_block = seeded_poisson.sample();

            while let Ok((to_transact, tx, sender)) = tx_receiver.recv() {
                // Execute the transaction, echo the logs to all agents, and report the execution result to the agent who made the transaction.
                if counter == expected_events_per_block {
                    counter = 0;
                    println!("EVM expected number of transactions reached. Moving to next block.");
                    println!("old block number: {:?}", evm.env.block.number);
                    evm.env.block.number += U256::from(1);
                    println!("new block number: {:?}", evm.env.block.number);
                    expected_events_per_block = seeded_poisson.sample();
                }

                evm.env.tx = tx;
                if to_transact {
                    let execution_result = match evm.transact_commit() {
                        Ok(val) => val,
                        // URGENT: change this to a custom error
                        Err(_) => panic!("failed"),
                    };

                    let _ = event_broadcaster.send(crate::utils::revm_logs_to_ethers_logs(
                        execution_result.logs(),
                    ));
                    let revm_result = RevmResult {
                        result: execution_result,
                        block_number: convert_uint_to_u64(evm.env.block.number).unwrap(),
                    };
                    sender.send(revm_result).unwrap();
                    counter += 1;
                } else {
                    let execution_result = match evm.transact() {
                        Ok(val) => val,
                        // URGENT: change this to a custom error
                        Err(_) => panic!("failed"),
                    };
                    let result_and_block = RevmResult {
                        result: execution_result.result,
                        block_number: convert_uint_to_u64(evm.env.block.number).unwrap(),
                    };
                    sender.send(result_and_block).unwrap();
                }
            }
        });
    }
}

#[cfg(test)]
pub(crate) mod tests {

    use super::*;

    pub(crate) const TEST_ENV_LABEL: &str = "test";

    #[test]
    fn new() {
        let engine = Engine::new();
        let env = Environment::new(TEST_ENV_LABEL.to_string(), 1.0, 1, engine);
        assert_eq!(env.label, TEST_ENV_LABEL);
        assert_eq!(env.state, State::Initialization);
    }

    #[test]
    fn run() {
        let engine = Engine::new();
        let mut environment = Environment::new(TEST_ENV_LABEL.to_string(), 1.0, 1, engine);
        environment.run();
        assert_eq!(environment.state, State::Running);
    }
}
