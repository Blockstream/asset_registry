extern crate asset_registry;
extern crate stderrlog;
extern crate structopt;
#[macro_use]
extern crate log;
extern crate base64;
#[macro_use]
extern crate failure;

use bitcoin_hashes::{hex::ToHex, sha256, Hash};
use serde_json::Value;
use structopt::StructOpt;

use asset_registry::asset::{Asset, AssetRequest};
use asset_registry::chain::ChainQuery;
use asset_registry::errors::{join_err, Result, ResultExt};

#[derive(StructOpt, Debug)]
struct Cli {
    #[structopt(
        short = "v",
        long = "verbose",
        parse(from_occurrences),
        help = "Increase verbosity (up to 3 times)"
    )]
    verbose: usize,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(StructOpt, Debug)]
enum Command {
    #[structopt(name = "verify-asset", about = "Verify asset associations")]
    VerifyAsset {
        #[cfg_attr(
            feature = "cli",
            structopt(
                short,
                long = "esplora-url",
                help = "url for querying chain state using the esplora api"
            )
        )]
        esplora_url: Option<String>,

        jsons: Vec<String>,
    },

    #[structopt(name = "register-asset", about = "Send asset to registry")]
    RegisterAsset {
        #[structopt(short, long = "registry-url", default_value = "https://assets.blockstream.info")]
        registry_url: String,

        #[structopt(flatten)]
        asset_req: AssetRequest,
    },

    #[structopt(
        name = "contract-json",
        about = "print contract json in canonical serialization (sorted)"
    )]
    ContractJson {
        json: String,
        #[structopt(short, long, help = "print contract hash (sha256)")]
        hash: bool,
    },
}

fn main() -> Result<()> {
    let args = Cli::from_args();
    stderrlog::new().verbosity(args.verbose + 2).init().unwrap();
    debug!("cli args: {:?}", args);

    match args.cmd {
        Command::VerifyAsset {
            esplora_url,
            jsons,
        } => {
            let chain = esplora_url.map(ChainQuery::new);
            let mut failed = false;

            for json in jsons {
                let asset: Asset = serde_json::from_str(&json).context("invalid asset json")?;
                debug!("verifying asset: {:?}", asset);

                match asset.verify(chain.as_ref()) {
                    Ok(()) => println!("{},true", asset.id().to_hex()),
                    Err(err) => {
                        warn!("asset verification failed: {}", join_err(&err));
                        println!("{},false", asset.id().to_hex());
                        failed = true;
                    }
                }
            }

            if failed {
                std::process::exit(1);
            }
        }

        Command::RegisterAsset {
            registry_url,
            asset_req,
        } => {
            info!("submiting to registry: {:#?}", asset_req);

            let client = reqwest::Client::new();
            let mut resp = client.post(&registry_url).json(&asset_req).send()?;
            if resp.status() != reqwest::StatusCode::CREATED {
                error!("invalid reply from registry: {:#?}", resp);
                error!("{}", resp.text()?);
                bail!("asset registeration failed")
            }

            let asset: Asset = resp.json()?;

            info!("registered succesfully: {:#?}", asset);
        }

        Command::ContractJson { json, hash } => {
            let contract: Value = serde_json::from_str(&json).context("invalid contract json")?;
            let contract_str = serde_json::to_string(&contract)?;

            if hash {
                let mut hash = sha256::Hash::hash(&contract_str.as_bytes()).into_inner();
                // reverse the hash to match the format expected by elementsd for the contract_hash
                hash.reverse();
                println!("{}", hex::encode(hash));
            } else {
                println!("{}", contract_str);
            }
        }
    }

    Ok(())
}
