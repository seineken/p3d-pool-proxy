use std::sync::Arc;

use crate::solo_handler::SoloAppContex;
use jsonrpsee::core::{async_trait, RpcResult};
use jsonrpsee::proc_macros::rpc;

#[rpc(server, client)]
pub trait SoloMiningRpc {
    /// get_meta ask to the blockchain for SOLO mining params
    #[method(name = "get_meta")]
    async fn get_meta(&self) -> RpcResult<String>;

    /// push_to_node handles the payload from the miner and push it to the node
    #[method(name = "push_to_node")]
    async fn push_to_node(&self, hash: String, obj: String) -> RpcResult<String>;

    #[method(name = "push_stats")]
    async fn push_stats(
        &self,
        name: String,
        cores: String,
        tag: String,
        hashrate: String,
        good_hashrate: String,
    ) -> RpcResult<String>;    
}

pub struct SoloMiningRpcServerImpl {
    pub(crate) ctx: Arc<SoloAppContex>,
}

impl SoloMiningRpcServerImpl {
    pub fn new(ctx: Arc<SoloAppContex>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl SoloMiningRpcServer for SoloMiningRpcServerImpl {
    async fn get_meta(&self) -> RpcResult<String> {
        let response = self
            .ctx
            .get_meta()
            .await
            .map_err(|e| e.to_string())
            .unwrap();
        Ok(response)
    }
    async fn push_to_node(&self, hash: String, obj: String) -> RpcResult<String> {
        let response = self
            .ctx
            .push_to_node(hash, obj)
            .await
            .map_err(|e| e.to_string())
            .unwrap();
        Ok(response)
    }
    async fn push_stats(
        &self,
        name: String,
        cores: String,
        tag: String,
        hashrate: String,
        good_hashrate: String,
    ) -> RpcResult<String> {
        let response = self
            .ctx
            .push_stats(name, cores, tag, hashrate, good_hashrate)
            .await
            .map_err(|e| e.to_string())
            .unwrap();
        Ok(response)
    }    
}
