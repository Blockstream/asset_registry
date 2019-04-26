use std::collections::HashMap;
use std::path::Path;

use bitcoin_hashes::{hex::FromHex, sha256d};
use rocket::State;
use rocket_contrib::json::Json;

use crate::asset::{Asset, AssetRegistry};
use crate::errors::Result;

#[get("/")]
fn list(registry: State<AssetRegistry>) -> Json<HashMap<sha256d::Hash, Asset>> {
    Json(registry.list())
}

#[get("/<id>")]
fn get(id: String, registry: State<AssetRegistry>) -> Result<Option<Json<Asset>>> {
    let id = sha256d::Hash::from_hex(&id)?;
    Ok(registry.get(&id).map(Json))
}

#[post("/", format = "application/json", data = "<asset>")]
fn update(asset: Json<Asset>, registry: State<AssetRegistry>) -> Result<()> {
    debug!("write asset: {:?}", asset);

    registry.write(asset.into_inner())
}

pub fn start_server(db_path: &Path) -> Result<rocket::Rocket> {
    let registry = AssetRegistry::load(db_path)?;

    info!("Starting Rocket web server with registry: {:?}", registry);

    Ok(rocket::ignite()
        .manage(registry)
        .mount("/", routes![list, get, update]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::OptionExt;
    use bitcoin_hashes::hex::ToHex;
    use rocket::local::{Client, LocalResponse};
    use std::collections::HashMap;

    lazy_static! {
        static ref CLIENT: Client = {
            let db_path =
                std::env::temp_dir().join(format!("asset-registry-testdb-{}", std::process::id()));
            std::fs::create_dir_all(&db_path).unwrap();

            let server = start_server(&db_path).unwrap();
            Client::new(server).unwrap()
        };
    }

    fn parse_json<T: for<'a> serde::de::Deserialize<'a>>(mut resp: LocalResponse) -> Result<T> {
        let body = resp.body_string().or_err("missing body")?;
        Ok(serde_json::from_str(&body)?)
    }

    #[test]
    fn test0_init() {
        stderrlog::new().verbosity(3).init(); //.unwrap();
    }

    #[test]
    fn test1_list_empty() -> Result<()> {
        let resp = CLIENT.get("/").dispatch();
        let assets: HashMap<sha256d::Hash, Asset> = parse_json(resp)?;
        ensure!(assets.len() == 0, "shouldn't have assets yet");
        Ok(())
    }

    #[test]
    fn test2_update() -> Result<()> {
        let resp = CLIENT.post("/")
            .header(rocket::http::ContentType::JSON)
            .body(r#"{
                "asset_id": "5a273edc116adeacc13a7e8c4e987d31385db05c411c465df91bac4cf3aa0504",
                "issuance_txid": "0a93069bba360df60d77ecfff99304a9de123fecb8217348bb9d35f4a96d2fca",
                "contract": "{\"issuer_pubkey\":\"026be637f97bc191c27522577bd6fe284b54404321652fcc4eb62aa0f4cfd6d172\"}",
                "name": "Foo Coin",
                "ticker": "FOO",
                "precision": 3,
                "entity": { "domain": "foo.com" },
                "signature": "H5P8HDEUBlZUAqp7M+v6N5sakwbFlm0XSioTMwAizBkyMt82uK7EwdzDugP9Z1KbYkllQiHUO8Y0F5EiEMF/NyY="
            }"#)
            .dispatch();
        assert_eq!(resp.status(), rocket::http::Status::Ok);
        Ok(())
    }

    #[test]
    fn test3_update_invalid_sig() -> Result<()> {
        let resp = CLIENT.post("/")
            .header(rocket::http::ContentType::JSON)
            .body(r#"{
                "asset_id": "5a273edc116adeacc13a7e8c4e987d31385db05c411c465df91bac4cf3aa0504",
                "issuance_txid": "0a93069bba360df60d77ecfff99304a9de123fecb8217348bb9d35f4a96d2fca",
                "contract": "{\"issuer_pubkey\":\"026be637f97bc191c27522577bd6fe284b54404321652fcc4eb62aa0f4cfd6d172\"}",
                "name": "Foo Coin",
                "ticker": "FOX",
                "precision": 3,
                "entity": { "domain": "foo.com" },
                "signature": "H5P8HDEUBlZUAqp7M+v6N5sakwbFlm0XSioTMwAizBkyMt82uK7EwdzDugP9Z1KbYkllQiHUO8Y0F5EiEMF/NyY="
            }"#)
            .dispatch();
        assert_ne!(resp.status(), rocket::http::Status::Ok);
        Ok(())
    }

    #[test]
    fn test4_get() -> Result<()> {
        let resp = CLIENT
            .get("/5a273edc116adeacc13a7e8c4e987d31385db05c411c465df91bac4cf3aa0504")
            .dispatch();
        let asset: Asset = parse_json(resp)?;
        debug!("asset: {:?}", asset);

        assert_eq!(
            asset.id().to_hex(),
            "5a273edc116adeacc13a7e8c4e987d31385db05c411c465df91bac4cf3aa0504"
        );
        assert_eq!(asset.name(), "Foo Coin");
        Ok(())
    }

    #[test]
    fn test5_list_with_asset() -> Result<()> {
        let resp = CLIENT.get("/").dispatch();
        let assets: HashMap<sha256d::Hash, Asset> = parse_json(resp)?;
        debug!("assets: {:?}", assets);

        assert!(assets.len() == 1, "should have one asset");

        let asset_id = assets.keys().next().unwrap();
        assert_eq!(
            asset_id.to_hex(),
            "5a273edc116adeacc13a7e8c4e987d31385db05c411c465df91bac4cf3aa0504"
        );
        assert_eq!(assets.get(asset_id).unwrap().name(), "Foo Coin");

        Ok(())
    }

}
