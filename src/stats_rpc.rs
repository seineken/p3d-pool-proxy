use jsonrpsee::core::RpcResult;
use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;

#[rpc(server, client)]
pub trait StatsRpc {
	#[method(name = "get_stats")]
	async fn get_stats(
		&self,
        member_id: String,
	) -> RpcResult<String>;
}

pub struct StatsRpcServerImpl;

impl StatsRpcServerImpl {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatsRpcServer for StatsRpcServerImpl {
	async fn get_stats(
		&self,
        member_id: String
	) -> RpcResult<String> {
		Ok(String::from(""))
	}  
}
