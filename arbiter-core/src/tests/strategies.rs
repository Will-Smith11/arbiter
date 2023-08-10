use super::*;

// #[test]
// fn attach_agent() {
//     let environment = &mut Environment::new(TEST_ENV_LABEL, 1.0, 1);
//     let agent = Agent::new(TEST_AGENT_NAME);
//     agent.attach_to_environment(environment);
//     assert_eq!(environment.agents[0].name, TEST_AGENT_NAME);
// }

// #[test]
// fn simulation_agent_wallet() {
//     let environment = &mut Environment::new(TEST_ENV_LABEL, 1.0, 1);
//     let agent = Agent::new(TEST_AGENT_NAME);
//     agent.attach_to_environment(environment);
//     assert_eq!(
//         environment.agents[0].client.default_sender().unwrap(),
//         Address::from_str("0x09e12ce98726acd515b68f87f49dc2e5558f6a72").unwrap()
//     );
// }

// #[test]
// fn multiple_agent_addresses() {
//     let environment = &mut Environment::new(TEST_ENV_LABEL, 1.0, 1);
//     let agent = Agent::new(TEST_AGENT_NAME);
//     agent.attach_to_environment(environment);
//     let agent2 = Agent::new(format!("new_{}", TEST_AGENT_NAME));
//     agent2.attach_to_environment(environment);
//     assert_ne!(
//         environment.agents[0].client.default_sender(),
//         environment.agents[1].client.default_sender()
//     );
// }

// // TODO: Test to see that we prvent agents with the same name from being added.
// #[test]
// fn agent_name_collision() {
//     todo!();
// }

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
                    let action = ArbiterActions::Mint(
                        TEST_MINT_AMOUNT,
                        self.arbiter_token_x.clone().unwrap(),
                        self.client.clone(),
                    );
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
    let mut manager = Manager::new();
    manager.add_environment(TEST_ENV_LABEL, 1.0, 1, Engine::new());
    let environment = manager.environments.get_mut(TEST_ENV_LABEL).unwrap();
    let client = Arc::new(RevmMiddleware::new(environment));
    environment
        .engine()
        .add_collector(Box::new(RevmCollector::new(client.clone())));
    environment
        .engine()
        .add_executor(Box::new(RevmExecutor::new(client.clone())));
    environment
        .engine()
        .add_strategy(Box::new(TestStrategy::new(
            TEST_STRATEGY_NAME,
            client.clone(),
        )));
    manager.start_environment(TEST_ENV_LABEL).await;
    // deploy token 1
    let constructor_args = (
        TEST_ARG_NAMEX.to_string(),
        TEST_ARG_SYMBOLX.to_string(),
        TEST_ARG_DECIMALS,
    );

    let arbiter_token_x = ArbiterToken::deploy(client.clone(), constructor_args)
        .unwrap()
        .send()
        .await?;
    println!("arbiter token: {:?}", arbiter_token_x);

    // deploy token 2
    let constructor_args = (
        TEST_ARG_NAMEY.to_string(),
        TEST_ARG_SYMBOLY.to_string(),
        TEST_ARG_DECIMALS,
    );
    let arbiter_token_y = ArbiterToken::deploy(client.clone(), constructor_args)
        .unwrap()
        .send()
        .await?;
    println!("arbiter token: {:?}", arbiter_token_y);

    // deploy liquid exchange
    let constructor_args = (
        arbiter_token_x.address(),
        arbiter_token_y.address(),
        TEST_MINT_AMOUNT,
    );
    let liquid_exchange = LiquidExchange::deploy(client.clone(), constructor_args)
        .unwrap()
        .send()
        .await?;
    println!("liquid exchange: {:?}", liquid_exchange);

    let filter = Filter {
        address: Some(ethers::types::ValueOrArray::Array(vec![
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
