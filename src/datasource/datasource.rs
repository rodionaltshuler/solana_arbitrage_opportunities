use async_trait::async_trait;
use futures::stream::BoxStream;
use crate::datasource::quote::{Venue, Instrument, QuoteUpdate};

#[async_trait]
pub trait DataSource: Send + Sync {
    fn venue(&self) -> &Venue;

    async fn subscribe_best_quotes(&self, instrument: Instrument) -> anyhow::Result<BoxStream<'static, QuoteUpdate>>;

}
