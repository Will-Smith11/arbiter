#![allow(missing_docs)]
use std::{sync::Arc, task::Poll, time::Duration};


use futures::stream::Stream;
use anyhow::{Result};
use artemis_core::{types::{Strategy, Collector, CollectorStream}, engine::Engine};
use async_std::{task::sleep, sync::RwLock};


use crate::{bindings::{arbiter_token::*, self}, environment::*, middleware::{*, self}};
use crate::{
    bindings::arbiter_token::*,
    environment::{tests::TEST_ENV_LABEL, *},
    math::*,
    middleware::*,
};

pub const TEST_ARG_NAME: &str = "ArbiterToken";
pub const TEST_STRATEGY_NAME: &str = "ArbiterTokenDeployer";
pub const TEST_ARG_SYMBOL: &str = "ARBT";
pub const TEST_ARG_DECIMALS: u8 = 18;
pub const TEST_MINT_AMOUNT: u128 = 1;
pub const TEST_MINT_TO: &str = "0xf7e93cc543d97af6632c9b8864417379dba4bf15";

#[test]
fn token_mint() -> Result<()> {
    Ok(())
}

#[test]
fn arbiter_math() -> Result<()> {
    Ok(())
}

pub struct StartupStream {
    pub max: u64,
    pub current: async_lock::RwLock<Vec<ArbiterEvents>>,  // Wrap the Vec in RefCell for interior mutability
}

impl StartupStream {
    pub fn new() -> Self {
        let max = u64::MAX;
        Self { max , current: RwLock::new(vec![ArbiterEvents::StartupStream]) }
    }
}

impl Default for StartupStream {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Collector<ArbiterEvents> for StartupStream {
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, ArbiterEvents>> {
        // Clone the data
        let cloned_data = self.current.read().await.clone();

        // Convert the Vec to a Stream
        let stream = futures::stream::iter(cloned_data.into_iter());

        Ok(Box::pin(stream))
    }
}

impl Stream for StartupStream {
    type Item = ArbiterEvents;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        // Try writing without blocking
        if let Some(mut current) = self.current.try_write() {
            if current.is_empty() {
                Poll::Ready(None)
            } else {
                let action = current.pop().unwrap();
                let waker = cx.waker().clone();
                async_std::task::spawn(async move {
                    sleep(Duration::from_millis(500)).await;
                    waker.wake();
                });
                Poll::Pending
            }
        } else {
            Poll::Pending
        }
    }
}


pub(crate) struct DeployStrategy {
    pub name: String,
    pub client: Arc<RevmMiddleware>,
    pub constructor_args: (String, String, u8),
}

impl DeployStrategy {
    pub fn new<S: Into<String>>(name: S, client: Arc<RevmMiddleware>) -> Self {
        Self {
            name: name.into(),
            client,
            constructor_args: (
                TEST_ARG_NAME.to_string(),
                TEST_ARG_SYMBOL.to_string(),
                TEST_ARG_DECIMALS,
            ),
        }
    }
}

#[async_trait::async_trait]
impl Strategy<ArbiterEvents, ArbiterActions> for DeployStrategy {
    async fn sync_state(&mut self) -> Result<()> {
        return Ok(());
    }

    async fn process_event(&mut self, event: ArbiterEvents) -> Vec<ArbiterActions> {
        println!("Got event: {:?} in proccess event", event);
        let client_clone = self.client.clone();
        let constructor_clone = self.constructor_args.clone();
        if let ArbiterEvents::StartupStream = event {
            match ArbiterToken::deploy(client_clone, constructor_clone) {
                Ok(deploy_tx) => {
                    println!("returning deploy tx");
                    return vec![ArbiterActions::Deploy(deploy_tx)];
                }
                Err(e) => {
                    println!("Error deploying contract: {:?}", e);
                    return vec![];
                }            
            }
        }
        else {
            return vec![];
        }
    }
}


async fn deploy() -> Result<()> {
    let engine = Engine::new();

    let environment = &mut Environment::new(TEST_ENV_LABEL, 1.0, 1, engine);

    let client = Arc::new(RevmMiddleware::new(environment));


    let filter = ethers_core::types::Filter::default();
    let start_collector = StartupStream::default();
    let deployer_strategy = DeployStrategy::new(TEST_STRATEGY_NAME, client.clone());

    environment.engine().add_collector(Box::new(start_collector));
    environment.engine().add_strategy(Box::new(deployer_strategy));


    environment.run().await;
    println!("Got here");
    Ok(())
    // Ok(ArbiterToken::deploy(
    //     environment.agents[0].client.clone(),
    //     (
    //         TEST_ARG_NAME.to_string(),
    //         TEST_ARG_SYMBOL.to_string(),
    //         TEST_ARG_DECIMALS,
    //     ),
    // )?
    // .send()
    // .await?)
}

#[tokio::test]
async fn test_deploy() -> Result<()> {
    tracing_subscriber::fmt::init();
    let arbiter_token = deploy().await?;
    // println!("{:?}", arbiter_token);
    // assert_eq!(
    //     arbiter_token.address(),
    //     Address::from_str("0x1a9bb958b1ea4d24475aaa545b25fc2e7eb0871c").unwrap()
    // );
    Ok(())
}

// #[tokio::test]
// async fn call() -> Result<()> {
//     let arbiter_token = deploy().await?;
//     let admin = arbiter_token.admin();
//     let output = admin.call().await?;
//     assert_eq!(
//         output,
//         Address::from_str("0x09e12ce98726acd515b68f87f49dc2e5558f6a72")?
//     );
//     Ok(())
// }

// #[tokio::test]
// async fn transact() -> Result<()> {
//     let arbiter_token = deploy().await?;
//     let mint = arbiter_token.mint(
//         Address::from_str(TEST_MINT_TO).unwrap(),
//         ethers::types::U256::from(TEST_MINT_AMOUNT),
//     );
//     let receipt = mint.send().await?.await?.unwrap();
//     assert_eq!(receipt.logs[0].address, arbiter_token.address());
//     let topics = vec![
//         ethers::core::types::H256::from_str(
//             "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
//         )
//         .unwrap(),
//         ethers::core::types::H256::from_str(
//             "0x0000000000000000000000000000000000000000000000000000000000000000",
//         )
//         .unwrap(),
//         ethers::core::types::H256::from_str(
//             "0x000000000000000000000000f7e93cc543d97af6632c9b8864417379dba4bf15",
//         )
//         .unwrap(),
//     ];
//     assert_eq!(receipt.logs[0].topics, topics);
//     let bytes = hex::decode("0000000000000000000000000000000000000000000000000000000000000001")?;
//     assert_eq!(
//         receipt.logs[0].data,
//         ethers::core::types::Bytes::from(bytes)
//     );
//     println!("logs are: {:#?}", receipt.logs);
//     Ok(())
// }

// #[tokio::test]
// async fn filter_watcher() -> Result<()> {
//     let environment = &mut Environment::new(TEST_ENV_LABEL, 1.0, 1);
//     let revm_middleware = RevmMiddleware::new(&environment);
//     environment.run();
//     let client = revm_middleware.provider().clone();
//     let arbiter_token = deploy().await.unwrap();
//     println!("arbiter token address: {:?}", arbiter_token.address());
//     let filter = arbiter_token.approval_filter().filter;
//     println!("filter address: {:#?}", filter.address);
//     println!("filter in test: {:?}", filter);
//     let mut filter_watcher = client.watch(&Filter::default()).await?;
//     let event = filter_watcher.next();
//     let approval = arbiter_token.approve(client.default_sender().unwrap(), ethers::types::U256::from(100));
//     let thing = approval.send().await?.await?;
//     println!("approval sent");
//     println!("thing: {:?}", thing);
//     let event = event.await;
//     println!("{:?}", event);
//     Ok(())

//     // TODO: Test that we can filter out approvals and NOT transfers (or something like this)
// }

// // This test has two parts
// // 1 check that the expected number of transactions per block is the actual number of transactions per block.
// // 2 check the block number is incremented after the expected number of transactions is reached.
// // #[tokio::test]
// // async fn transaction_loop() -> Result<()> {
// //     let mut env = Environment::new(TEST_ENV_LABEL, 2.0, 1);

// //     let mut dist = env.seeded_poisson.clone();
// //     let expected_tx_per_block = dist.sample();

// //     println!("expected_tx_per_block: {}", expected_tx_per_block);
// //     // tx_0 is the transaction that creates the token contract
// //     let arbiter_token = deploy().await?;

// //     for index in 1..expected_tx_per_block {
// //         println!("index: {}", index);
// //         let tx = arbiter_token
// //             .mint(agent.client.default_sender().unwrap(), 1000u64.into())
// //             .send()
// //             .await
// //             .unwrap()
// //             .await
// //             .unwrap()
// //             .unwrap();

// //         // minus 1 from deploy tx
// //         if index < expected_tx_per_block - 1 {
// //             let block_number = tx.block_number.unwrap();
// //             println!("block_number: {}", block_number);
// //             assert_eq!(block_number, U64::from(0));
// //         } else {
// //             let block_number = tx.block_number.unwrap();
// //             println!("block_number: {}", block_number);
// //             assert_eq!(block_number, U64::from(1));
// //         }
// //     }
// //     Ok(())
// // }
