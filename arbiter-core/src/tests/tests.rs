#![allow(missing_docs)]
use std::{sync::Arc, task::Poll, time::Duration};


use futures::stream::unfold; // Use unfold from the futures crate
use ethers::{types::Log, providers::Middleware};
use ethers_core::types::Filter;
use futures::stream::Stream;
use anyhow::{Result};
use artemis_core::{types::{Strategy, Collector, CollectorStream, Executor}, engine::Engine, collectors::log_collector::LogCollector};
use async_std::{task::sleep, sync::RwLock};
use tracing_subscriber::filter;


use crate::{bindings::{arbiter_token::*, self, liquid_exchange::LiquidExchange}, environment::*, middleware::{*, self}};
use crate::{
    bindings::arbiter_token::*,
    environment::{tests::TEST_ENV_LABEL, *},
    math::*,
    middleware::*,
};

pub const TEST_ARG_NAMEX: &str = "ArbiterTokenX";
pub const TEST_ARG_NAMEY: &str = "ArbiterTokenY";
pub const TEST_STRATEGY_NAME: &str = "ArbiterTokenDeployer";
pub const TEST_ARG_SYMBOLX: &str = "ARBTX";
pub const TEST_ARG_SYMBOLY: &str = "ARBTY";
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


#[async_trait::async_trait]
impl Strategy<ArbiterEvents, ArbiterActions> for TestStrategy {
    async fn sync_state(&mut self) -> Result<()> {
        return Ok(());
    }

    /// get event and make actions based on them
    async fn process_event(&mut self, event: ArbiterEvents) -> Vec<ArbiterActions> {
        if let ArbiterEvents::Event(logs) = event {
            let mut actions = Vec::new();
            // always mint on a valid event just for testing
            for log in logs {
                println!("log: {:?}", log);
                if true {
                    let action = ArbiterActions::Mint(TEST_MINT_AMOUNT, self.arbiter_token_x.clone().unwrap(), self.client.clone());
                    actions.push(action);
                }
            }
            return actions;
        } else {
            return Vec::new();
        }
    }
}


pub(crate) struct TestStrategy {
    pub name: String,
    pub client: Arc<RevmMiddleware>,
    pub arbiter_token_x: Option<ArbiterToken<RevmMiddleware>>,

}

impl TestStrategy {
    pub fn new<S: Into<String>>(name: S, client: Arc<RevmMiddleware>) -> Self {
        Self {
            name: name.into(),
            client,
            arbiter_token_x: None,
        }
    }
}


/// Notes: Currently the deploy strategy works but then breaks when there is no more events comming from the collector.
/// I do not believe it makes sense to have a strategy for deploying a contract.
/// My thoughts are to build a more closed system to test this with. 
/// The idea is to 1) deploye a liquid exchange contract and two arbiter tokens
/// Then the idea is to have a strategy that will continually update the price of the liquid exchange contract
/// The collector will then collect the price updates and send them to the strategy
/// The strategy will then take in these price update events and emit price update actions
/// the executor will then take these price update actions and update the price of the liquid exchange contract by sending them to the client

async fn deploy() -> Result<()> {
    let engine = Engine::new();

    let environment = &mut Environment::new(TEST_ENV_LABEL, 1.0, 1, engine);

    // note this is admin client
    let client = Arc::new(RevmMiddleware::new(environment));

    // deploy token 1
    let constructor_args = (
        TEST_ARG_NAMEX.to_string(),
        TEST_ARG_SYMBOLX.to_string(),
        TEST_ARG_DECIMALS,
    );

    let arbiter_token_x = ArbiterToken::deploy(client.clone(), constructor_args).unwrap().send().await?;
    println!("arbiter token: {:?}", arbiter_token_x);


    // deploy token 2
    let constructor_args = (
        TEST_ARG_NAMEY.to_string(),
        TEST_ARG_SYMBOLY.to_string(),
        TEST_ARG_DECIMALS,
    );
    let arbiter_token_y = ArbiterToken::deploy(client.clone(), constructor_args).unwrap().send().await?;
    println!("arbiter token: {:?}", arbiter_token_y);


    // deploy liquid exchange
    let constructor_args = (
        arbiter_token_x.address(),
        arbiter_token_y.address(),
        TEST_MINT_AMOUNT,
    );
    let liquid_exchange = LiquidExchange::deploy(client.clone(), constructor_args).unwrap().send().await?;
    println!("liquid exchange: {:?}", liquid_exchange);


    let filter = Filter {
        address: Some(ethers_core::types::ValueOrArray::Array(vec![
            liquid_exchange.address()
        ])),
        topics: [None, None, None, None], // None for all topics
        ..Default::default()
    };

    // i think we should use a log collector from artemis core for most everything we want to do, although we can't untill we have pub sub
    // let collector = artemis_core::collectors::log_collector::LogCollector::new(client.clone(), filter);
    let mut test_strategy = TestStrategy::new(TEST_STRATEGY_NAME, client.clone());
    test_strategy.arbiter_token_x = Some(arbiter_token_x);

    // environment.engine().add_collector(Box::new(collector));
    // environment.engine().add_strategy(Box::new(test_strategy));
    // let mut join_set = environment.start_engine().await;
    // environment.run().await;
    // while let Some(res) = join_set.join_next().await {
    //     println!("res: {:?}", res);
    // }
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
    // tracing_subscriber::fmt::init();
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
