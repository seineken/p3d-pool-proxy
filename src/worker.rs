use hyper::Method;
use jsonrpsee::server::{RpcModule, Server};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::pool_rpc::{MiningRpcServer, MiningRpcServerImpl};

use super::PoolContex;

pub(crate) async fn run_rpc_server(ctx: Arc<PoolContex>) -> anyhow::Result<SocketAddr> {
    let cors = CorsLayer::new()
        .allow_methods([Method::POST])
        .allow_origin(Any)
        .allow_headers([hyper::header::CONTENT_TYPE]);
    let middleware = tower::ServiceBuilder::new().layer(cors);

    let socker_url: SocketAddr = ctx.pool_url.clone().parse::<SocketAddr>()?;
    let server = Server::builder()
        .set_middleware(middleware)
        .build(socker_url)
        .await?;

    let mut module = RpcModule::new(ctx.clone());

    module.merge(MiningRpcServerImpl::new(ctx.clone()).into_rpc())?;

    let addr = server.local_addr()?;
    let handle = server.start(module);

    tokio::spawn(handle.stopped());

    Ok(addr)
}
