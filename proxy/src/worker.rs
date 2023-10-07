use codec::Encode;
use hyper::Method;
use jsonrpsee::server::{RpcModule, Server};

use primitive_types::{H256, U256};
use serde::Serialize;
use sha3::{Digest, Sha3_256};

use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    pool_rpc::{PoolMiningRpcServer, PoolMiningRpcServerImpl},
    solo_handler::SoloAppContex,
    solo_rpc::{SoloMiningRpcServer, SoloMiningRpcServerImpl},
    stats_rpc::{StatsRpcServer, StatsRpcServerImpl},
};

use super::AppContex;

#[derive(Clone)]
pub(crate) struct MiningParams {
    pub(crate) pre_hash: H256,
    pub(crate) parent_hash: H256,
    pub(crate) win_difficulty: U256,
    pub(crate) pow_difficulty: U256,
    pub(crate) pub_key: ecies_ed25519::PublicKey,
}

#[derive(Clone)]
pub(crate) struct DynamicMiningParams {
    pub(crate) dynamic_difficulty: U256,
}

#[derive(Clone, Encode)]
pub(crate) enum AlgoType {
    Grid2d,
    Grid2dV2,
    Grid2dV3,
    Grid2dV3_1,
}

impl AlgoType {
    pub(crate) fn as_p3d_algo(&self) -> p3d::AlgoType {
        match self {
            Self::Grid2d => p3d::AlgoType::Grid2d,
            Self::Grid2dV2 => p3d::AlgoType::Grid2dV2,
            Self::Grid2dV3 |
            Self::Grid2dV3_1 => p3d::AlgoType::Grid2dV3,
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Grid2d => "Grid2d",
            Self::Grid2dV2 => "Grid2dV2",
            Self::Grid2dV3 => "Grid2dV3",
            Self::Grid2dV3_1 => "Grid2dV3.1",
        }
    }
}

pub(crate) struct MiningObj {
    pub(crate) obj_id: u64,
    pub(crate) obj: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct P3dParams {
    pub(crate) algo: AlgoType,
    pub(crate) grid: usize,
    pub(crate) sect: usize,
}

impl P3dParams {
    pub(crate) fn new(ver: &str) -> Self {
        let grid = 8;
        let (algo, sect) = match ver {
            "grid2d" => (AlgoType::Grid2d, 66),
            "grid2d_v2" => (AlgoType::Grid2dV2, 12),
            "grid2d_v3" => (AlgoType::Grid2dV3, 12),
            "grid2d_v3.1" => (AlgoType::Grid2dV3_1, 12),
            _ => panic!("Unknown algorithm: {}", ver),
        };

        Self { algo, grid, sect }
    }
}

#[derive(Encode)]
pub struct DoubleHash {
    pub pre_hash: H256,
    pub obj_hash: H256,
}

impl DoubleHash {
    pub fn calc_hash(self) -> H256 {
        H256::from_slice(Sha3_256::digest(&self.encode()[..]).as_slice())
    }
}

#[derive(Serialize)]
pub(crate) struct Payload {
    pub(crate) pool_id: String,
    pub(crate) member_id: String,
    pub(crate) pre_hash: H256,
    pub(crate) parent_hash: H256,
    pub(crate) algo: String,
    pub(crate) dfclty: U256,
    pub(crate) hash: H256,
    pub(crate) obj_id: u64,
    pub(crate) obj: Vec<u8>,
}

pub(crate) async fn pool_rpc_server(ctx: Arc<AppContex>) -> anyhow::Result<SocketAddr> {
    let cors = CorsLayer::new()
        .allow_methods([Method::POST])
        .allow_origin(Any)
        .allow_headers([hyper::header::CONTENT_TYPE]);
    let middleware = tower::ServiceBuilder::new().layer(cors);

    let socker_url: SocketAddr = ctx.proxy_address.clone().parse::<SocketAddr>()?;
    let server = Server::builder()
        .set_middleware(middleware)
        .build(socker_url)
        .await?;

    let mut module = RpcModule::new(ctx.clone());

    module.merge(PoolMiningRpcServerImpl::new(ctx.clone()).into_rpc())?;

    let addr = server.local_addr()?;
    let handle = server.start(module);

    tokio::spawn(handle.stopped());

    Ok(addr)
}

pub(crate) async fn solo_rpc_server(ctx: Arc<SoloAppContex>) -> anyhow::Result<SocketAddr> {
    let cors = CorsLayer::new()
        .allow_methods([Method::POST])
        .allow_origin(Any)
        .allow_headers([hyper::header::CONTENT_TYPE]);
    let middleware = tower::ServiceBuilder::new().layer(cors);

    let socker_url: SocketAddr = ctx.proxy_address.clone().parse::<SocketAddr>()?;
    let server = Server::builder()
        .set_middleware(middleware)
        .build(socker_url)
        .await?;

    let mut module = RpcModule::new(ctx.clone());

    module.merge(SoloMiningRpcServerImpl::new(ctx.clone()).into_rpc())?;

    let addr = server.local_addr()?;
    let handle = server.start(module);

    tokio::spawn(handle.stopped());

    Ok(addr)
}

pub(crate) async fn run_stats_server(proxy_address: String) -> anyhow::Result<SocketAddr> {
    let cors = CorsLayer::new()
        .allow_methods([Method::POST])
        .allow_origin(Any)
        .allow_headers([hyper::header::CONTENT_TYPE]);
    let middleware = tower::ServiceBuilder::new().layer(cors);

    let socker_url: SocketAddr = proxy_address.clone().parse::<SocketAddr>()?;
    let server = Server::builder()
        .set_middleware(middleware)
        .build(socker_url)
        .await?;

    let mut module = RpcModule::new(());

    module.merge(StatsRpcServerImpl::new().into_rpc())?;

    let addr = server.local_addr()?;
    let handle = server.start(module);

    tokio::spawn(handle.stopped());

    Ok(addr)
}