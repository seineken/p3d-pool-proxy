use codec::Encode;
use hyper::Method;
use jsonrpsee::core::JsonValue;
use jsonrpsee::server::{RpcModule, Server};
use p3d::p3d_process;
use primitive_types::{H256, U256};
use sha3::{Digest, Sha3_256};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tower_http::cors::{Any, CorsLayer};

use crate::pool_rpc::{MiningRpcServer, MiningRpcServerImpl};

use crate::rpc::{AlgoType, MiningObj, MiningParams, MiningProposal, P3dParams};

use super::PoolContex;

const ASK_MINING_PARAMS_PERIOD: Duration = Duration::from_secs(1);

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

#[derive(Clone, Encode)]
pub struct Compute {
    pub difficulty: U256,
    pub pre_hash: H256,
    pub poscan_hash: H256,
}

impl Compute {
    pub(crate) fn get_work(&self) -> H256 {
        let encoded_data = self.encode();
        let hash_digest = Sha3_256::digest(&encoded_data);
        H256::from_slice(&hash_digest)
    }
}

pub fn get_hash_difficulty(hash: &H256) -> U256 {
    let num_hash = U256::from(&hash[..]);
    let max = U256::max_value();
    max / num_hash
}

pub(crate) async fn queue_management(ctx: Arc<PoolContex>) {
    let P3dParams { algo, sect, grid } = ctx.p3d_params.clone();
    let mut processed_hashes: HashSet<H256> = HashSet::new();

    loop {
        let maybe_prop = {
            let mut lock = ctx.out_queue.lock().unwrap();
            (*lock).pop_front()
        };
        if let Some(prop) = maybe_prop {
            println!("Entrando: {:?}", prop.hash.clone());
            loop {
                let rot_hash = match &algo {
                    AlgoType::Grid2dV3_1 => prop.params.pre_hash.clone(),
                    _ => prop.params.parent_hash.clone(),
                };

                let rot = rot_hash.encode()[0..4].try_into().ok();

                let mining_obj: MiningObj = MiningObj {
                    obj_id: 1,
                    obj: prop.obj.clone(),
                };

                let res_hashes = p3d_process(
                    mining_obj.obj.as_slice(),
                    algo.as_p3d_algo(),
                    grid as i16,
                    sect as i16,
                    rot,
                );

                let (first_hash, obj_hash, poscan_hash) = match res_hashes {
                    Ok(hashes) if !hashes.is_empty() => {
                        let first_hash = hashes[0].clone();
                        let obj_hash = H256::from_str(&first_hash).unwrap();
                        if processed_hashes.contains(&obj_hash) {
                            continue;
                        }
                        let poscan_hash = DoubleHash {
                            pre_hash: prop.params.pre_hash.clone(),
                            obj_hash,
                        }
                        .calc_hash();
                        processed_hashes.insert(obj_hash.clone());
                        (first_hash, obj_hash, poscan_hash)
                    }
                    _ => {
                        continue;
                    }
                };

                for difficulty in [
                    prop.params.pow_difficulty.clone(),
                    prop.params.win_difficulty.clone(),
                ] {
                    let comp = Compute {
                        difficulty,
                        pre_hash: prop.params.pre_hash.clone(),
                        poscan_hash,
                    };

                    let diff = get_hash_difficulty(&comp.get_work());

                    if diff >= difficulty {
                        let prop = MiningProposal {
                            params: prop.params.clone(),
                            hash: obj_hash,
                            obj_id: mining_obj.obj_id,
                            obj: mining_obj.obj.clone(),
                        };
                        println!("obj_hash: {:?}", obj_hash);

                        let res = ctx.push_to_pool(prop).await;
                        if let Err(e) = res {
                            println!("🟥 Error: {}", &e);
                        }
                    }
                }
            }
        } else {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

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

    module.merge(
        MiningRpcServerImpl::new(
            ctx.p3d_params.clone(),
            ctx.pool_id.clone(),
            ctx.member_id.clone(),
            "Grid2dV3.1".into(),
            ctx.key.clone(),
            Arc::new(ctx.client.clone()),
        )
        .into_rpc(),
    )?;

    let addr = server.local_addr()?;
    let handle = server.start(module);

    tokio::spawn(handle.stopped());

    Ok(addr)
}

pub(crate) fn start_timer(ctx: Arc<PoolContex>) {
    let _forever = tokio::spawn(async move {
        let mut interval = time::interval(ASK_MINING_PARAMS_PERIOD);

        loop {
            interval.tick().await;

            let res = ctx.get_mining_params(ctx.pool_id.clone()).await;
            if let Err(e) = res {
                println!("🟥 Ask for mining params error: {}", &e);
            }
        }
    });
}
