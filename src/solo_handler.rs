use ansi_term::Style;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::core::{Error, JsonValue};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::rpc_params;

use std::result::Result;

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
            println!(
                "{}",
                Style::new().bold().paint(format!("✅ Block found and proposed to the chain"))
            );
            Ok("✅ Block found and proposed to the chain".into())
        } else {
            println!("{}", Style::new().bold().paint("⛔ Block Rejected"));
            return Err(Error::Custom("⛔ Block Rejected".into()));
        }
    }

}