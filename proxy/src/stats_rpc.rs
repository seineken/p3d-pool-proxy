use jsonrpsee::core::{async_trait, SubscriptionResult};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::{PendingSubscriptionSink, SubscriptionMessage};

#[rpc(server, client)]
pub trait StatsRpc {
    /// Subscription the Pool stats and delivers to the UI dashboard.
    #[subscription(name = "get_general" => "override", item = Vec<String>)]
    async fn get_general(&self) -> SubscriptionResult;

    /// Subscription the Pool stats and delivers to the UI dashboard.
    #[subscription(name = "get_details" => "override", item = Vec<String>)]
    async fn get_details(&self, member_id: String) -> SubscriptionResult;
}

pub struct StatsRpcServerImpl;

impl StatsRpcServerImpl {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatsRpcServer for StatsRpcServerImpl {
    async fn get_general(&self, pending: PendingSubscriptionSink) -> SubscriptionResult {
        let sink = pending.accept().await?;
        let msg: SubscriptionMessage = SubscriptionMessage::from_json(&vec![[0; 32]])?;
        sink.send(msg).await?;

        Ok(())
    }
    async fn get_details(
        &self,
        pending: PendingSubscriptionSink,
        _member_id: String,
    ) -> SubscriptionResult {
        let sink = pending.accept().await?;
        let msg = SubscriptionMessage::from_json(&vec![[0; 32]])?;
        sink.send(msg).await?;

        Ok(())
    }
}
