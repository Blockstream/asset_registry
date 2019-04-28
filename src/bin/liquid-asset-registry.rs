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
use structopt::StructOpt;

use asset_registry::asset::{format_sig_msg, Asset, AssetFields};
use asset_registry::entity::AssetEntity;
use asset_registry::errors::{Result, ResultExt};

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
        #[structopt(long = "asset-id", parse(try_from_str = "sha256d::Hash::from_hex"))]
        asset_id: sha256d::Hash,
        #[structopt(flatten)]
        fields: AssetArgs,
    },

    #[structopt(
        name = "make-submission",
        about = "Prepare asset submission to registry"
    )]
    MakeSubmission {
        #[structopt(long = "asset-id", parse(try_from_str = "sha256d::Hash::from_hex"))]
        asset_id: sha256d::Hash,
        #[structopt(flatten)]
        fields: AssetArgs,
        #[structopt(
            long = "issuance-txid",
            parse(try_from_str = "sha256d::Hash::from_hex")
        )]
        issuance_txid: sha256d::Hash,
        #[structopt(long)]
        contract: String,

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

#[derive(StructOpt, Debug)]
struct AssetArgs {
    #[structopt(long, help = "Asset name (5-16 characters)")]
    name: String,
    #[structopt(long, help = "Asset ticker (alphanumeric, 3-5 chars)")]
    ticker: Option<String>,
    #[structopt(long, help = "Asset decimal precision (up to 8)")]
    precision: Option<u8>,
    //#[structopt(long, help = "Domain name to associate with the asset")]
    //domain: String,
    #[structopt(
        name = "domain",
        long,
        help = "Domain name to associate with the asset",
        parse(from_str = "parse_domain_entity")
    )]
    entity: AssetEntity,
}

fn parse_domain_entity(domain: &str) -> AssetEntity {
    AssetEntity::DomainName(domain.to_string())
}

impl AssetArgs {
    fn into_fields(self) -> AssetFields {
        let AssetArgs {
            name,
            ticker,
            precision,
            entity,
        } = self;

        AssetFields {
            name,
            ticker,
            precision,
            entity,
        }
    }
}

fn main() -> Result<()> {
    let args = Cli::from_args();
    stderrlog::new().verbosity(args.verbose + 2).init().unwrap();
    debug!("cli args: {:?}", args);

    match args.cmd {
        Command::MakeSigMessage { asset_id, fields } => {
            let fields = fields.into_fields();
            let msg = format_sig_msg(&asset_id, &fields);
            println!("{}", msg);
        }

        Command::MakeSubmission {
            asset_id,
            issuance_txid,
            contract,
            fields,
            signature,
            verify,
        } => {
            let signature = base64::decode(&signature).context("invalid signature")?;
            let fields = fields.into_fields();
            let asset = Asset {
                asset_id,
                issuance_txid,
                contract,
                fields,
                signature,
            };

            println!("{}", serde_json::to_string(&asset)?);

            if verify {
                asset.verify()?;
                info!("Asset verified successfully");
            }
        }

        Command::VerifyAsset { fail, jsons } => {
            // always fail if we have a single json
            let fail = fail || jsons.len() == 1;
            for json in jsons {
                let asset: Asset = serde_json::from_str(&json).context("invalid asset json")?;
                debug!("verifying asset: {:?}", asset);

                match asset.verify() {
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
