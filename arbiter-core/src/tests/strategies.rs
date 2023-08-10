use ethers_core::types::U256;

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
    pub client: Arc<RevmMiddleware>,
    pub arbiter_token: ArbiterToken<RevmMiddleware>,
    pub mint_params: (Address, U256),
}

impl TestStrategy {
    pub fn new<S: Into<String>>(name: S, client: Arc<RevmMiddleware>, arbiter_token: ArbiterToken<RevmMiddleware>) -> Self {
        Self {
            name: name.into(),
            client,
            arbiter_token,
            mint_params: (client.default_sender().unwrap(), U256::from(1000)),
        }
    }
}

#[async_trait::async_trait]
impl Strategy<ArbiterEvents, ArbiterActions> for TestStrategy {
    async fn sync_state(&mut self) -> Result<()> {
        return Ok(());
    }

    /// get event and make actions based on them
    async fn process_event(&mut self, event: ArbiterEvents) -> Vec<ArbiterActions> {
        match event {
            ArbiterEvents::Mint => {
                let tx1 = self.arbiter_token.mint(self.mint_params.0, self.mint_params.1);
                vec![ArbiterActions::ContractCall(tx1)]
            }
            _ => vec![],
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
