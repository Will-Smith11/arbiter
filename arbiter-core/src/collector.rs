use std::sync::Arc;
use artemis_core::types::Collector;
use crate::{middleware::RevmMiddleware, environment::ArbiterEvents};


struct RevmCollector {
    client: Arc<RevmMiddleware>,
}

impl RevmCollector {
    pub fn new(client: Arc<RevmMiddleware>) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl Collector<ArbiterEvents> for RevmCollector {
    async fn get_event_stream(&self) -> anyhow::Result<artemis_core::types::CollectorStream<'_, ArbiterEvents>> {
        todo!()
    }
}