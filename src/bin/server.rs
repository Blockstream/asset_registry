#![feature(proc_macro_hygiene, decl_macro)]

extern crate asset_registry;
extern crate stderrlog;
#[macro_use]
extern crate log;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;

use std::path::Path;

use bitcoin_hashes::{hex::FromHex, sha256d};
use rocket::request::{self, FromRequest, Request};
use rocket::{http::Status, Outcome, State};
use rocket_contrib::json::{Json, JsonValue};

use asset_registry::asset::{Asset, AssetRegistry};
use asset_registry::errors::Result;

struct ReqSig(String);
impl<'a, 'r> FromRequest<'a, 'r> for ReqSig {
    type Error = ();
    fn from_request(request: &'a Request<'r>) -> request::Outcome<ReqSig, ()> {
        match request.headers().get_one("X-Signature") {
            Some(signature) => Outcome::Success(ReqSig(signature.to_string())),
            None => Outcome::Failure((Status::Unauthorized, ())),
        }
    }
}

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
fn write(asset: Json<Asset>, signature: ReqSig, registry: State<AssetRegistry>) -> Result<()> {
    // TODO verify asset_id, issuance_txid, associated contract_hash and wrapped issuer_pubkey
    // TODO check signature
    // TODO verify online entity link
    debug!("write asset: {:?},\nsignature: {}", asset, signature.0);
    registry.write(asset.into_inner())
}

fn main() {
    let registry = AssetRegistry::load(&Path::new("./db")).expect("failed initializing assets db");

    info!("Registry: {:?}", registry);

    rocket::ignite()
        .manage(registry)
        .mount("/", routes![list, get, write])
        .launch();
}
