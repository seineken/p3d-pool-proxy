use ansi_term::Style;
use indicatif::ProgressBar;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::core::{Error, JsonValue};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::rpc_params;
use redis::Commands;
use std::result::Result;
use std::thread::sleep;
use std::time::Duration;

use crate::message::{StatsPayload, Message};
use crate::utils::{log, connect};

pub struct SoloAppContex {
    pub(crate) proxy_address: String,

    pub(crate) client: HttpClient,
}

impl SoloAppContex {
    pub(crate) async fn new(
        node_addr: &str,
        proxy_address: String,
    ) -> anyhow::Result<Self> {        
        Ok(SoloAppContex {
            proxy_address,
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

    pub(crate) async fn push_to_node(&self, obj: String, hash: String) -> Result<String, Error> {
        let params = rpc_params![
            serde_json::json!(serde_json::json!(obj)),
            serde_json::json!(serde_json::json!(hash))
        ];
        let _response: JsonValue = self
            .client
            .request("poscan_pushMiningObject", params)
            .await
            .unwrap();

        if _response == 0 {
            let message = format!("✅ Block found and proposed to the chain");
            log(message.clone());
            Ok(message)
        } else {
            let message = format!("{}","⛔ Block Rejected");
            log(message.clone());
            return Err(Error::Custom(message));
        }
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

        // let message = Message::new(String::from("123456"), payload);
        // let response = self
        //     .publish_message(message)
        //     .map_err(|e| e.to_string())
        //     .unwrap();
        Ok(String::from("WIP"))
    }    

    fn publish_message(&self, message: Message) -> Result<String, Error> {
        let mut con = connect();
        let payload = serde_json::to_string(&message)?;
        let result: u64 = con
            .publish(message.channel, payload)
            .map_err(|e| e)
            .unwrap();
        let response = format!("{:?}", result);
        Ok(response)
    }

    fn update_interface(pb: ProgressBar, message: String) {
        loop {
            pb.set_message(&format!(
                "{}",
                Style::new()
                    .bold()
                    .paint(message.clone()))
            );
            sleep(Duration::from_millis(100));
        }
    }
}