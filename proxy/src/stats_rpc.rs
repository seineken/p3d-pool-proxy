use jsonrpsee::core::RpcResult;
use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use redis::ErrorKind;
use redis::RedisResult;
use redis::from_redis_value;
use redis::{Commands, Value};

use crate::message::StatsPayload;
use crate::utils::connect;

use redis::FromRedisValue;

#[rpc(server, client)]
pub trait StatsRpc {
	#[method(name = "get_stats")]
	async fn get_stats(
		&self,
        member_id: String,
	) -> RpcResult<String>;
}

pub struct StatsRpcServerImpl;

impl StatsRpcServerImpl {
    pub fn new() -> Self {
        Self
    }
}

// impl FromRedisValue for StatsPayload {
    // fn from_redis_value(value: &Value) -> redis::RedisResult<Self> {
        // match value {
        //     Value::Data(bytes) => {
        //         // Convierte los bytes en una instancia de StatsPayload utilizando serde
        //         let stats_payload: Result<StatsPayload, _> = serde_json::from_slice(bytes);
        //         match stats_payload {
        //             Ok(payload) => Ok(payload),
        //             Err(_) => {
        //                 Err(redis::RedisError::from((
        //                     redis::ErrorKind::ResponseError,
        //                     "Error al deserializar StatsPayload desde Redis",
        //                 )))
        //             }
        //         }


        //         let v: String = from_redis_value(value)?;
        //         if let Some((id, desc)) = v.split_once('-') {
        //             if let Ok(id) = id.parse() {
        //                 Ok(Task {
        //                     id,
        //                     desc: desc.to_string(),
        //                 })
        //             } else {
        //                 Err((ErrorKind::TypeError, "bad first token").into())
        //             }
        //         } else {
        //             Err((ErrorKind::TypeError, "missing dash").into())
        //         }
        //     }
        //     _ => {
        //         Err(redis::RedisError::from((
        //             redis::ErrorKind::ResponseError,
        //             "El valor de Redis no es un Data",
        //         )))
        //     }
        // }
    // }
//     fn from_redis_value(v: &Value) -> RedisResult<Self> {
//         let v: String = from_redis_value(v)?;
//         if let Some((id, stats)) = v.split_once('-') {
//             if let Ok(id) = id.parse() {
//                 let stats_payload: StatsPayload = serde_json::from_str(stats).expect("Fallo el parseo del objeto StatsPayload");
//                 Ok(stats_payload)
//             } else {
//                 Err((ErrorKind::TypeError, "bad first token").into())
//             }
//         } else {
//             Err((ErrorKind::TypeError, "missing dash").into())
//         }
//     }
// }

#[async_trait]
impl StatsRpcServer for StatsRpcServerImpl {
	async fn get_stats(
		&self,
        member_id: String
	) -> RpcResult<String> {
		let mut con = connect();
		let response = con.get(member_id.clone()).map_err(|e| e).unwrap();
		Ok(response)
	}  
}
