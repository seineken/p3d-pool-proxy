use ansi_term::Style;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::core::{Error, JsonValue};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::rpc_params;
use redis::Commands;
use std::result::Result;

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
            let message = format!(
                "{}",
                Style::new().bold().paint(format!("✅ Block found and proposed to the chain"))
            );
            log(message.clone());
            Ok(message)
        } else {
            let message = format!("{}", Style::new().bold().paint("⛔ Block Rejected"));
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

        let message = Message::new(String::from("123456"), payload);
        let response = self
            .publish_message(message)
            .map_err(|e| e.to_string())
            .unwrap();
        Ok(response)
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
}