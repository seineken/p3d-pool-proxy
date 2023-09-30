use std::sync::Arc;

use crate::rpc::PoolContex;
use jsonrpsee::core::{async_trait, RpcResult};
use jsonrpsee::proc_macros::rpc;

#[rpc(server, client)]
pub trait MiningRpc {
    /// get_meta ask to the blockchain for SOLO mining params
    #[method(name = "get_meta")]
    async fn get_meta(&self) -> RpcResult<String>;

    /// get_mining_params ask to the blockchain for POOL mining params
    #[method(name = "get_mining_params")]
    async fn get_mining_params(&self) -> RpcResult<String>;

    /// push_to_node handles the payload from the miner and push it to the node
    #[method(name = "push_to_node")]
    async fn push_to_node(&self, hash: String, obj: String) -> RpcResult<String>;

    /// push_to_pool handles the payload from the miner and push it to the POOL
    #[method(name = "push_to_pool")]
    async fn push_to_pool(&self, hash: String, obj: String) -> RpcResult<String>;
}

pub struct MiningRpcServerImpl {
    pub(crate) ctx: Arc<PoolContex>,
}

impl MiningRpcServerImpl {
    pub fn new(ctx: Arc<PoolContex>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl MiningRpcServer for MiningRpcServerImpl {
    async fn get_meta(&self) -> RpcResult<String> {
        let response = self
            .ctx
            .get_meta()
            .await
            .map_err(|e| e.to_string())
            .unwrap();
        Ok(response)
    }
    async fn get_mining_params(&self) -> RpcResult<String> {
        let response = self
            .ctx
            .get_mining_params()
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
    async fn push_to_pool(&self, hash: String, obj: String) -> RpcResult<String> {
        let response = self
            .ctx
            .push_to_pool(hash, obj)
            .await
            .map_err(|e| e.to_string())
            .unwrap();
        Ok(response)
    }
}
