extern crate asset_registry;
extern crate stderrlog;
extern crate structopt;
#[macro_use]
extern crate log;
extern crate base64;
#[macro_use]
extern crate failure;

use bitcoin_hashes::{
    hex::{FromHex, ToHex},
    sha256d,
};
use elements::{AssetId, OutPoint};
use serde_json::Value;
use structopt::StructOpt;

use asset_registry::asset::{format_sig_msg, Asset, AssetFields};
use asset_registry::errors::{OptionExt, Result, ResultExt};
use asset_registry::util::TxInput;

#[derive(StructOpt, Debug)]
struct Cli {
    #[structopt(
        short = "v",
        long = "verbose",
        parse(from_occurrences),
        help = "Increase verbosity (up to 3)"
    )]
    verbose: usize,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(StructOpt, Debug)]
enum Command {
    #[structopt(name = "make-sig-message", about = "Prepare signed message format")]
    MakeSigMessage {
        #[structopt(long = "asset-id", parse(try_from_str = "AssetId::from_hex"))]
        asset_id: AssetId,
        #[structopt(flatten)]
        fields: AssetFields,
    },

    #[structopt(
        name = "make-submission",
        about = "Prepare asset submission to registry"
    )]
    MakeSubmission {
        #[structopt(long = "asset-id", parse(try_from_str = "AssetId::from_hex"))]
        asset_id: AssetId,

        #[structopt(flatten)]
        fields: AssetFields,

        #[structopt(
            long = "issuance-txid",
            parse(try_from_str = "sha256d::Hash::from_hex")
        )]
        issuance_txid: sha256d::Hash,

        #[structopt(
            long = "issuance-prevout",
            help = "Outpoint used for asset issuance in txid:vout format",
            parse(try_from_str = "parse_outpoint")
        )]
        issuance_prevout: OutPoint,

        #[structopt(long)]
        contract: Value,

        #[structopt(long)]
        signature: String,

        #[structopt(long)]
        verify: bool,
    },

    #[structopt(name = "verify-asset", about = "Verify asset associations")]
    VerifyAsset {
        #[structopt(
            long,
            help = "exit with an error code if any of the veritifications fail"
        )]
        fail: bool,
        jsons: Vec<String>,
    },

    #[structopt(name = "register-asset", about = "Send asset to registry")]
    RegisterAsset {
        #[structopt(short, long = "registry-url")]
        registry_url: String,
        json: String,
    },
}

fn parse_outpoint(arg: &str) -> Result<OutPoint> {
    let mut s = arg.split(":");

    Ok(OutPoint {
        txid: sha256d::Hash::from_hex(s.next().req()?)?,
        vout: s.next().req()?.parse()?,
    })
}

fn main() -> Result<()> {
    let args = Cli::from_args();
    stderrlog::new().verbosity(args.verbose + 2).init().unwrap();
    debug!("cli args: {:?}", args);

    match args.cmd {
        Command::MakeSigMessage { asset_id, fields } => {
            let msg = format_sig_msg(&asset_id, &fields);
            println!("{}", msg);
        }

        Command::MakeSubmission {
            asset_id,
            fields,
            issuance_txid,
            issuance_prevout,
            contract,
            signature,
            verify,
        } => {
            let signature = base64::decode(&signature).context("invalid signature base64")?;
            let asset = Asset {
                asset_id,
                fields,
                issuance_tx: TxInput {
                    txid: issuance_txid,
                    vin: 0,
                }, // TODO
                issuance_prevout,
                contract,
                signature,
            };

            println!("{}", serde_json::to_string(&asset)?);

            if verify {
                // TODO verify with ChainQuery
                asset.verify(None)?;
                info!("Asset verified successfully");
            }
        }

        Command::VerifyAsset { fail, jsons } => {
            // always fail if we have a single json
            let fail = fail || jsons.len() == 1;
            for json in jsons {
                let asset: Asset = serde_json::from_str(&json).context("invalid asset json")?;
                debug!("verifying asset: {:?}", asset);

                match asset.verify(None) {
                    Ok(()) => println!("{},true", asset.id().to_hex()),
                    Err(err) => {
                        warn!("asset verification failed: {:}", err);
                        println!("{},false,\"{}\"", asset.id().to_hex(), err.to_string());
                        ensure!(!fail, "failed verifying asset, aborting");
                    }
                }
            }
        }

        Command::RegisterAsset { registry_url, json } => {
            let asset: Asset = serde_json::from_str(&json).context("invalid asset json")?;
            let client = reqwest::Client::new();
            let mut resp = client.post(&registry_url).json(&asset).send()?;

            if resp.status() != reqwest::StatusCode::OK {
                error!("invalid reply from registry: {:#?}", resp);
                error!("{}", resp.text()?);
                bail!("asset registeration failed")
            }

            info!("asset submitted to registry: {:?}", asset);
        }
    }

    Ok(())
}
