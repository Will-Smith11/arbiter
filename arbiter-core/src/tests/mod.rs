#![allow(missing_docs)]

mod interaction;
mod management;
mod strategies;

use std::{sync::Arc, task::Poll, time::Duration};
use std::str::FromStr;

use anyhow::{Ok, Result};
use ethers::{
    prelude::{EthLogDecode, Middleware, StreamExt},
    types::{Address, Filter, ValueOrArray, U64},
};
use artemis_core::{types::{Strategy, Collector, CollectorStream, Executor}, engine::Engine, collectors::log_collector::LogCollector};
use crate::{
    bindings::{arbiter_token::*, self, liquid_exchange::LiquidExchange},
    environment::{tests::TEST_ENV_LABEL, *},
    manager::*,
    math::*,
    middleware::*,
    strategies::*,
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
