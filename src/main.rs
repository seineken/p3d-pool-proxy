use ansi_term::{Colour, Style};
use bip39::{Language, Mnemonic};
use pool_handler::AppContex;
use std::{env, process::Command, sync::Arc, thread::sleep, time::Duration};
use structopt::StructOpt;
use substrate_bip39::mini_secret_from_entropy;

use crate::worker::P3dParams;

mod message;
mod pool_handler;
mod pool_rpc;
mod stats_rpc;
mod utils;
mod worker;

#[derive(Debug, StructOpt)]
enum SubCommand {
    #[structopt(name = "run", about = "Use run to start the pool proxy")]
    Run(RunOptions),
    #[structopt(name = "inspect", about = "Use inspect to convert seed to key")]
    Inspect(InspectOptions),
}

#[derive(Debug, StructOpt)]
struct RunOptions {
    /// 3d hash algorithm
    #[structopt(default_value = "grid2d_v3.1", short = "l", long = "algo")]
    /// Mining algorithm. Supported algorithm: grid2d_v3.1
    algo: String,

    #[structopt(default_value = "0.0.0.0:3336", short = "a", long = "proxy-address")]
    /// Pool proxy address
    proxy_address: String,

    #[structopt(
    default_value = "http://127.0.0.1:9933",
    short = "n",
    long = "node-url"
    )]
    /// Node url
    node_url: String,

    #[structopt(short = "p", long = "pool-id", required_if("proxy-mode", "pool"))]
    /// Pool id
    pool_id: Option<String>,
}

#[derive(Debug, StructOpt)]
struct InspectOptions {
    #[structopt(short, long)]
    /// Seed phrase
    seed: String,
}

#[derive(StructOpt)]
struct Cli {
    #[structopt(subcommand)]
    cmd: SubCommand,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::from_args();

    match args.cmd {
        SubCommand::Inspect(opt) => {
            let mnemonic = Mnemonic::from_phrase(&opt.seed, Language::English);
            match mnemonic {
                Ok(mnemonic) => match mini_secret_from_entropy(mnemonic.entropy(), "") {
                    Ok(mini_key) => println!("{}", hex::encode(mini_key.to_bytes())),
                    Err(e) => println!("{:?}", e),
                },
                Err(e) => println!("{:?}", e),
            };
            Ok(())
        }
        SubCommand::Run(opt) => {
            clear_console();

            const VERSION: &str = env!("CARGO_PKG_VERSION");

            println!(
                "{}",
                format!(
                    "{}",
                    Style::new()
                        .bold()
                        .fg(Colour::Green)
                        .paint(format!("📱 P3D Pool Proxy v{}\n", String::from(VERSION)))
                )
            );

            let p3d_params = P3dParams::new(opt.algo.as_str());
            let mongo_url = env::var("MONGO_URL").expect("MONGO_URL must be set");

            let pool_ctx = AppContex::new(
                p3d_params,
                opt.node_url.as_str(),
                opt.proxy_address.clone(),
                opt.pool_id.clone().unwrap(),
                mongo_url.as_str(),
            )
                .await?;

            let ctx = Arc::new(pool_ctx);
            let _server_addr = worker::pool_rpc_server(ctx.clone()).await?;

            println!(
                "{}",
                format!("💻  Running        :: http://{}", _server_addr)
            );
            println!(
                "{}",
                format!("🌀  Mode           :: {}", String::from("POOL"))
            );
            println!(
                "{}",
                format!("🆔  Pool Id        :: {}", opt.pool_id.clone().unwrap())
            );

            let stats_server_address =
                worker::run_stats_server(String::from("0.0.0.0:3533")).await?;
            let _stats_ws_address = format!("{}", stats_server_address);

            println!(
                "{}",
                format!("💻  Stats server   :: http://{}", _stats_ws_address)
            );
            // std::thread::spawn(move || ctx.adjust_difficulty());

            futures::future::pending().await
        }
    }
}

fn clear_console() {
    if cfg!(target_os = "windows") {
        // Comando para limpiar la consola en Windows
        let _ = Command::new("cmd").arg("/c").arg("cls").status();
    } else {
        // Comando para limpiar la consola en sistemas Unix
        let _ = Command::new("sh").arg("-c").arg("clear").status();
    }

    // Espera breve para dar tiempo a que se vea la pantalla limpia
    sleep(Duration::from_millis(100));
}
