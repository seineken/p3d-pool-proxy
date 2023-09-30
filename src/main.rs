use bip39::{Language, Mnemonic};
use rpc::PoolContex;
use std::sync::Arc;
use structopt::StructOpt;
use substrate_bip39::mini_secret_from_entropy;

use crate::rpc::P3dParams;

mod rpc;
mod worker;
mod pool_rpc;

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
    #[structopt(default_value = "grid2d_v3.1", short, long)]
    /// Mining algorithm. Supported algorithm: grid2d_v3.1
    algo: String,

    #[structopt(default_value = "0.0.0.0:3333", short, long)]
    /// Pool proxy url
    pool_url: String,

    #[structopt(default_value = "http://127.0.0.1:9933", short, long)]
    /// Node url
    node_url: String,

    #[structopt(default_value = "d1CVfTXNxP73KXoBf7gbwNnBVF9hqtJJ1ZAxGEfgTdLboj8UV", short, long)]
    /// Pool id
    pool_id: String,

    #[structopt(short, long)]
    /// Member id (wallet)
    member_id: String,

    #[structopt(short, long)]
    /// Member private key to sign requests
    member_key: String,
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
            let p3d_params = P3dParams::new(opt.algo.as_str());
            let ctx = PoolContex::new(
                p3d_params,
                opt.node_url.as_str(),
                opt.pool_url,
                opt.pool_id,
                opt.member_id,
                opt.member_key,
            ).await?;

            let ctx = Arc::new(ctx);
            let server_addr = worker::run_rpc_server(ctx.clone()).await?;
            println!("Pool proxy runing on :: http://{}", server_addr);

            futures::future::pending().await
        }
    }
}