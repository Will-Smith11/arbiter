use async_std::channel;
use ethers_core::types::U256;
use tracing_subscriber::{filter, prelude::*};
use crate::bindings::counter::Counter;
use tracing::{error, info, Level};

use super::*;

/// This is the arbitraguer strategy.
pub struct ArbitraguerStrategy {
    client: Arc<RevmMiddleware>,
    /// I am not sure the best way to make the contracts generic
    // What i would like to do is have the contstructor take in two exchange contracts and then we don't neeed the client
    // There might be a way to just use the client and maybe the exchange addresses, but I am not sure if it will be clean.
    // exchanges: (LiquidExchange<RevmMiddleware>, <RevmMiddleware>),
    exchange_prices: (f64, f64),
    event_sender: crossbeam_channel::Sender<ArbiterEvents>,
}

impl ArbitraguerStrategy {
    /// Constructor for the [`ArbitraguerStrategy`].
    pub fn new(
        client: Arc<RevmMiddleware>,
        event_sender: crossbeam_channel::Sender<ArbiterEvents>,
    ) -> Self {
        Self {
            client,
            exchange_prices: (0.0, 0.0),
            event_sender,
        }
    }

    /// This function builds two function calls to execute an arbitrage on the liquid exchange and the external market
    pub fn build_arbitrage_call(
        &self,
        _arb_size: usize,
    ) -> (ContractFunctionCall, ContractFunctionCall) {
        // one call for each leg
        todo!()
    }
}

impl Arbitraguer for ArbitraguerStrategy {
    /// check bounds, if in bounds return the size of the arbitrage
    /// else return None
    fn detect_arbitrage(&self, _new_price: f64) -> Option<usize> {
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
                if let Some(arb_size) = self.detect_arbitrage(new_price) {
                    let (tx1, tx2) = self.build_arbitrage_call(arb_size);
                    vec![
                        ArbiterActions::ContractCall(tx1),
                        ArbiterActions::ContractCall(tx2),
                    ]
                } else {
                    let _ = self.event_sender.send(ArbiterEvents::UpdatePrice(true));
                    vec![]
                }
            }
            _ => vec![],
        }
    }
}

pub struct TestStrategy {
    pub name: String,
    pub counter: Counter<RevmMiddleware>,
    pub count: usize,
    pub sender: crossbeam_channel::Sender<ArbiterEvents>
}

impl TestStrategy {
    pub fn new<S: Into<String>>(name: S, client: Arc<RevmMiddleware>, counter: Counter<RevmMiddleware>, sender: crossbeam_channel::Sender<ArbiterEvents>) -> Self {
        Self {
            name: name.into(),
            counter,
            count: 0,
            sender,
        }
    }
}

#[async_trait::async_trait]
impl Strategy<ArbiterEvents, ArbiterActions> for TestStrategy {
    async fn sync_state(&mut self) -> Result<()> {
        println!("Strategy: Syncing state");
        let _ = self.sender.send(ArbiterEvents::Increment);
        Ok(())
    }

    /// get event and make actions based on them
    async fn process_event(&mut self, event: ArbiterEvents) -> Vec<ArbiterActions> {
        match event {
            ArbiterEvents::Increment => {
                let tx1 = self.counter.increment();
                if self.count <= 5 {
                    self.count += 1;
                    let _ = self.sender.send(ArbiterEvents::Increment);
                    vec![ArbiterActions::ContractCall(tx1)]
                } else {
                    let _ = self.sender.send(ArbiterEvents::SetNumber(U256::from(0)));
                    vec![]
                }
            }
            ArbiterEvents::SetNumber(num) => {
                let tx1 = self.counter.set_number(num);
                vec![ArbiterActions::ContractCall(tx1)]
            }
            _ => vec![],
        }
    }
}


/// I do not believe it makes sense to have a strategy for deploying a contract.
/// My thoughts are to build a more closed system to test this with.
/// The idea is to deploy a counter contract
/// Then the idea is to have a strategy that will continually update the counter and communicate an increment event to the collector
/// The collector will then collect the increment events and send them to the strategy
/// The strategy will then take in these events and emit increment actions to the executor via function calls
/// the executor will then take these update actions and update the count by sending the calls by to the client

async fn init() -> Result<()> {


    let mut manager = Manager::new();

    manager.add_environment(TEST_ENV_LABEL, 1.0, 1, Engine::new())?;
    println!("Added environment");
    // let environment = manager.environments.get_mut(TEST_ENV_LABEL).unwrap();
    println!("Made client");

    // let client = Arc::new(RevmMiddleware::new(environment));

    let client = {
        let environment = manager.environments.get_mut(TEST_ENV_LABEL).unwrap();
        Arc::new(RevmMiddleware::new(environment))
    };

    let _ = manager.start_environment(TEST_ENV_LABEL).await;

    let environment = manager.environments.get_mut(TEST_ENV_LABEL).unwrap();
    // Deploy a counter
    let counter = Counter::deploy(client.clone(), ())?.send().await?;
    println!("Counter Address: {:#?}", counter.address());

    // make a channel between the collector and the strategy
    let (send, rec) = crossbeam_channel::unbounded(); 
    // make strategy, collector, and executor
    let strategy = TestStrategy::new("test", client.clone(), counter, send);
    let collector = AgentCollector::new(rec);
    let executor = RevmExecutor::new(client.clone());

    println!("Made strategy, collector, and executor");
    environment.engine().add_collector(Box::new(collector));
    environment.engine().add_strategy(Box::new(strategy));
    environment.engine().add_executor(Box::new(executor));
    println!("Added strategy, collector, and executor");

    let mut set = environment.start_engine().await;

    while let Some(res) = set.join_next().await {
        info!("res: {:?}", res);
    }

    Ok(())
}


#[tokio::test]
async fn test_strategy() -> Result<()> {
    tracing_subscriber::fmt::init();
    init().await?;
    Ok(())

}
