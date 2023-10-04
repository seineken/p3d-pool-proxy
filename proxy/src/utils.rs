use std::env;

use ansi_term::Style;
use chrono::Local;


pub(crate) fn log(message: String) {
    let timestamp = Local::now();
    let formatted_timestamp = timestamp.format("%Y-%m-%d %H:%M:%S").to_string();
    println!(
        "[{}]: {}",
        formatted_timestamp,
        Style::new().bold().paint(format!("{}", message))
    );
}

pub(crate) fn connect() -> redis::Connection {
    //format - host:port
    let redis_host_name =
        env::var("REDIS_HOSTNAME").expect("missing environment variable REDIS_HOSTNAME");
    
    let redis_password = env::var("REDIS_PASSWORD").unwrap_or_default();
    //if Redis server needs secure connection
    let uri_scheme = match env::var("IS_TLS") {
        Ok(_) => "rediss",
        Err(_) => "redis",
    };
    let redis_conn_url = format!("{}://:{}@{}", uri_scheme, redis_password, redis_host_name);
    redis::Client::open(redis_conn_url)
        .expect("Invalid connection URL")
        .get_connection()
        .expect("failed to connect to Redis")
}