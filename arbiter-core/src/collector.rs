use std::sync::Arc;
use artemis_core::types::Collector;
use crate::{middleware::RevmMiddleware, environment::ArbiterEvents};

/// Idea here is to have a collector that can be used to collect events from the revm middleware.
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