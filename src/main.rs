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
    /// Mining algorithm. Supported algorithms: grid2d, grid2d_v2, grid2d_v3
    algo: String,

    #[structopt(default_value = "0.0.0.0:3334", short, long)]
    /// Pool url
    pool_url: String,

    #[structopt(default_value = "http://pool.3dpassmining.info:9933", short, long)]
    /// Node url
    node_url: String,

    #[structopt(default_value = "d1G2JYmaLeoyDbqAQRD3bfdbNosjAC2bDM6Qkvtjx6iZ3u88Z", short, long)]
    /// Pool id
    pool_id: String,

    #[structopt(default_value = "d1Cz2nkxocd4JmHBtmvM2ysJbB1zGFuaurNimKgJ5rpNUA6Tv", short, long)]
    /// Member id
    member_id: String,

    #[structopt(default_value = "", short, long)]
    /// Member key to sign requests
    key: String,
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

fn init_logging() {
    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .expect("setting default subscriber failed");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();

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
                opt.key,
            ).await?;

            let ctx = Arc::new(ctx);
            tokio::spawn(worker::queue_management(ctx.clone()));

            worker::start_timer(ctx.clone());

            let server_addr = worker::run_rpc_server(ctx.clone()).await?;
            println!("Pool runing on :: http://{}", server_addr);

            futures::future::pending().await
        }
    }
}