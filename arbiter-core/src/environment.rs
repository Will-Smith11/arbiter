#![warn(missing_docs)]
#![warn(unsafe_code)]

use artemis_core::engine::Engine;
use ethers::{
    core::types::{Log, U64},
    prelude::ContractDeploymentTx, types::Transaction,
};
use ethers_core::types::transaction::eip2718::TypedTransaction;
use revm::{
    db::{CacheDB, EmptyDB},
    primitives::{ExecutionResult, TxEnv, U256},
    EVM,
};
// use artemis_core::types::Strategy;
use anyhow::Result;
// TODO: Add logging especially inside of the run function. This will be necessary for pausing and debugging.
// TODO: Add custom errors.

use crate::{
    bindings::arbiter_token::ArbiterToken, math::stochastic_process::SeededPoisson,
    middleware::RevmMiddleware,
};
use crossbeam_channel::{unbounded, Receiver, Sender};
use tokio::task::JoinSet;

use std::{
    fmt::Debug,
    sync::{Arc, Condvar, Mutex},
    thread,
};

#[derive(Debug, Clone)]
pub struct RevmResult {
    /// The result of the execution.
    pub result: ExecutionResult,
    /// The block number of the execution.
    pub block_number: U64,
}

pub(crate) type ToTransact = bool;
pub(crate) type ResultSender = Sender<RevmResult>;
pub(crate) type ResultReceiver = Receiver<RevmResult>;
pub(crate) type TxSender = Sender<(ToTransact, TxEnv, ResultSender)>;
pub(crate) type TxReceiver = Receiver<(ToTransact, TxEnv, ResultSender)>;

#[atomic_enum::atomic_enum]
#[derive(Eq, PartialEq)]
pub enum State {
    Initialization,
    Running,
    Paused,
    Stopped,
}

#[derive(Debug, Clone)]
pub(crate) struct Socket {
    pub(crate) tx_sender: TxSender,
    pub(crate) tx_receiver: TxReceiver,
    pub(crate) event_broadcaster: Arc<Mutex<EventBroadcaster>>,
}

pub struct Environment {
    /// The name of the environment.
    pub label: String,
    pub(crate) state: Arc<AtomicState>,
    pub(crate) evm: EVM<CacheDB<EmptyDB>>,
    pub(crate) seeded_poisson: SeededPoisson,
    pub(crate) engine: Option<Engine<ArbiterEvents, ArbiterActions>>,
    pub(crate) socket: Socket,
    pub(crate) pausevar: Arc<(Mutex<()>, Condvar)>,
}



impl Environment {
    /// Creates a new [`Environment`] with the given label.
    pub(crate) fn new<S: Into<String>>(
        label: S,
        block_rate: f64,
        seed: u64,
        mut engine: Engine<ArbiterEvents, ArbiterActions>,
    ) -> Self {
        let mut evm = EVM::new();
        let db = CacheDB::new(EmptyDB {});
        evm.database(db);
        let seeded_poisson = SeededPoisson::new(block_rate, seed);
        evm.env.cfg.limit_contract_code_size = Some(0x100000); // This is a large contract size limit, beware!
        evm.env.block.gas_limit = U256::MAX;

        let (tx_sender, tx_receiver) = unbounded();

        let socket = Socket {
            tx_sender,
            tx_receiver,
            event_broadcaster: Arc::new(Mutex::new(EventBroadcaster::new())),
        };

        Self {
            label: label.into(),
            state: Arc::new(AtomicState::new(State::Initialization)),
            evm,
            socket,
            seeded_poisson,
            engine: Some(engine),
            pausevar: Arc::new((Mutex::new(()), Condvar::new())),
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
            } else {
                panic!("Engine failed to start");
            }
        } else {
            panic!("Engine is missing");
        }
    }

    // TODO: Run should now run the agents as well as the evm.
    pub(crate) async fn run(&mut self) -> std::thread::JoinHandle<()> {
        let mut evm = self.evm.clone();
        let tx_receiver = self.socket.tx_receiver.clone();
        let event_broadcaster = self.socket.event_broadcaster.clone();

        let mut seeded_poisson = self.seeded_poisson.clone();

        // self.state = State::Running;
        let mut counter: usize = 0;
        self.state
            .store(State::Running, std::sync::atomic::Ordering::Relaxed);
        let state = Arc::clone(&self.state);
        let pausevar = Arc::clone(&self.pausevar);

        println!("Starting environment");
        let handle = thread::spawn(move || {
            let mut expected_events_per_block = seeded_poisson.sample();
            loop {
                match state.load(std::sync::atomic::Ordering::Relaxed) {
                    State::Stopped => break,
                    State::Paused => {
                        let (lock, cvar) = &*pausevar;
                        let mut guard = lock.lock().unwrap();
                        while state.load(std::sync::atomic::Ordering::Relaxed) == State::Paused {
                            guard = cvar.wait(guard).unwrap();
                        }
                    }
                    State::Running => {
                        if let Ok((to_transact, tx, sender)) = tx_receiver.recv() {
                            if counter == expected_events_per_block {
                                counter = 0;
                                evm.env.block.number += U256::from(1);
                                expected_events_per_block = seeded_poisson.sample();
                            }

                            evm.env.tx = tx;
                            if to_transact {
                                let execution_result = match evm.transact_commit() {
                                    Ok(val) => val,
                                    // URGENT: change this to a custom error
                                    Err(_) => panic!("failed"),
                                };
                                let event_broadcaster = event_broadcaster.lock().unwrap();
                                event_broadcaster.broadcast(
                                    crate::middleware::revm_logs_to_ethers_logs(
                                        execution_result.logs(),
                                    ),
                                );
                                let revm_result = RevmResult {
                                    result: execution_result,
                                    block_number: convert_uint_to_u64(evm.env.block.number)
                                        .unwrap(),
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
                                    block_number: convert_uint_to_u64(evm.env.block.number)
                                        .unwrap(),
                                };
                                sender.send(result_and_block).unwrap();
                            }
                        }
                    }
                    State::Initialization => {
                        panic!("Environment is in an invalid state: Initialization. This should not be possible.");
                    }
                }
            }
        });
        self.start_engine().await;
        handle
    }
}

#[derive(Clone, Debug)]
pub struct EventBroadcaster(Vec<crossbeam_channel::Sender<Vec<Log>>>);

impl EventBroadcaster {
    pub(crate) fn new() -> Self {
        Self(vec![])
    }

    pub(crate) fn add_sender(&mut self, sender: crossbeam_channel::Sender<Vec<Log>>) {
        self.0.push(sender);
    }

    pub(crate) fn broadcast(&self, logs: Vec<Log>) {
        for sender in &self.0 {
            sender.send(logs.clone()).unwrap();
        }
    }
}

/// The actions that the [`Environment`] can take
#[derive(Clone, Debug)]
pub enum ArbiterActions {
    SendTx(Transaction, ResultSender),
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

/// Convert a U256 to a U64, discarding the higher bits if the number is larger than 2^64
/// # Arguments
/// * `input` - The U256 to convert.
/// # Returns
/// * `Ok(U64)` - The converted U64.
/// Used for block number which is a U64.
#[inline]
pub fn convert_uint_to_u64(input: U256) -> Result<U64, &'static str> {
    let as_str = input.to_string();
    match as_str.parse::<u64>() {
        Ok(val) => Ok(val.into()),
        Err(_) => Err("U256 value is too large to fit into u64"),
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
        // assert_eq!(env.state, State::Initialization);
    }

    #[test]
    fn run() {
        let engine = Engine::new();
        let mut environment = Environment::new(TEST_ENV_LABEL.to_string(), 1.0, 1, engine);
        environment.run();
        let state = environment.state.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(state, State::Running);
    }

    #[test]
    fn test_conversion() {
        // Test with a value that fits in u64.
        let input = U256::from(10000);
        assert_eq!(convert_uint_to_u64(input).unwrap(), U64::from(10000));

        // Test with a value that is exactly at the limit of u64.
        let input = U256::from(u64::MAX);
        assert_eq!(convert_uint_to_u64(input).unwrap(), U64::from(u64::MAX));

        // Test with a value that exceeds the limit of u64.
        let input = U256::from(u64::MAX) + U256::from(1);
        assert!(convert_uint_to_u64(input).is_err());
    }
}


