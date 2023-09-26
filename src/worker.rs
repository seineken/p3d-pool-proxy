use codec::Encode;
use hex::ToHex;
use hyper::Method;
use jsonrpsee::core::{JsonValue, params};
use jsonrpsee::server::{RpcModule, Server};
use primitive_types::{H256, U256};
use tokio::time;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};

use super::PoolContex;
use crate::rpc::{MiningParams, MiningProposal};

const ASK_MINING_PARAMS_PERIOD: Duration = Duration::from_secs(1);

pub(crate) async fn queue_management(ctx: Arc<PoolContex>) {
    loop {
        let maybe_prop = {
            let mut lock = ctx.out_queue.lock().unwrap();
            (*lock).pop_front()
        };
        if let Some(prop) = maybe_prop {
            let res = ctx.push_to_pool(prop).await;
            if let Err(e) = res {
                println!("🟥 Error: {}", &e);
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

    let mut module = RpcModule::new(ctx);

    module.register_async_method("push_to_node", |params, context| {
        let response: JsonValue = params.parse().unwrap();

        async move {
            let obj: Option<&str> = response.get(0).expect("Expect obj").as_str();
            let hash: Option<&str> = response.get(1).expect("Expect hash").as_str();

            let pre_hash: Option<&str> = response.get(2).expect("Expect pre_hash").as_str();
            let parent_hash: Option<&str> = response.get(3).expect("Expect parent_hash").as_str();
            let pow_difficulty: Option<&str> =
                response.get(5).expect("Expect pow_difficulty").as_str();
            let pub_key: Option<&str> = response.get(6).expect("public key").as_str();

            match (hash, obj, pre_hash, parent_hash, pow_difficulty, pub_key) {
                (
                    Some(hash),
                    Some(obj),
                    Some(pre_hash),
                    Some(parent_hash),
                    Some(pow_difficulty),
                    Some(pub_key),
                ) => {
                    let hash = H256::from_str(hash).unwrap();
                    let obj = obj.encode();

                    let pre_hash = H256::from_str(pre_hash).unwrap();
                    let parent_hash = H256::from_str(parent_hash).unwrap();
                    let pow_difficulty = U256::from_str_radix(pow_difficulty, 16).unwrap();
                    // let pub_key = U256::from_str_radix(pub_key, 16).unwrap();
                    let pub_key =  H256::from_str(pub_key).unwrap();
                    let mut pub_key = pub_key.encode();
                    pub_key.reverse();
                    let pub_key = ecies_ed25519::PublicKey::from_bytes(&pub_key).unwrap();

                    let mining_params = MiningParams {
                        pre_hash,
                        parent_hash,
                        pow_difficulty,
                        pub_key,
                    };

                    let prop = MiningProposal {
                        params: mining_params.clone(),
                        hash,
                        obj_id: 1,
                        obj,
                    };

                    println!(
                        "💎 Share found \n pre_hash: {}\n parent_hast: {}\n hash: {}\n",
                        prop.hash.clone(),
                        prop.params.parent_hash.clone(),
                        prop.hash.clone(),
                    );
                    context.push_to_queue(prop);
                }
                _ => {
                    println!("🟥 Something failed retrieving the params.");
                }
            }
        }
    })?;

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
