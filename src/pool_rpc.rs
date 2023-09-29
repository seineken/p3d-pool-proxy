use std::collections::HashSet;
use std::sync::Arc;

use crate::rpc::{AlgoType, MiningObj, MiningProposal, P3dParams, Payload};
use crate::worker::{Compute, DoubleHash};
use codec::{Codec, Encode};
use ecies_ed25519::encrypt;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::core::JsonValue;
use jsonrpsee::core::{async_trait, Error as JsonRpseeError, RpcResult};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::rpc_params;
use p3d::p3d_process;
use primitive_types::{H256, U256};
use rand::{rngs::StdRng, SeedableRng};
use schnorrkel::{ExpansionMode, MiniSecretKey, PublicKey, SecretKey, Signature};
use std::str::FromStr;

fn sign(key: SecretKey, msg: &[u8]) -> Signature {
    const CTX: &[u8] = b"Mining pool";
    key.sign_simple(CTX, msg, &key.to_public())
}

pub fn get_hash_difficulty(hash: &H256) -> U256 {
    let num_hash = U256::from(&hash[..]);
    let max = U256::max_value();
    max / num_hash
}

type ParamsResp = (H256, H256, U256, U256, U256);

#[rpc(server, client)]
pub trait MiningRpc {
    #[method(name = "get_meta")]
    async fn get_meta(&self) -> RpcResult<String>;

    #[method(name = "get_mining_params")]
    async fn get_mining_params(&self) -> RpcResult<String>;

    #[method(name = "push_to_pool")]
    async fn push_to_pool(
        &self,
        pool_id: String,
        member_id: String,
        pre_hash: String,
        parent_hash: String,
        win_difficulty: String,
        pow_difficulty: String,
        hash: String,
        obj: String,
        pub_key: String,
    ) -> RpcResult<u64>;
}

pub struct MiningRpcServerImpl {
    pub(crate) p3d_params: P3dParams,
    pub(crate) pool_id: String,
    pub(crate) member_id: String,
    pub(crate) algo: String,
    pub(crate) key: SecretKey,
    pub(crate) client: Arc<HttpClient>,
}

impl MiningRpcServerImpl {
    pub fn new(
        p3d_params: P3dParams,
        pool_id: String,
        member_id: String,
        algo: String,
        key: SecretKey,
        client: Arc<HttpClient>,
    ) -> Self {
        Self {
            p3d_params,
            pool_id,
            member_id,
            algo,
            key,
            client,
        }
    }
}

#[async_trait]
impl MiningRpcServer for MiningRpcServerImpl {
    async fn get_meta(&self) -> RpcResult<String> {
        let meta = self
            .client
            .request::<String, _>("poscan_getMeta", rpc_params![])
            .await
            .unwrap();
        Ok(meta)
    }
    async fn get_mining_params(&self) -> RpcResult<String> {
        let meta: JsonValue = self
            .client
            .request::<JsonValue, _>(
                "poscan_getMiningParams",
                rpc_params![serde_json::json!(self.pool_id)],
            )
            .await
            .unwrap();

        let mut content: Vec<String> = Vec::new();

        if let Some(params) = meta.as_array() {
            for param in params {
                if let Some(param_str) = param.as_str() {
                    // println!("{}", param_str);
                    content.push(param_str.to_string());
                }
            }
        }

        let pre_hash = H256::from_str(&content[0].clone()).unwrap();
        let parent_hash = H256::from_str(&content[1].clone()).unwrap();
        let win_difficulty = U256::from_str_radix(&content[2].clone(), 16).unwrap();
        let pow_difficulty = U256::from_str_radix(&content[3].clone(), 16).unwrap();
        let pub_key = U256::from_str_radix(&content[4].clone(), 16).unwrap();

        Ok(format!(
            "{}",
            hex::encode(
                (
                    pre_hash,
                    parent_hash,
                    win_difficulty,
                    pow_difficulty,
                    pub_key,
                )
                    .encode()
            )
        ))
    }
    async fn push_to_pool(
        &self,
        pool_id: String,
        member_id: String,
        pre_hash: String,
        parent_hash: String,
        win_difficulty: String,
        pow_difficulty: String,
        hash: String,
        obj: String,
        pub_key: String,
    ) -> RpcResult<u64> {
        let P3dParams { algo, sect, grid } = self.p3d_params.clone();
        let mut processed_hashes: HashSet<H256> = HashSet::new();

        let hash = H256::from_str(hash.as_str()).unwrap();

        let mut payload = Payload {
            pool_id: self.pool_id.clone(),
            member_id: self.member_id.clone(),
            pre_hash: H256::from_str(pre_hash.as_str()).unwrap(),
            parent_hash: H256::from_str(parent_hash.as_str()).unwrap(),
            algo: self.algo.clone(),
            dfclty: U256::from_str_radix(pow_difficulty.as_str(), 16).unwrap(),
            hash,
            obj_id: 1,
            obj: obj.as_bytes().to_vec(),
        };

        let rot_hash = match &algo {
            AlgoType::Grid2dV3_1 => payload.pre_hash.clone(),
            _ => payload.parent_hash.clone(),
        };

        let rot = rot_hash.encode()[0..4].try_into().ok();

        let mining_obj: MiningObj = MiningObj {
            obj_id: 1,
            obj: payload.obj.clone(),
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
                // if processed_hashes.contains(&obj_hash) {
                //     continue;
                // }
                let poscan_hash = DoubleHash {
                    pre_hash: payload.pre_hash.clone(),
                    obj_hash,
                }
                .calc_hash();
                processed_hashes.insert(obj_hash.clone());
                (first_hash, obj_hash, poscan_hash)
            }
            _ => panic!("Something really weird is going on in the Antartida."),
        };

        for difficulty in [
            payload.dfclty.clone(),
            U256::from_str_radix(&win_difficulty, 16).unwrap(),
        ] {
            let comp = Compute {
                difficulty,
                pre_hash: payload.pre_hash.clone(),
                poscan_hash,
            };

            let diff = get_hash_difficulty(&comp.get_work());

            if diff >= difficulty {
                payload.hash = obj_hash;

                let pub_key = U256::from_str_radix(&pub_key, 16).unwrap();
                let mut pub_key = pub_key.encode();
                pub_key.reverse();
                let pub_key = ecies_ed25519::PublicKey::from_bytes(&pub_key).unwrap();

                let message = serde_json::to_string(&payload).unwrap();
                let mut csprng = StdRng::from_seed(obj_hash.encode().try_into().unwrap());
                let encrypted = encrypt(&pub_key, message.as_bytes(), &mut csprng).unwrap();
                let sign = sign(self.key.clone(), &encrypted);

                let params = rpc_params![
                    serde_json::json!(encrypted.clone()),
                    serde_json::json!(self.member_id.clone()),
                    serde_json::json!(hex::encode(sign.to_bytes()))
                ];

                let _response: JsonValue = self
                    .client
                    .request("poscan_pushMiningObjectToPool", params)
                    .await
                    .unwrap();

                println!("{}", _response);
            }
        }

        Ok(0)
    }
}
