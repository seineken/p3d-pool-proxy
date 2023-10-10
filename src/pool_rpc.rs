use std::sync::Arc;

use crate::pool_handler::AppContex;
use jsonrpsee::core::{async_trait, RpcResult};
use jsonrpsee::proc_macros::rpc;

#[rpc(server, client)]
pub trait PoolMiningRpc {
    /// get_mining_params ask to the blockchain for POOL mining params
    #[method(name = "get_mining_params")]
    async fn get_mining_params(&self) -> RpcResult<String>;

    /// push_to_pool handles the payload from the miner and push it to the POOL
    #[method(name = "push_to_pool")]
    async fn push_to_pool(&self, hash: String, obj: String) -> RpcResult<String>;

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

pub struct PoolMiningRpcServerImpl {
    pub(crate) ctx: Arc<AppContex>,
}

impl PoolMiningRpcServerImpl {
    pub fn new(ctx: Arc<AppContex>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl PoolMiningRpcServer for PoolMiningRpcServerImpl {
    async fn get_mining_params(&self) -> RpcResult<String> {
        let response = self
            .ctx
            .get_mining_params()
            .await
            .map_err(|e| e.to_string())
            .unwrap();
        Ok(response)
    }
    async fn push_to_pool(&self, hash: String, obj: String) -> RpcResult<String> {
        let response = self
            .ctx
            .push_to_pool(hash, obj)
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
