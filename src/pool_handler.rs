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
use std::collections::HashSet;
use std::result::Result;
use std::str::FromStr;
use std::sync::Mutex;

use crate::worker::{P3dParams, MiningParams, Payload, AlgoType, MiningObj, DoubleHash, Compute};

pub struct AppContex {
    pub(crate) p3d_params: P3dParams,
    pub(crate) pool_id: String,
    pub(crate) member_id: String,
    pub(crate) key: SecretKey,
    pub(crate) proxy_address: String,
    pub(crate) cur_state: Mutex<Option<MiningParams>>,

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

    pub(crate) async fn push_to_pool(&self, hash: String, obj: String) -> Result<String, Error> {
        // Clone the p3d_params into separate variables algo, sect, and grid
        let P3dParams { algo, sect, grid } = self.p3d_params.clone();
    
        // Create a HashSet to store processed hashes
        let mut processed_hashes: HashSet<H256> = HashSet::new();
    
        // Take ownership of the mining_params by cloning it from the locked cur_state
        let mining_params = {
            let params_lock = self.cur_state.lock().unwrap();
            if let Some(mp) = (*params_lock).clone() {
                mp
            } else {
                return Err(Error::Custom("Mining params not available".into()));
            }
        };
    
        // Destructure the mining_params into separate variables
        let MiningParams {
            pre_hash,
            parent_hash,
            win_difficulty,
            pow_difficulty,
            pub_key,
        } = mining_params;
    
        // Create new variables win_dfclty and dfclty with values from win_difficulty and pow_difficulty
        let win_dfclty = win_difficulty;
        let dfclty = pow_difficulty;
    
        // Parse the input hash as H256
        let hash = H256::from_str(&hash).unwrap();
    
        // Create a Payload struct with various fields initialized using the cloned values
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
    
        // Start a loop
        loop {
            // Determine the value of rot_hash based on algo
            let rot_hash = match &algo {
                AlgoType::Grid2dV3_1 => pre_hash.clone(),
                _ => parent_hash.clone(),
            };
    
            // Take the first 4 bytes of rot_hash and convert them to a fixed-size array or None
            let rot = rot_hash.encode()[0..4].try_into().ok();
    
            // Create a MiningObj struct with obj field initialized using the cloned payload.obj
            let mining_obj: MiningObj = MiningObj {
                obj: payload.obj.clone(),
            };
    
            // Call the p3d_process function with relevant arguments to get res_hashes
            let res_hashes = p3d_process(
                mining_obj.obj.as_slice(),
                algo.as_p3d_algo(),
                grid as i16,
                sect as i16,
                rot,
            );
    
            // Match the result of res_hashes
            let (_first_hash, obj_hash, poscan_hash) = match res_hashes {
                Ok(hashes) if !hashes.is_empty() => {
                    // Clone the first hash from hashes and parse it as H256
                    let first_hash = hashes[0].clone();
                    let obj_hash = H256::from_str(&first_hash).unwrap();
    
                    // Check if obj_hash is already processed; if so, continue looping
                    if processed_hashes.contains(&obj_hash) {
                        continue;
                    }
    
                    // Calculate the poscan_hash using DoubleHash structure
                    let poscan_hash = DoubleHash { pre_hash, obj_hash }.calc_hash();
                    // Insert obj_hash into processed_hashes set
                    processed_hashes.insert(obj_hash.clone());
    
                    // Return the first_hash, obj_hash, and poscan_hash as a tuple
                    (first_hash, obj_hash, poscan_hash)
                }
                _ => {
                    // If res_hashes is an Err or empty, continue looping
                    continue;
                }
            };
    
            // Iterate over difficulty values [dfclty, win_dfclty]
            for difficulty in [dfclty, win_dfclty] {
                // Create a Compute struct with relevant fields
                let comp = Compute {
                    difficulty,
                    pre_hash,
                    poscan_hash,
                };
    
                // Calculate the hash difficulty using the get_work method of comp
                let diff = get_hash_difficulty(&comp.get_work());
    
                if diff >= difficulty {
                    // Convert the payload to JSON string
                    let message = serde_json::to_string(&payload).unwrap();
    
                    // Create a cryptographically secure PRNG from obj_hash
                    let mut csprng = StdRng::from_seed(obj_hash.encode().try_into().unwrap());
                    
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
    
                    // Print the hash and difficulty information
                    println!(
                        "💎 Hash > Pool Difficulty: {} > {} (win: {})",
                        Style::new().bold().paint(format!("{:.2}", &diff)),
                        &dfclty,
                        &win_dfclty,
                    );
    
                    // Make the RPC request to poscan_pushMiningObjectToPool method
                    let _response: JsonValue = self
                        .client
                        .request("poscan_pushMiningObjectToPool", params)
                        .await
                        .unwrap();
    
                    // Check the response value and print appropriate messages
                    if _response == 0 {
                        println!(
                            "{}",
                            Style::new().bold().paint(format!("✅ Share accepted"))
                        );
                    } else {
                        println!("{}", Style::new().bold().paint("⛔ Share Rejected"));
                    }
                }
                // Break the loop after processing one difficulty value
                break;
            }
            // Break the outer loop after processing one set of res_hashes
            break;
        }
    
        // Return an Ok result with a string indicating "Proposed to Pool"
        Ok("Proposed to Pool".into())
    }

    fn sign(&self, msg: &[u8]) -> Signature {
        // Define a constant CTX as a byte array with the value "Mining pool"
        const CTX: &[u8] = b"Mining pool";
    
        // Call the `sign_simple` method on the `self.key` object (a private key)
        // Pass in the CTX, the message (msg), and the public key derived from the private key
        self.key.sign_simple(CTX, msg, &self.key.to_public())
    }
}

fn get_hash_difficulty(hash: &H256) -> U256 {
    // Convert the hash to a U256 number
    let num_hash = U256::from(&hash[..]);
    
    // Get the maximum value of U256
    let max = U256::max_value();
    
    // Calculate the difficulty by dividing the max value by the num_hash
    max / num_hash
}