use ansi_term::Style;
use codec::Encode;
use ecies_ed25519::encrypt;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::core::{Error, JsonValue};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::rpc_params;
use p3d::p3d_process;
use primitive_types::{H256, U256};
use rand::{rngs::StdRng, SeedableRng};
use redis::{Commands, Value};
use schnorrkel::{ExpansionMode, MiniSecretKey, SecretKey, Signature};
use sha3::{Digest, Sha3_256};
use std::collections::HashSet;
// use tokio_stream::StreamExt;
use std::result::Result;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

extern crate redis;

use crate::message::{Message, StatsPayload};
use crate::utils::{connect, log};
use crate::worker::{
    AlgoType, DoubleHash, DynamicMiningParams, MiningObj, MiningParams, P3dParams, Payload,
};

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

pub struct AppContex {
    pub(crate) p3d_params: P3dParams,
    pub(crate) pool_id: String,
    pub(crate) member_id: String,
    pub(crate) key: SecretKey,
    pub(crate) proxy_address: String,
    pub(crate) cur_state: Mutex<Option<MiningParams>>,
    pub(crate) dynamic_mp: Mutex<Option<DynamicMiningParams>>,

    pub(crate) client: HttpClient,
}

impl AppContex {
    pub(crate) async fn new(
        p3d_params: P3dParams,
        node_addr: &str,
        proxy_address: String,
        pool_id: String,
        member_id: String,
        key: String,
    ) -> anyhow::Result<Self> {
        let key = key.replacen("0x", "", 1);
        let key_data = hex::decode(&key[..])?;
        let key = MiniSecretKey::from_bytes(&key_data[..])
            .expect("Invalid key")
            .expand(ExpansionMode::Ed25519);

        Ok(AppContex {
            p3d_params,
            pool_id,
            member_id,
            key,
            proxy_address,
            cur_state: Mutex::new(None),
            dynamic_mp: Mutex::new(None),
            client: HttpClientBuilder::default().build(node_addr)?,
        })
    }

    pub(crate) async fn get_mining_params(&self) -> Result<String, Error> {
        let meta: JsonValue = self
            .client
            .request::<JsonValue, _>(
                "poscan_getMiningParams",
                rpc_params![serde_json::json!(self.pool_id)],
            )
            .await
            .unwrap();

        let default_response: Vec<JsonValue> = Vec::new();

        let content: Vec<String> = meta
            .as_array()
            .unwrap_or_else(|| &default_response)
            .iter()
            .filter_map(|param| param.as_str().map(String::from))
            .collect();

        let (pre_hash, parent_hash, win_difficulty, mut pow_difficulty, pub_key) =
            match content.as_slice() {
                [pre_hash, parent_hash, win_difficulty, pow_difficulty, pub_key] => (
                    H256::from_str(pre_hash).unwrap(),
                    H256::from_str(parent_hash).unwrap(),
                    U256::from_str_radix(win_difficulty, 16).unwrap(),
                    U256::from_str_radix(pow_difficulty, 16).unwrap(),
                    U256::from_str_radix(pub_key, 16).unwrap(),
                ),
                _ => {
                    return Err(Error::Custom(
                        "There are not enough elements in content".into(),
                    ));
                }
            };

        let mut pub_key_extra = pub_key.clone().encode();
        pub_key_extra.reverse();
        let pub_key_extra = ecies_ed25519::PublicKey::from_bytes(&pub_key_extra).unwrap();

        let dynamic_diff: DynamicMiningParams = {
            let dyn_param = self.dynamic_mp.lock().unwrap();
            if let Some(dp) = (*dyn_param).clone() {
                dp
            } else {
                drop(dyn_param);
                DynamicMiningParams {
                    dynamic_difficulty: U256::zero(),
                }
            }
        };

        let DynamicMiningParams { dynamic_difficulty } = dynamic_diff;

        if dynamic_difficulty > pow_difficulty {
            pow_difficulty = dynamic_difficulty;

            // If the dynamic difficulty is higher than network's difficulty
            // then the difficulty for the miner is the network's difficulty
            if pow_difficulty >= win_difficulty {
                pow_difficulty = win_difficulty;
            }
        }

        let mut lock = self.cur_state.lock().unwrap();
        (*lock) = Some(MiningParams {
            pre_hash,
            parent_hash,
            win_difficulty,
            pow_difficulty,
            pub_key: pub_key_extra,
        });

        Ok(format!(
            "{}",
            hex::encode(
                (
                    pre_hash,
                    parent_hash,
                    win_difficulty,
                    pow_difficulty,
                    pub_key
                )
                    .encode()
            )
        ))
    }

    pub(crate) async fn push_to_pool(&self, hash: String, obj: String) -> Result<String, Error> {
        let P3dParams { algo, sect, grid } = self.p3d_params.clone();
        let mut processed_hashes: HashSet<H256> = HashSet::new();

        let hash = H256::from_str(&hash).unwrap();

        loop {
            let mining_params = {
                let params_lock = self.cur_state.lock().unwrap();
                if let Some(mp) = (*params_lock).clone() {
                    mp
                } else {
                    drop(params_lock);
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
            };

            let MiningParams {
                pre_hash,
                parent_hash,
                win_difficulty,
                pow_difficulty,
                pub_key,
            } = mining_params;
            let rot_hash = match &algo {
                AlgoType::Grid2dV3_1 => pre_hash,
                _ => parent_hash,
            };

            let rot = rot_hash.encode()[0..4].try_into().ok();

            let mining_obj: MiningObj = MiningObj {
                obj_id: 1,
                obj: obj.as_bytes().to_vec(),
            };

            let res_hashes = p3d_process(
                mining_obj.obj.as_slice(),
                algo.as_p3d_algo(),
                grid as i16,
                sect as i16,
                rot,
            );

            let (_first_hash, _obj_hash, poscan_hash) = match res_hashes {
                Ok(hashes) if !hashes.is_empty() => {
                    let first_hash = hashes[0].clone();
                    let obj_hash = H256::from_str(&first_hash).unwrap();
                    if processed_hashes.contains(&obj_hash) {
                        continue;
                    }
                    let poscan_hash = DoubleHash { pre_hash, obj_hash }.calc_hash();
                    processed_hashes.insert(obj_hash.clone());
                    (first_hash, obj_hash, poscan_hash)
                }
                _ => {
                    continue;
                }
            };

            for difficulty in [pow_difficulty, win_difficulty] {
                let comp = Compute {
                    difficulty,
                    pre_hash,
                    poscan_hash,
                };

                let diff = get_hash_difficulty(&comp.get_work());

                if diff >= difficulty {
                    let payload = Payload {
                        pool_id: self.pool_id.clone(),
                        member_id: self.member_id.clone(),
                        pre_hash,
                        parent_hash,
                        algo: self.p3d_params.algo.as_str().to_owned(),
                        dfclty: diff.clone(),
                        hash,
                        obj_id: 1,
                        obj: obj.as_bytes().to_vec(),
                    };

                    // Convert the payload to JSON string
                    let message = serde_json::to_string(&payload).unwrap();

                    // Create a cryptographically secure PRNG from hash
                    let mut csprng = StdRng::from_seed(hash.encode().try_into().unwrap());

                    // Encrypt the message using the public key and csprng
                    let encrypted = encrypt(&pub_key, message.as_bytes(), &mut csprng).unwrap();
                    // Sign the encrypted message using the sign method
                    let sign = self.sign(&encrypted);

                    // Create RPC parameters using encrypted, self.member_id, and sign
                    let params = rpc_params![
                        serde_json::json!(encrypted.clone()),
                        serde_json::json!(self.member_id.clone()),
                        serde_json::json!(hex::encode(sign.to_bytes()))
                    ];

                    log(format!(
                        "💎 Share found difficulty: {} :: Pool Difficulty: {} :: Chain difficulty: {}",
                        Style::new().bold().paint(format!("{:.2}", &diff)),
                        Style::new().bold().paint(format!("{:.2}", &pow_difficulty)),
                        &win_difficulty
                    ));

                    // Make the RPC request to poscan_pushMiningObjectToPool method
                    let _response: JsonValue = self
                        .client
                        .request("poscan_pushMiningObjectToPool", params)
                        .await
                        .unwrap();

                    // Check the response value and print appropriate messages
                    if _response == 0 {
                        let message = format!(
                            "{}",
                            Style::new().bold().paint(format!("✅ Share accepted"))
                        );
                        log(message.clone());

                        let mut lock_mp = self.dynamic_mp.lock().unwrap();
                        (*lock_mp) = Some(DynamicMiningParams {
                            dynamic_difficulty: diff.clone(),
                        });
                        thread::sleep(Duration::from_millis(10));
                    } else {
                        let message = format!("{}", Style::new().bold().paint("⛔ Share Rejected"));
                        log(message.clone());
                    }
                }
                break;
            }
            break;
        }
        Ok(String::from("Pushed to pool for validation"))
    }

    pub(crate) async fn push_stats(
        &self,
        name: String,
        cores: String,
        tag: String,
        hashrate: String,
        good_hashrate: String,
    ) -> Result<String, Error> {
        let payload = StatsPayload {
            name,
            cores,
            tag,
            hashrate,
            good_hashrate,
        };

        // let message = Message::new(self.member_id.clone(), payload);
        // let response = self
        //     .store_stats(message)
        //     .map_err(|e| e.to_string())
        //     .unwrap();
        // Ok(hex::encode(payload.encode()))
        Ok(String::from("WIP"))
    }

    fn sign(&self, msg: &[u8]) -> Signature {
        // Define a constant CTX as a byte array with the value "Mining pool"
        const CTX: &[u8] = b"Mining pool";

        // Call the `sign_simple` method on the `self.key` object (a private key)
        // Pass in the CTX, the message (msg), and the public key derived from the private key
        self.key.sign_simple(CTX, msg, &self.key.to_public())
    }

    fn store_stats(&self, message: Message) -> Result<String, Error> {
        let mut con = connect();
        let payload = serde_json::to_string(&message)?;
        let result: String = con.set(message.channel, payload).map_err(|e| e).unwrap();
        let response = format!("{}", result);
        Ok(response)
    }
}
