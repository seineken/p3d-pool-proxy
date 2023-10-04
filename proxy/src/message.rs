use serde::{Serialize, Deserialize};
use uuid::Uuid;


#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub channel: String,
    pub payload: StatsPayload,
}

impl Message {
    pub fn new (
        member_id: String,
        payload: StatsPayload,
    ) -> Message {
        Message {
            id: Message::generate_id(),
            channel: String::from(member_id),
            payload
        }
    }

    fn generate_id() -> String {
        Uuid::new_v4().to_string()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatsPayload {
    pub name: String,
    pub cores: String,
    pub tag: String,
    pub hashrate: String,
    pub good_hashrate: String,    
}