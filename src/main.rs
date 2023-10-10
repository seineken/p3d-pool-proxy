use ansi_term::{Colour, Style};
use bip39::{Language, Mnemonic};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use pool_handler::AppContex;
use std::{net::SocketAddr, process::Command, sync::Arc, thread::sleep, time::Duration};
use structopt::StructOpt;
use substrate_bip39::mini_secret_from_entropy;

use crate::{solo_handler::SoloAppContex, worker::P3dParams};

mod message;
mod pool_handler;
mod pool_rpc;
mod solo_handler;
mod solo_rpc;
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

    #[structopt(default_value = "127.0.0.1:3333", short = "a", long = "proxy-address")]
    /// Pool proxy address
    proxy_address: String,

    #[structopt(
        default_value = "http://seineken.ddns.net:9933",
        short = "n",
        long = "node-url"
    )]
    /// Node url
    node_url: String,

    #[structopt(short = "p", long = "pool-id", required_if("proxy-mode", "pool"))]
    /// Pool id
    pool_id: Option<String>,

    #[structopt(short = "m", long = "member-id", required_if("proxy-mode", "pool"))]
    /// Member id (wallet)
    member_id: Option<String>,

    #[structopt(short = "k", long = "member-key", required_if("proxy-mode", "pool"))]
    /// Member private key to sign requests
    member_key: Option<String>,

    #[structopt(short = "o", long = "mode")]
    /// Proxy mode (solo or pool)
    proxy_mode: Option<String>,
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

            let stats_server_address =
                worker::run_stats_server(String::from("0.0.0.0:3533")).await?;
            let _stats_ws_address = format!("http://{}", stats_server_address);

            if let Some(proxy_mode) = opt.proxy_mode {
                if proxy_mode == "solo" {
                    let solo_ctx =
                        SoloAppContex::new(opt.node_url.as_str(), opt.proxy_address.clone())
                            .await?;
                    let solo_ctx = Arc::new(solo_ctx);
                    let server_addr = worker::solo_rpc_server(solo_ctx.clone()).await?;

                    println!("{}",format!(
                        "{}",
                        Style::new()
                            .bold()
                            .paint(format!("{} :: running on http://{}", String::from("🌐 Pool Proxy :: SOLO :: "), server_addr))
                    ));
                } else {
                    let p3d_params = P3dParams::new(opt.algo.as_str());                              

                    let pool_ctx = AppContex::new(
                        p3d_params,
                        opt.node_url.as_str(),
                        opt.proxy_address.clone(),
                        opt.pool_id.clone().unwrap(),
                        opt.member_id.clone().unwrap(),
                        opt.member_key.clone().unwrap(),
                    )
                    .await?;
                    let ctx = Arc::new(pool_ctx);
                    let _server_addr = worker::pool_rpc_server(ctx.clone()).await?;

                    println!("{}",format!(
                        "{}",
                        Style::new()
                            .bold()
                            .paint(format!("{} :: running on http://{}", String::from("🌐 Pool Proxy :: POOL :: "), _server_addr))
                    ));

                    println!("{}",format!("🪪  Member Id      :: {}", opt.member_id.clone().unwrap()));
                    
                    println!("{}",format!("🆔  Pool Id        :: {}", opt.pool_id.clone().unwrap()
                    ));
                }
            } else {
                if opt.pool_id.is_none() || opt.member_id.is_none() || opt.member_key.is_none() {
                    println!(
                        "{}",
                        Style::new().bold().paint(format!(
                            "🚨 POOL mode requires pool-id, member-id and member-key."
                        ))
                    );
                    std::process::exit(1);
                }
            }

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
