#![warn(missing_docs)]
#![warn(unsafe_code)]

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use artemis_core::{
    engine::{self, Engine},
    types::Strategy,
};

// TODO: Add any necessary logging.
// TODO: Add any necessary custom errors.

use crate::environment::{ArbiterActions, ArbiterEvents, AtomicState, Environment, State};
use std::sync::Arc;

#[derive(Default)]
pub struct Manager {
    pub environments: HashMap<String, Environment>,
    handles_and_states: HashMap<String, (std::thread::JoinHandle<()>, Arc<AtomicState>)>,
}

impl Manager {
    pub fn new() -> Self {
        Self {
            environments: HashMap::new(),
            handles_and_states: HashMap::new(),
        }
    }

    pub fn add_environment<S: Into<String> + Clone>(
        &mut self,
        environment_label: S,
        block_rate: f64,
        seed: u64,
        engine: Engine<ArbiterEvents, ArbiterActions>,
    ) -> Result<()> {
        if self
            .environments
            .get(&environment_label.clone().into())
            .is_some()
        {
            return Err(anyhow!("Environment already exists."));
        }
        self.environments.insert(
            environment_label.clone().into(),
            Environment::new(environment_label, block_rate, seed, engine),
        );
        Ok(())
    }

    pub async fn start_environment<S: Into<String> + Clone>(
        &mut self,
        environment_label: S,
    ) -> Result<()> {
        match self.environments.get_mut(&environment_label.clone().into()) {
            Some(environment) => match environment.state.load(std::sync::atomic::Ordering::Relaxed)
            {
                State::Initialization => {
                    let handle = environment.run().await;
                    self.handles_and_states.insert(
                        environment_label.into(),
                        (handle, environment.state.clone()),
                    );
                    Ok(())
                }
                State::Paused => {
                    environment
                        .state
                        .store(State::Running, std::sync::atomic::Ordering::Relaxed);
                    let (lock, pausevar) = &*environment.pausevar;
                    let _guard = lock.lock().unwrap();
                    pausevar.notify_all();
                    Ok(())
                }
                State::Running => Err(anyhow!("Environment is already running.")),
                State::Stopped => Err(anyhow!("Environment is stopped and cannot be restarted.")),
            },
            None => Err(anyhow!("Environment does not exist.")),
        }
    }

    pub fn pause_environment<S: Into<String> + Clone>(
        &mut self,
        environment_label: S,
    ) -> Result<()> {
        match self.environments.get_mut(&environment_label.clone().into()) {
            Some(environment) => match environment.state.load(std::sync::atomic::Ordering::Relaxed)
            {
                State::Initialization => Err(anyhow!("Environment is not running.")),
                State::Running => {
                    environment
                        .state
                        .store(State::Paused, std::sync::atomic::Ordering::Relaxed);
                    println!("Changed state to paused.");
                    Ok(())
                }
                State::Paused => Err(anyhow!("Environment is already paused.")),
                State::Stopped => Err(anyhow!("Environment is stopped and cannot be paused.")),
            },
            None => Err(anyhow!("Environment does not exist.")),
        }
    }

    pub fn stop_environment<S: Into<String> + Clone>(
        &mut self,
        environment_label: S,
    ) -> Result<()> {
        match self.environments.get_mut(&environment_label.clone().into()) {
            Some(environment) => match environment.state.load(std::sync::atomic::Ordering::Relaxed)
            {
                State::Initialization => Err(anyhow!("Environment is not running.")),
                State::Running => {
                    let (handle, state) = self
                        .handles_and_states
                        .remove(&environment_label.into())
                        .unwrap();
                    state.store(State::Stopped, std::sync::atomic::Ordering::Relaxed);
                    handle.join().unwrap();
                    Ok(())
                }
                State::Paused => {
                    // TODO: GIVE THE RESTART LOGIC HERE TOO
                    let (handle, state) = self
                        .handles_and_states
                        .remove(&environment_label.into())
                        .unwrap();
                    state.store(State::Stopped, std::sync::atomic::Ordering::Relaxed);
                    handle.join().unwrap();
                    Ok(())
                }
                State::Stopped => Err(anyhow!("Environment is already stopped.")),
            },
            None => Err(anyhow!("Environment does not exist.")),
        }
    }
}

// #[cfg(test)]
// pub(crate) mod tests {

//     use super::*;

//     #[test]
//     fn new_manager() {
//         let manager = Manager::new();
//         assert!(manager.environments.is_empty());
//     }

//     #[test]
//     fn add_environment() {
//         let mut manager = Manager::new();
//         let engine = Engine::new();

//         let label = "test".to_string();
//         manager
//             .add_environment(label.clone(), 1.0, 1, engine)
//             .unwrap();
//         assert!(manager.environments.contains_key(&label));
//     }

//     #[test]
//     fn run_environment() {
//         let engine = Engine::new();
//         let mut manager = Manager::new();
//         let label = "test".to_string();
//         manager
//             .add_environment(label.clone(), 1.0, 1, engine)
//             .unwrap();
//         manager.start_environment(label.clone()).unwrap();
//         // assert_eq!(
//         //     manager.environments.get(&label).unwrap().state,
//         //     State::Running
//         // );
//     }
// }
