use async_trait::async_trait;
use futures::stream::BoxStream;
use crate::datasource::quote::{Exchange, Instrument, QuoteUpdate};

#[async_trait]
pub trait DataSource: Send + Sync {
    fn exchange(&self) -> &Exchange;

    async fn subscribe_best_quotes(&self, instrument: Instrument) -> anyhow::Result<BoxStream<'static, QuoteUpdate>>;

}
