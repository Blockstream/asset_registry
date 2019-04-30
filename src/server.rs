use std::collections::HashMap;
use std::path::Path;

use bitcoin_hashes::hex::FromHex;
use elements::AssetId;
use rocket::{http, State};
use rocket_contrib::json::Json;

use crate::asset::Asset;
use crate::errors::Result;
use crate::registry::Registry;

#[get("/")]
fn list(registry: State<Registry>) -> Json<HashMap<AssetId, Asset>> {
    Json(registry.list())
}

#[get("/<id>")]
fn get(id: String, registry: State<Registry>) -> Result<Option<Json<Asset>>> {
    let id = AssetId::from_hex(&id)?;
    Ok(registry.get(&id).map(Json))
}

#[post("/", format = "application/json", data = "<asset>")]
fn update(asset: Json<Asset>, registry: State<Registry>) -> Result<http::Status> {
    debug!("write asset: {:?}", asset);

    registry.write(asset.into_inner())?;

    Ok(http::Status::NoContent)
}

pub fn start_server(db_path: &Path, hook_cmd: Option<String>) -> Result<rocket::Rocket> {
    let registry = Registry::load(db_path, hook_cmd)?;

    info!("Starting Rocket web server with registry: {:?}", registry);

    Ok(rocket::ignite()
        .manage(registry)
        .mount("/", routes![list, get, update]))
}

// needs to be run with --test-threads 1
#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::tests::spawn_verifier_server;
    use crate::errors::OptionExt;
    use bitcoin_hashes::hex::ToHex;
    use rocket::local::{Client, LocalResponse};
    use std::collections::HashMap;

    lazy_static! {
        static ref CLIENT: Client = {
            let db_path =
                std::env::temp_dir().join(format!("asset-registry-testdb-{}", std::process::id()));
            std::fs::create_dir_all(&db_path).unwrap();

            let server = start_server(&db_path, None).unwrap();
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
        spawn_verifier_server();
    }

    #[test]
    fn test1_list_empty() -> Result<()> {
        let resp = CLIENT.get("/").dispatch();
        let assets: HashMap<AssetId, Asset> = parse_json(resp)?;
        ensure!(assets.len() == 0, "shouldn't have assets yet");
        Ok(())
    }

    #[test]
    fn test2_update() -> Result<()> {
        let resp = CLIENT.post("/")
            .header(http::ContentType::JSON)
            .body(r#"{
                "asset_id": "9a51761132b7399d34819c2c5d03af71794ff3aa0f78a434ddf20605545c86f2",
                "issuance_txid": "0a93069bba360df60d77ecfff99304a9de123fecb8217348bb9d35f4a96d2fca",
                "issuance_prevout":{"txid":"8e818b4561de8c731db7cd7a3b67784d525f96ecc7b564b82d8a01cab390b2d4","vout":1},
                "contract": "{\"issuer_pubkey\":\"026be637f97bc191c27522577bd6fe284b54404321652fcc4eb62aa0f4cfd6d172\"}",
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
                "issuance_txid": "0a93069bba360df60d77ecfff99304a9de123fecb8217348bb9d35f4a96d2fca",
                "issuance_prevout":{"txid":"8e818b4561de8c731db7cd7a3b67784d525f96ecc7b564b82d8a01cab390b2d4","vout":1},
                "contract": "{\"issuer_pubkey\":\"026be637f97bc191c27522577bd6fe284b54404321652fcc4eb62aa0f4cfd6d172\"}",
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

    #[test]
    fn test5_list_with_asset() -> Result<()> {
        let resp = CLIENT.get("/").dispatch();
        let assets: HashMap<AssetId, Asset> = parse_json(resp)?;
        debug!("assets: {:?}", assets);

        assert!(assets.len() == 1, "should have one asset");

        let asset_id = assets.keys().next().unwrap();
        assert_eq!(
            asset_id.to_hex(),
            "9a51761132b7399d34819c2c5d03af71794ff3aa0f78a434ddf20605545c86f2"
        );
        assert_eq!(assets.get(asset_id).unwrap().name(), "Foo Coin");

        Ok(())
    }

}
