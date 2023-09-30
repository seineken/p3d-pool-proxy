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
use schnorrkel::{ExpansionMode, MiniSecretKey, SecretKey, Signature};
use serde::Serialize;
use sha3::{Digest, Sha3_256};
use std::collections::HashSet;
use std::result::Result;
use std::str::FromStr;
use std::sync::Mutex;

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

#[derive(Clone)]
pub(crate) struct MiningParams {
    pub(crate) pre_hash: H256,
    pub(crate) parent_hash: H256,
    pub(crate) win_difficulty: U256,
    pub(crate) pow_difficulty: U256,
    pub(crate) pub_key: ecies_ed25519::PublicKey,
}

pub(crate) struct MiningObj {
    pub(crate) obj: Vec<u8>,
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
            Self::Grid2dV3 | Self::Grid2dV3_1 => p3d::AlgoType::Grid2dV3,
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

#[derive(Clone)]
pub struct P3dParams {
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

pub struct PoolContex {
    pub(crate) p3d_params: P3dParams,
    pub(crate) pool_id: String,
    pub(crate) member_id: String,
    pub(crate) key: SecretKey,
    pub(crate) pool_url: String,
    pub(crate) cur_state: Mutex<Option<MiningParams>>,

    pub(crate) client: HttpClient,
}

fn get_hash_difficulty(hash: &H256) -> U256 {
    let num_hash = U256::from(&hash[..]);
    let max = U256::max_value();
    max / num_hash
}

impl PoolContex {
    pub(crate) async fn new(
        p3d_params: P3dParams,
        node_addr: &str,
        pool_url: String,
        pool_id: String,
        member_id: String,
        key: String,
    ) -> anyhow::Result<Self> {
        let key = key.replacen("0x", "", 1);
        let key_data = hex::decode(&key[..])?;
        let key = MiniSecretKey::from_bytes(&key_data[..])
            .expect("Invalid key")
            .expand(ExpansionMode::Ed25519);

        Ok(PoolContex {
            p3d_params,
            pool_id,
            member_id,
            key,
            pool_url,
            cur_state: Mutex::new(None),
            client: HttpClientBuilder::default().build(node_addr)?,
        })
    }

    pub(crate) async fn get_meta(&self) -> Result<String, Error> {
        let meta = self
            .client
            .request::<String, _>("poscan_getMeta", rpc_params![])
            .await
            .map_err(|e| e.to_string())
            .unwrap();
        Ok(meta)
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

        let (pre_hash, parent_hash, win_difficulty, pow_difficulty, pub_key) =
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

    pub(crate) async fn push_to_node(&self, hash: String, obj: String) -> Result<String, Error> {
        let params = rpc_params![
            serde_json::json!(serde_json::json!(hash)),
            serde_json::json!(serde_json::json!(obj))
        ];
        let _response: JsonValue = self
            .client
            .request("poscan_pushMiningObject", params)
            .await
            .unwrap();

        if _response == 0 {
            println!(
                "{}",
                Style::new().bold().paint(format!("✅ Share accepted"))
            );
            Ok("✅ Share accepted".into())
        } else {
            println!("{}", Style::new().bold().paint("⛔ Share Rejected"));
            return Err(Error::Custom("⛔ Share Rejected".into()));
        }
    }

    pub(crate) async fn push_to_pool(&self, hash: String, obj: String) -> Result<String, Error> {
        let P3dParams { algo, sect, grid } = self.p3d_params.clone();
        let mut processed_hashes: HashSet<H256> = HashSet::new();

        let mining_params = {
            let params_lock = self.cur_state.lock().unwrap();
            if let Some(mp) = (*params_lock).clone() {
                mp
            } else {
                return Err(Error::Custom("Mining params not available".into()));
            }
        };

        let MiningParams {
            pre_hash,
            parent_hash,
            win_difficulty,
            pow_difficulty,
            pub_key,
        } = mining_params;

        let win_dfclty = win_difficulty;
        let dfclty = pow_difficulty;

        // Hash from miner
        let hash = H256::from_str(&hash).unwrap();

        let payload = Payload {
            pool_id: self.pool_id.clone(),
            member_id: self.member_id.clone(),
            pre_hash,
            parent_hash,
            algo: self.p3d_params.algo.as_str().to_owned(),
            dfclty,
            hash,
            obj_id: 1,
            obj: obj.as_bytes().to_vec(),
        };

        loop {
            let rot_hash = match &algo {
                AlgoType::Grid2dV3_1 => pre_hash.clone(),
                _ => parent_hash.clone(),
            };

            let rot = rot_hash.encode()[0..4].try_into().ok();

            let mining_obj: MiningObj = MiningObj {
                obj: payload.obj.clone(),
            };

            let res_hashes = p3d_process(
                mining_obj.obj.as_slice(),
                algo.as_p3d_algo(),
                grid as i16,
                sect as i16,
                rot,
            );

            let (_first_hash, obj_hash, poscan_hash) = match res_hashes {
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

            for difficulty in [dfclty, win_dfclty] {
                let comp = Compute {
                    difficulty,
                    pre_hash,
                    poscan_hash,
                };

                let diff = get_hash_difficulty(&comp.get_work());

                if diff >= difficulty {
                    let message = serde_json::to_string(&payload).unwrap();
                    let mut csprng = StdRng::from_seed(obj_hash.encode().try_into().unwrap());
                    let encrypted = encrypt(&pub_key, message.as_bytes(), &mut csprng).unwrap();
                    let sign = self.sign(&encrypted);

                    let params = rpc_params![
                        serde_json::json!(encrypted.clone()),
                        serde_json::json!(self.member_id.clone()),
                        serde_json::json!(hex::encode(sign.to_bytes()))
                    ];

                    println!(
                        "💎 Hash > Pool Difficulty: {} > {} (win: {})",
                        Style::new().bold().paint(format!("{:.2}", &diff)),
                        &dfclty,
                        &win_dfclty,
                    );

                    let _response: JsonValue = self
                        .client
                        .request("poscan_pushMiningObjectToPool", params)
                        .await
                        .unwrap();

                    if _response == 0 {
                        println!(
                            "{}",
                            Style::new().bold().paint(format!("✅ Share accepted"))
                        );
                    } else {
                        println!("{}", Style::new().bold().paint("⛔ Share Rejected"));
                    }
                }
                break;
            }
            break;
        }
        Ok("Proposed to Pool".into())
    }

    fn sign(&self, msg: &[u8]) -> Signature {
        const CTX: &[u8] = b"Mining pool";
        self.key.sign_simple(CTX, msg, &self.key.to_public())
    }
}
