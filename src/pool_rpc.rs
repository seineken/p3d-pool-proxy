use std::sync::Arc;

use crate::rpc::Payload;
use codec::Encode;
use ecies_ed25519::encrypt;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::core::JsonValue;
use jsonrpsee::core::{async_trait, Error as JsonRpseeError, RpcResult};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::rpc_params;
use primitive_types::{H256, U256};
use rand::{rngs::StdRng, SeedableRng};
use schnorrkel::{ExpansionMode, MiniSecretKey, PublicKey, SecretKey, Signature};
use std::str::FromStr;

fn sign(key: SecretKey, msg: &[u8]) -> Signature {
    const CTX: &[u8] = b"Mining pool";
    key.sign_simple(CTX, msg, &key.to_public())
}

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
    pub(crate) pool_id: String,
    pub(crate) member_id: String,
    pub(crate) algo: String,
    pub(crate) key: SecretKey,
    pub(crate) client: Arc<HttpClient>,
}

impl MiningRpcServerImpl {
    pub fn new(
        pool_id: String,
        member_id: String,
        algo: String,
        key: SecretKey,
        client: Arc<HttpClient>,
    ) -> Self {
        Self {
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

        if let Some(params) = meta.as_array() {
            for param in params {
                if let Some(param_str) = param.as_str() {
                    println!("{}", param_str);
                }
            }
        }

        Ok("".into())
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
        let hash = H256::from_str(hash.as_str()).unwrap();

        let payload = Payload {
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

        let pub_key = U256::from_str_radix(pub_key.as_str(), 16).unwrap();
        let mut pub_key = pub_key.encode();
        pub_key.reverse();
        let pub_key = ecies_ed25519::PublicKey::from_bytes(&pub_key).unwrap();

        let message = serde_json::to_string(&payload).unwrap();
        let mut csprng = StdRng::from_seed(hash.encode().try_into().unwrap());
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

        Ok(0)
    }
}
