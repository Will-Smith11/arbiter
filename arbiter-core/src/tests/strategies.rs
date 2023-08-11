use async_std::channel;
use crossbeam_channel::unbounded;
use ethers_core::types::U256;

use super::*;

pub struct TestStrategy {
    pub name: String,
    pub sender: crossbeam_channel::Sender<SimulationEvents>,
}

impl TestStrategy {
    pub fn new<S: Into<String>>(
        name: S,
        sender: crossbeam_channel::Sender<SimulationEvents>,
    ) -> Self {
        Self {
            name: name.into(),
            sender,
        }
    }
}

#[async_trait::async_trait]
impl Strategy<SimulationEvents, SimulationActions> for TestStrategy {
    async fn sync_state(&mut self) -> Result<()> {
        println!("syncing state");
        self.sender
            .send(SimulationEvents::Message(
                "hello from the sync state".to_string(),
            ))
            .unwrap();

        return Ok(());
    }

    /// get event and make actions based on them
    async fn process_event(&mut self, event: SimulationEvents) -> Vec<SimulationActions> {
        println!("processing event: {:?}", event);
        match event {
            SimulationEvents::Message(string) => {
                let reply = format!("{}: {}", self.name, string);
                let action = SimulationActions::Reply(reply);
                vec![action]
            }
            _ => vec![],
        }
    }
}

/// Notes: Currently the deploy strategy works but then breaks when there is no more events comming from the collector.
/// I do not believe it makes sense to have a strategy for deploying a contract.
/// My thoughts are to build a more closed system to test this with.
/// The idea is to deploy a counter contract
/// Then the idea is to have a strategy that will continually update the counter and communicate an increment event to the collector
/// The collector will then collect the increment events and send them to the strategy
/// The strategy will then take in these events and emit increment actions to the executor via function calls
/// the executor will then take these update actions and update the count by sending the calls by to the client

async fn init() -> Result<()> {
    let mut manager = Manager::new();

    let _ = manager.add_environment(TEST_ENV_LABEL, 1.0, 1, Engine::new());
    let client = Arc::new(RevmMiddleware::new(
        manager.environments.get(TEST_ENV_LABEL).unwrap(),
    ));

    // make a channel to use for communication of events
    let (sender, receiver) = crossbeam_channel::unbounded();

    let strategy = TestStrategy::new("test", sender.clone());
    let collector = SimulationCollector::new(receiver);
    let executor = SimulationExecutor::new(client.clone());

    // TODO: Giving the manager a way to add all of these and control the engine would be better.
    manager
        .environments
        .get_mut(TEST_ENV_LABEL)
        .unwrap()
        .engine()
        .add_collector(Box::new(collector));
    manager
        .environments
        .get_mut(TEST_ENV_LABEL)
        .unwrap()
        .engine()
        .add_strategy(Box::new(strategy));
    manager
        .environments
        .get_mut(TEST_ENV_LABEL)
        .unwrap()
        .engine()
        .add_executor(Box::new(executor));

    // 1. start env in initialization
    // 2. start engine
    // 3. set is threads that the strategy collectors n executors are in
    sender
        .send(SimulationEvents::Message(
            "hello to the strategy".to_string(),
        ))
        .unwrap();
    manager.start_environment(TEST_ENV_LABEL).await?;
    println!("Here");

    Ok(())
}

#[tokio::test]
async fn test_strategy() -> Result<()> {
    init().await?;
    Ok(())
}
