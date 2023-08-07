#![warn(missing_docs)]
#![warn(unsafe_code)]

use artemis_core::{types::{Strategy, Executor}, engine::Engine};
use ethers::{core::types::U64, types::Address, prelude::ContractDeploymentTx};
// use core::result::Result;
// use crossbeam_channel::{unbounded, Receiver, Sender};
use revm::{
    db::{CacheDB, EmptyDB},
    primitives::{ExecutionResult, TxEnv, U256},
    EVM,
};
// use artemis_core::types::Strategy;
use anyhow::Result;
use serde::__private::de;
use std::{fmt::Debug, sync::{Arc, Mutex}, rc::Rc, cell::RefCell};


use crate::{
    math::stochastic_process::SeededPoisson,
    utils::convert_uint_to_u64, middleware::RevmMiddleware, bindings::arbiter_token::ArbiterToken,
};
use tokio::sync::broadcast;

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

/// The Proxy Pattern offers several benefits:
/// Isolation: The proxy can isolate consumers from the details of the proxied type. If the Engine has quirks or complexities, the proxy can hide or manage these.
/// Extended Behavior: The proxy can add behavior before or after forwarding a call to the original type. For example, it can log calls, manage lifecycles, or handle edge cases.
/// Interface Adaption: If the Engine type has a problematic interface (like the consuming run method in this case), the proxy can provide a more convenient or idiomatic interface to callers.
pub struct EngineProxy {
    engine: Option<Engine<ArbiterEvents, ArbiterActions>>,
}

impl EngineProxy {
    /// Constructor function to instantiate a [`EngineProxy`].
    pub fn new(engine: Engine<ArbiterEvents, ArbiterActions>) -> Self {
        Self {
            engine: Some(engine),
        }
    }

    /// Adds a [`Strategy`] to the [`EngineProxy`].
    pub fn add_strategy(&mut self, strategy: Box<dyn Strategy<ArbiterEvents, ArbiterActions>>) {
        if let Some(engine) = &mut self.engine {
            engine.add_strategy(strategy);
        } else {
            // Handle the case where the engine might be missing.
            // For example, log an error or panic.
            panic!("Engine is missing")
        }
    }

    /// Adds an [`Executor`] to the [`EngineProxy`].
    pub fn add_executor(&mut self, executor: Box<dyn Executor<ArbiterActions>>) {
        if let Some(engine) = &mut self.engine {
            engine.add_executor(executor);
        } else {
            panic!("Engine is missing")
        }
    }

    /// Runs the [`EngineProxy`].
    pub async fn run(&mut self) -> Result<()> {
        if let Some(engine) = self.engine.take() {
            let _result = engine.run().await.unwrap();
            Ok(())
        } else {
            panic!("Engine is missing");
        }
    }
}
/// The environment struct.
pub struct Environment {
    pub label: String,
    pub(crate) state: State,
    pub(crate) evm: EVM<CacheDB<EmptyDB>>,
    pub(crate) socket: Socket,
    pub(crate) seeded_poisson: SeededPoisson,
    pub(crate) engine: EngineProxy,
}

/// The actions that the [`Environment`] can take
#[derive(Clone, Debug)]
pub enum ArbiterActions {
    SendTx(TxEnv, ResultSender),
    // Alert(Address),
    Deploy(ContractDeploymentTx<Arc<RevmMiddleware>, RevmMiddleware, ArbiterToken<RevmMiddleware>>)

}


#[derive(Clone, Debug)]
pub enum ArbiterEvents {
    TxResult(RevmResult),
    Start(ArbiterToken<RevmMiddleware>),
}

// TODO: This could be improved.
impl Debug for Socket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Socket").finish()
    }
}

#[async_trait::async_trait]
impl Executor<ArbiterActions> for Socket {
    async fn execute(&self, _arbiter: ArbiterActions) -> Result<()> {
        let action = match _arbiter {
            ArbiterActions::SendTx(tx_env, sender) => {
                self.tx_sender.send((true, tx_env, sender))?;
            }
            ArbiterActions::Deploy(deploy_tx) => {
                let thing = deploy_tx.send().await?;
                print!("{:?}", thing);

            }
        };
        Ok(action)
    }
}


impl Environment {
    /// Creates a new [`Environment`] with the given label.
    pub(crate) fn new<S: Into<String>>(label: S, block_rate: f64, seed: u64) -> Self {
        let mut evm = EVM::new();
        let db = CacheDB::new(EmptyDB {});
        evm.database(db);
        let seeded_poisson = SeededPoisson::new(block_rate, seed);
        evm.env.cfg.limit_contract_code_size = Some(0x100000); // This is a large contract size limit, beware!
        evm.env.block.gas_limit = U256::MAX;

        let (tx_sender, tx_receiver) = crossbeam_channel::bounded(16);
        let (event_sender, _) = tokio::sync::broadcast::channel(16);
        let socket = Socket {
            tx_sender,
            tx_receiver,
            event_sender,
        };
        let mut engine = EngineProxy::new(Engine::new());
        engine.add_executor(Box::new(socket.clone()));
        Self {
            label: label.into(),
            state: State::Initialization,
            evm,
            socket,
            seeded_poisson,
            engine,
        }
    }

    pub(crate) fn add_strategy(&mut self, strategy: Box<dyn Strategy<ArbiterEvents, ArbiterActions>>) {
        self.engine.add_strategy(strategy);
    }

    // TODO: Run should now run the agents as well as the evm.
    pub(crate) async fn run(&mut self) {
        let mut evm = self.evm.clone();
        let tx_receiver = self.socket.tx_receiver.clone();
        let event_broadcaster = self.socket.event_sender.clone();
        println!("Starting engine");
        let _ = self.engine.run().await;
        println!("Engine has finished running");
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
        let env = Environment::new(TEST_ENV_LABEL.to_string(), 1.0, 1);
        assert_eq!(env.label, TEST_ENV_LABEL);
        assert_eq!(env.state, State::Initialization);
    }

    #[test]
    fn run() {
        let mut environment = Environment::new(TEST_ENV_LABEL.to_string(), 1.0, 1);
        environment.run();
        assert_eq!(environment.state, State::Running);
    }
}
