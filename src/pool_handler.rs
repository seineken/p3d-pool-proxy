use std::cmp::{max, min};
use codec::Encode;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::core::{Error, JsonValue};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::rpc_params;
use p3d::p3d_process;
use primitive_types::{H256, U256};
use sha3::{Digest, Sha3_256};
use std::collections::HashSet;
use std::result::Result;
use std::str::FromStr;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use ansi_term::Style;

extern crate redis;

use crate::message::{Message, StatsPayload};
use crate::utils::log;
use crate::worker::{
    AlgoType, DoubleHash, DynamicMiningParams, MiningObj, MiningParams, P3dParams,
};
use mongodb::{options::ClientOptions, Client as ClientMongo, Cursor};
use mongodb::bson::{DateTime, doc};
use mongodb::options::FindOptions;
use serde::{Deserialize, Serialize};

pub const BLOCK_TIME_SEC: u64 = 60;
/// Block time interval in milliseconds.
pub const BLOCK_TIME: u64 = BLOCK_TIME_SEC * 1000;
// pub const BLOCK_TIME_WINDOW: u64 = BLOCK_TIME_SEC * 1000;
pub const TARGET_BLOCK_TIME: u64 = BLOCK_TIME_SEC * 1000;

pub const HOUR_HEIGHT: u64 = 3600 / BLOCK_TIME_SEC;
// /// A day is 1440 blocks
// pub const DAY_HEIGHT: u64 = 24 * HOUR_HEIGHT;
// /// A week is 10_080 blocks
// pub const WEEK_HEIGHT: u64 = 7 * DAY_HEIGHT;
// /// A year is 524_160 blocks
// pub const YEAR_HEIGHT: u64 = 52 * WEEK_HEIGHT;

/// Number of blocks used to calculate difficulty adjustments
pub const DIFFICULTY_ADJUST_WINDOW: u64 = HOUR_HEIGHT;
/// Clamp factor to use for difficulty adjustment
/// Limit value to within this factor of goal
pub const CLAMP_FACTOR: u128 = 2;
/// Dampening factor to use for difficulty adjustment
pub const DIFFICULTY_DAMP_FACTOR: u128 = 3;
/// Minimum difficulty, enforced in diff retargetting
/// avoids getting stuck when trying to increase difficulty subject to dampening
const INITIAL_DIFFICULTY: u64 = 2000000;
pub const MIN_DIFFICULTY: u128 = INITIAL_DIFFICULTY as u128;
/// Maximum difficulty.
pub const MAX_DIFFICULTY: u128 = u128::max_value();

#[derive(Clone)]
pub struct DifficultyAndTimestamp {
    pub difficulty: U256,
    pub timestamp: i64,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Share {
    pub miner_wallet: String,
    pub rig_name: String,
    pub timestamp: DateTime,
    pub difficulty: U256,
    pub accounted: bool,
    pub paid: bool,
}

pub struct AppContex {
    pub(crate) p3d_params: P3dParams,
    pub(crate) pool_id: String,
    pub(crate) proxy_address: String,
    pub(crate) cur_state: Mutex<Option<MiningParams>>,
    pub(crate) dynamic_mp: Mutex<Option<DynamicMiningParams>>,
    pub(crate) processed_hashes: Mutex<Option<HashSet<H256>>>,

    pub(crate) mongo: ClientMongo,
    pub(crate) client: HttpClient,
}

impl AppContex {
    pub(crate) async fn new(
        p3d_params: P3dParams,
        node_addr: &str,
        proxy_address: String,
        pool_id: String,
        mongo_addr: &str,
    ) -> anyhow::Result<Self> {

        let client_options = ClientOptions::parse(mongo_addr)
            .await
            .expect("Failed to load mongoDB.");

        Ok(AppContex {
            p3d_params,
            pool_id,
            proxy_address,
            cur_state: Mutex::new(None),
            dynamic_mp: Mutex::new(None),
            processed_hashes: Mutex::new(Some(HashSet::new())),
            mongo: ClientMongo::with_options(client_options)?,
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

        // Reverse the bytes of the public key
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
                    no_shares_round: false,
                }
            }
        };

        let DynamicMiningParams {
            dynamic_difficulty,
            ..
        } = dynamic_diff;

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

    pub(crate) async fn push_to_pool(&self, _hash: String, obj: String, wallet: String, rig_name: String) -> Result<String, Error> {
        let P3dParams { algo, sect, grid } = self.p3d_params.clone();
        let _hash = H256::from_str(&_hash).unwrap();

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
                ..
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

                    let mut processed_hashes = {
                        let params_lock = self.processed_hashes.lock().unwrap();
                        if let Some(mp) = (*params_lock).clone() {
                            mp
                        } else {
                            drop(params_lock);
                            thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                    };

                    if processed_hashes.contains(&obj_hash) {
                        log(format!(
                            "🚩 Duplicated hash discarded {:x}",
                            obj_hash.clone()
                        ));
                        break;
                    }

                    let poscan_hash = DoubleHash { pre_hash, obj_hash }.calc_hash();
                    processed_hashes.insert(obj_hash.clone());

                    let mut lock_ph = self.processed_hashes.lock().unwrap();
                    (*lock_ph) = Some(processed_hashes);
                    thread::sleep(Duration::from_millis(10));

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

                self.submit_share(
                    "pool-p3d",
                    "shares",
                    wallet.clone(),
                    rig_name.clone(),
                    diff,
                ).await.unwrap();

                let response = self
                    .client
                    .request::<u64, _>(
                        "poscan_pushMiningObject",
                        rpc_params![serde_json::json!(1), serde_json::json!(obj)],
                    )
                    .await
                    .unwrap();

                if response == 0 {
                    log(format!(
                        "💎 Share found difficulty: {} :: Pool Difficulty: {} :: Chain difficulty: {}",
                        Style::new().bold().paint(format!("{:.2}", &diff)),
                        Style::new().bold().paint(format!("{:.2}", &pow_difficulty)),
                        &win_difficulty
                    ));
                    self.adjust_difficulty(wallet, rig_name).await.unwrap();
                }
                break;
            }
            break;
        }
        Ok(String::from("Pushed to pool for validation"))
    }

    async fn submit_share(
        &self,
        db_name: &str,
        coll_name: &str,
        miner_wallet: String,
        rig_name: String,
        difficulty: U256,
    ) -> anyhow::Result<u64> {
        let db = self.mongo.database(db_name);
        let coll = db.collection::<Share>(coll_name);
        let share = Share {
            miner_wallet,
            rig_name,
            timestamp: DateTime::now(),
            difficulty,
            accounted: false,
            paid: false,
        };
        coll.insert_one(share, None).await?;
        Ok(0)
    }

    pub(crate) async fn push_stats(
        &self,
        name: String,
        cores: String,
        tag: String,
        hashrate: String,
        good_hashrate: String,
    ) -> Result<String, Error> {
        let _payload = StatsPayload {
            name,
            cores,
            tag,
            hashrate,
            good_hashrate: good_hashrate.clone(),
        };

        // self.adjust_difficulty(good_hashrate.clone());

        // let message = Message::new(self.member_id.clone(), payload);
        // let response = self
        //     .store_stats(message)
        //     .map_err(|e| e.to_string())
        //     .unwrap();
        // Ok(hex::encode(payload.encode()))
        Ok(String::from("WIP"))
    }

    fn store_stats(&self, _message: Message) -> Result<String, Error> {
        Ok(String::from("store_stats"))
    }

    async fn get_miner_shares(
        &self,
        db_name: &str,
        coll_name: &str,
        miner_wallet: String,
        rig_name: String,
    ) -> anyhow::Result<Vec<Share>> {
        let db = self.mongo.database(db_name);
        let coll = db.collection::<Share>(coll_name);
        let filter = doc! {"miner_wallet": miner_wallet, "rig_name": rig_name ,"accounted": false};
        let find_options = FindOptions::builder()
            .sort(doc! { "timestamp": -1 })
            .build();
        let mut cursor: Cursor<Share> = coll.find(filter, find_options).await?;

        let mut result = Vec::new();

        while cursor.advance().await? {
            let share = cursor.deserialize_current()?;
            result.push(share);
        }

        Ok(result)
    }

    pub async fn adjust_difficulty(
        &self,
        wallet: String,
        rig_name: String,
    ) -> anyhow::Result<()> {
        log(String::from("💯 Adjusting difficulty"));
        let db_name = "pool-p3d";
        let coll_name = "shares";

        let mut shares = self
            .get_miner_shares(db_name, coll_name, wallet.clone(), rig_name.clone())
            .await
            .unwrap();

        if shares.len() > 5 {
            shares.sort_by_key(|share| share.timestamp);

            let mut data = vec![None; DIFFICULTY_ADJUST_WINDOW as usize];
            for i in 1..shares.len() {
                data[i - 1] = Some(DifficultyAndTimestamp {
                    timestamp: shares[i].timestamp.timestamp_millis(),
                    difficulty: shares[i].difficulty,
                });
            }

            let mut ts_delta = 0;
            for i in 1..DIFFICULTY_ADJUST_WINDOW as usize {
                let prev = data[i - 1].as_ref().map(|d| d.timestamp);
                let cur = data[i].as_ref().map(|d| d.timestamp);

                let delta = match (prev, cur) {
                    (Some(prev), Some(cur)) => cur.saturating_sub(prev),
                    _ => TARGET_BLOCK_TIME as i64,
                };
                ts_delta += delta;
            }

            if ts_delta == 0 {
                ts_delta = 1;
            }

            let mut diff_sum = U256::zero();
            for i in 0..DIFFICULTY_ADJUST_WINDOW as usize {
                let diff = match data[i].as_ref().map(|d| d.difficulty) {
                    Some(diff) => U256::from(diff),
                    None => U256::from(INITIAL_DIFFICULTY),
                };
                diff_sum += diff;
            }

            if diff_sum < U256::from(MIN_DIFFICULTY) {
                diff_sum = U256::from(MIN_DIFFICULTY);
            }

            let adj_ts = self.clamp(
                self.damp(
                    ts_delta as u128,
                    BLOCK_TIME as u128,
                    DIFFICULTY_DAMP_FACTOR,
                ),
                BLOCK_TIME as u128,
                CLAMP_FACTOR,
            );

            let difficulty = min(
                U256::from(MAX_DIFFICULTY),
                max(
                    U256::from(MIN_DIFFICULTY),
                    diff_sum * U256::from(TARGET_BLOCK_TIME) / U256::from(adj_ts),
                ),
            );

            let mut lock_mp = self.dynamic_mp.lock().unwrap();
            (*lock_mp) = Some(DynamicMiningParams {
                dynamic_difficulty: difficulty,
                no_shares_round: false,
            });
            log(format!("🦾 New adjusted difficulty set to {}", difficulty));
        } else {
            let mut lock_mp = self.dynamic_mp.lock().unwrap();
            (*lock_mp) = Some(DynamicMiningParams {
                dynamic_difficulty: U256::from(INITIAL_DIFFICULTY),
                no_shares_round: false,
            });
            log(format!("🦾 Difficulty set to {}", INITIAL_DIFFICULTY));
        }

        Ok(())
    }

    /// Move value linearly toward a goal
    fn damp(&self, actual: u128, goal: u128, damp_factor: u128) -> u128 {
        (actual + (damp_factor - 1) * goal) / damp_factor
    }

    /// limit value to be within some factor from a goal
    fn clamp(&self, actual: u128, goal: u128, clamp_factor: u128) -> u128 {
        max(goal / clamp_factor, min(actual, goal * clamp_factor))
    }
}
