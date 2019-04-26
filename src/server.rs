use std::path::Path;

use bitcoin_hashes::{hex::FromHex, sha256d};
use rocket::State;
use rocket_contrib::json::{Json, JsonValue};

use crate::asset::{Asset, AssetRegistry};
use crate::errors::Result;

#[get("/")]
fn list(registry: State<AssetRegistry>) -> JsonValue {
    json!(registry.list())
}

#[get("/<id>")]
fn get(id: String, registry: State<AssetRegistry>) -> Result<Option<JsonValue>> {
    let id = sha256d::Hash::from_hex(&id)?;
    Ok(registry.get(&id).map(|asset| json!(asset)))
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
