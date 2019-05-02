use std::path::PathBuf;

use bitcoin_hashes::hex::FromHex;
use elements::AssetId;
use rocket::{http, State};
use rocket_contrib::json::Json;
#[cfg(feature = "cli")]
use structopt::StructOpt;

use crate::asset::Asset;
use crate::chain::ChainQuery;
use crate::errors::Result;
use crate::registry::Registry;

#[get("/<id>")]
fn get(id: String, registry: State<Registry>) -> Result<Option<Json<Asset>>> {
    let id = AssetId::from_hex(&id)?;
    Ok(registry.load(&id)?.map(Json))
}

#[post("/", format = "application/json", data = "<asset>")]
fn update(asset: Json<Asset>, registry: State<Registry>) -> Result<http::Status> {
    debug!("write asset: {:?}", asset);

    registry.write(asset.into_inner())?;

    Ok(http::Status::NoContent)
}

#[derive(Debug)]
#[cfg_attr(feature = "cli", derive(StructOpt))]
pub struct Config {
    #[cfg_attr(
        feature = "cli",
        structopt(
            short,
            long,
            parse(from_occurrences),
            help = "Increase verbosity (up to 3)"
        )
    )]
    verbose: usize,

    #[cfg_attr(
        feature = "cli",
        structopt(short, long = "db-path", help = "Path to database directory")
    )]
    db_path: PathBuf,

    #[cfg_attr(
        feature = "cli",
        structopt(
            short,
            long = "hook-cmd",
            help = "Hook script to run after every registry update"
        )
    )]
    hook_cmd: Option<String>,

    #[cfg_attr(
        feature = "cli",
        structopt(
            short,
            long = "esplora-url",
            help = "url for querying chain state using the esplora api"
        )
    )]
    esplora_url: String,
}

pub fn start_server(config: Config) -> Result<rocket::Rocket> {
    info!("Starting Rocket web server with config: {:?}", config);

    #[allow(unused_must_use)]
    stderrlog::new().verbosity(config.verbose + 2).init();

    let chain = ChainQuery::new(config.esplora_url);
    let registry = Registry::new(&config.db_path, chain, config.hook_cmd);

    Ok(rocket::ignite()
        .manage(registry)
        .mount("/", routes![get, update]))
}

// needs to be run with --test-threads 1
#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::tests::spawn_mock_esplora_server;
    use crate::entity::tests::spawn_mock_verifier_server;
    use crate::errors::OptionExt;
    use bitcoin_hashes::hex::ToHex;
    use rocket::local::{Client, LocalResponse};

    lazy_static! {
        static ref CLIENT: Client = {
            let config = Config {
                verbose: 1,
                hook_cmd: None,
                esplora_url: "http://localhost:58713".to_string(),
                db_path: std::env::temp_dir()
                    .join(format!("asset-registry-testdb-{}", std::process::id())),
            };

            std::fs::create_dir_all(&config.db_path).unwrap();

            let server = start_server(config).unwrap();
            Client::new(server).unwrap()
        };
    }

    fn parse_json<T: for<'a> serde::de::Deserialize<'a>>(mut resp: LocalResponse) -> Result<T> {
        let body = resp.body_string().or_err("missing body")?;
        Ok(serde_json::from_str(&body)?)
    }

    #[test]
    fn test0_init() {
        stderrlog::new().verbosity(3).init();
        spawn_mock_verifier_server();
        spawn_mock_esplora_server();
    }

    #[test]
    fn test2_update() -> Result<()> {
        let resp = CLIENT.post("/")
            .header(http::ContentType::JSON)
            .body(r#"{
                "asset_id": "9a51761132b7399d34819c2c5d03af71794ff3aa0f78a434ddf20605545c86f2",
                "issuance_txin": {"txid":"77f21099c47646b30a9978a1a39acf658f6eb9bd68f677d23f132c587bb93836", "vin":0},
                "issuance_prevout":{"txid":"8e818b4561de8c731db7cd7a3b67784d525f96ecc7b564b82d8a01cab390b2d4","vout":1},
                "contract": {"issuer_pubkey":"026be637f97bc191c27522577bd6fe284b54404321652fcc4eb62aa0f4cfd6d172"},
                "name": "Foo Coin",
                "ticker": "FOO",
                "precision": 3,
                "entity": { "domain": "test.dev" },
                "signature": "IAbn0kr44f8+HJI/qpNaXvU48b/L9mBZUli197Okg5BVYXin3xA1ilbxAvHZ00BL/0+3URIuVtAeqkl7WxWmuhY="
            }"#)
            .dispatch();
        assert_eq!(resp.status(), http::Status::NoContent);
        Ok(())
    }

    #[test]
    fn test3_update_invalid_sig() -> Result<()> {
        let resp = CLIENT.post("/")
            .header(http::ContentType::JSON)
            .body(r#"{
                "asset_id": "9a51761132b7399d34819c2c5d03af71794ff3aa0f78a434ddf20605545c86f2",
                "issuance_txin": {"txid":"77f21099c47646b30a9978a1a39acf658f6eb9bd68f677d23f132c587bb93836", "vin":0},
                "issuance_prevout":{"txid":"8e818b4561de8c731db7cd7a3b67784d525f96ecc7b564b82d8a01cab390b2d4","vout":1},
                "contract": {"issuer_pubkey":"026be637f97bc191c27522577bd6fe284b54404321652fcc4eb62aa0f4cfd6d172"},
                "name": "Foo Coin",
                "ticker": "FOX",
                "precision": 3,
                "entity": { "domain": "test.dev" },
                "signature": "IAbn0kr44f8+HJI/qpNaXvU48b/L9mBZUli197Okg5BVYXin3xA1ilbxAvHZ00BL/0+3URIuVtAeqkl7WxWmuhY="
            }"#)
            .dispatch();
        assert_ne!(resp.status(), http::Status::Ok);
        assert_ne!(resp.status(), http::Status::NoContent);
        Ok(())
    }

    #[test]
    fn test4_get() -> Result<()> {
        let resp = CLIENT
            .get("/9a51761132b7399d34819c2c5d03af71794ff3aa0f78a434ddf20605545c86f2")
            .dispatch();
        let asset: Asset = parse_json(resp)?;
        debug!("asset: {:?}", asset);

        assert_eq!(
            asset.id().to_hex(),
            "9a51761132b7399d34819c2c5d03af71794ff3aa0f78a434ddf20605545c86f2"
        );
        assert_eq!(asset.name(), "Foo Coin");
        Ok(())
    }
}
