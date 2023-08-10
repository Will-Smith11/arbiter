#![allow(missing_docs)]

mod interaction;
mod management;
mod strategies;

use std::str::FromStr;
use std::{sync::Arc, task::Poll, time::Duration};

use crate::{
    bindings::{self, arbiter_token::*, liquid_exchange::LiquidExchange},
    environment::{tests::TEST_ENV_LABEL, *},
    manager::*,
    math::*,
    middleware::*,
    strategies::*,
};
use anyhow::{Ok, Result};
use artemis_core::{
    collectors::log_collector::LogCollector,
    engine::Engine,
    types::{Collector, CollectorStream, Executor, Strategy},
};
use ethers::{
    prelude::{EthLogDecode, Middleware, StreamExt},
    types::{Address, Filter, ValueOrArray, U64},
};

pub const TEST_ARG_NAME: &str = "ArbiterToken";
pub const TEST_ARG_SYMBOL: &str = "ARBT";
pub const TEST_ARG_DECIMALS: u8 = 18;
pub const TEST_MINT_AMOUNT: u128 = 69;
pub const TEST_MINT_TO: &str = "0xf7e93cc543d97af6632c9b8864417379dba4bf15";
pub const TEST_APPROVAL_AMOUNT: u128 = 420;
pub const TEST_STRATEGY_NAME: &str = "ArbiterTokenDeployer";
pub const TEST_ARG_NAMEX: &str = "ArbiterTokenX";
pub const TEST_ARG_NAMEY: &str = "ArbiterTokenY";
pub const TEST_ARG_SYMBOLX: &str = "ARBTX";
pub const TEST_ARG_SYMBOLY: &str = "ARBTY";

//TODO: Send a tx before and after pausing the environment.

// async fn deploy_and_start() -> Result<(ArbiterToken<RevmMiddleware>, Environment)> {
//     let mut environment = Environment::new(TEST_ENV_LABEL, 1.0, 1);
//     let agent = Agent::new(TEST_AGENT_NAME);
//     agent.attach_to_environment(&mut environment);
//     environment.run();
//     Ok((
//         ArbiterToken::deploy(
//             environment.agents[0].client.clone(),
//             (
//                 TEST_ARG_NAME.to_string(),
//                 TEST_ARG_SYMBOL.to_string(),
//                 TEST_ARG_DECIMALS,
//             ),
//         )?
//         .send()
//         .await
//         .unwrap(),
//         environment,
//     ))
// }
