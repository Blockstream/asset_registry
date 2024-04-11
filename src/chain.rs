use reqwest::{blocking::Client as ReqClient, StatusCode};
use serde_json::Value;

use bitcoin::hashes::{sha256, Hash};
use bitcoin::hex::FromHex;
use elements::{
    encode::deserialize, issuance::ContractHash, AssetId, BlockHash, Transaction, Txid,
};

use crate::asset::Asset;
use crate::errors::{OptionExt, Result, ResultExt};

#[derive(Debug)]
pub struct ChainQuery {
    api_url: String,
    rclient: ReqClient,
}

#[derive(Deserialize)]
pub struct BlockId {
    pub block_height: usize,
    pub block_hash: BlockHash,
    pub block_time: u32,
}

impl ChainQuery {
    pub fn new(api_url: String) -> Self {
        ChainQuery {
            api_url: api_url.trim_end_matches('/').into(),
            rclient: ReqClient::new(),
        }
    }

    pub fn get_tx(&self, txid: &Txid) -> Result<Option<Transaction>> {
        let resp = self
            .rclient
            .get(&format!("{}/tx/{}/hex", self.api_url, txid))
            .send()
            .context("failed fetching tx")?;

        Ok(if resp.status() == StatusCode::NOT_FOUND {
            None
        } else {
            let hex = resp
                .error_for_status()
                .context("failed fetching tx")?
                .text()
                .context("failed reading tx")?;
            let raw = Vec::from_hex(hex.trim())?;

            Some(deserialize(&raw)?)
        })
    }

    pub fn get_tx_status(&self, txid: &Txid) -> Result<Option<BlockId>> {
        let status: Value = self
            .rclient
            .get(&format!("{}/tx/{}/status", self.api_url, txid))
            .send()
            .context("failed fetching tx status")?
            .error_for_status()
            .context("failed fetching tx status")?
            .json()?;

        Ok(if status["confirmed"].as_bool().unwrap_or(false) {
            Some(serde_json::from_value(status)?)
        } else {
            None
        })
    }

    pub fn get_asset(&self, asset_id: &AssetId) -> Result<Option<Value>> {
        let resp = self
            .rclient
            .get(&format!("{}/asset/{}", self.api_url, asset_id))
            .send()
            .context("failed fetching tx")?;

        Ok(if resp.status() == StatusCode::NOT_FOUND {
            None
        } else {
            Some(
                resp.error_for_status()
                    .context("failed fetching asset")?
                    .json()
                    .context("failed reading asset")?,
            )
        })
    }
}

pub fn verify_asset_issuance_tx(chain: &ChainQuery, asset: &Asset) -> Result<BlockId> {
    let tx = chain
        .get_tx(&asset.issuance_txin.txid)?
        .or_err("issuance transaction not found")?;
    let txin = tx
        .input
        .get(asset.issuance_txin.vin)
        .or_err("issuance transaction missing input")?;
    let blockid = chain
        .get_tx_status(&asset.issuance_txin.txid)?
        .or_err("issuance transaction unconfirmed")?;

    ensure!(
        tx.txid() == asset.issuance_txin.txid,
        "issuance txid mismatch"
    );
    ensure!(txin.has_issuance(), "input has no issuance");
    ensure!(
        txin.previous_output == asset.issuance_prevout,
        "issuance prevout mismatch"
    );
    ensure!(
        txin.asset_issuance.asset_entropy == asset.contract_hash().to_byte_array(),
        "issuance entropy does not match contract hash"
    );

    // this is already verified as part of verify_asset_commitment, but we double-check here as a
    // sanity check
    let entropy = AssetId::generate_asset_entropy(
        txin.previous_output,
        ContractHash::from(sha256::Hash::from_byte_array(
            txin.asset_issuance.asset_entropy,
        )),
    );
    ensure!(
        AssetId::from_entropy(entropy) == asset.asset_id,
        "asset id mismatch"
    );

    debug!(
        "verified on-chain issuance of asset {}, tx input {:?}",
        asset.asset_id, asset.issuance_txin,
    );

    Ok(blockid)
}

// needs to be run with --test-threads 1
#[cfg(test)]
pub mod tests {
    use super::*;
    use rocket::serde::json::Json;
    use serde_json::Value;
    use std::path::PathBuf;
    use std::sync::Once;
    use std::{fs, str::FromStr};

    static SPAWN_ONCE: Once = Once::new();

    // a server that identifies as "test.dev" and verifies any requested asset id
    #[rocket::main]
    async fn launch_mock_esplora_server() {
        let config = rocket::Config::figment().merge(("port", 58713));
        let rocket = rocket::custom(config).mount(
            "/",
            rocket::routes![tx_hex_handler, tx_status_handler, asset_handler],
        );
        rocket.launch().await.unwrap();
    }
    pub fn spawn_mock_esplora_server() {
        SPAWN_ONCE.call_once(|| {
            std::thread::spawn(launch_mock_esplora_server);
        });
    }

    #[rocket::get("/tx/<txid>/hex")]
    fn tx_hex_handler(txid: &str) -> String {
        let path = format!("test/issuance-tx-{}.hex", &txid[..6]);
        fs::read_to_string(path).unwrap()
    }

    #[rocket::get("/asset/<asset_id>")]
    fn asset_handler(asset_id: &str) -> Json<Value> {
        let path = format!("test/asset-{}.json", &asset_id[..6]);
        let jsonstr = fs::read_to_string(path).unwrap();
        Json(serde_json::Value::from_str(&jsonstr).unwrap())
    }

    #[rocket::get("/tx/<_txid>/status")]
    fn tx_status_handler(_txid: &str) -> Json<Value> {
        Json(json!({
            "confirmed": true,
            "block_height": 999,
            "block_hash": "6ef1b8ac6cfacae9493e8d214d5ddd70322abe39bc0ab82727849b47bfb1fce6",
            "block_time": 1556733700
        }))
    }

    #[test]
    fn test0_init() {
        stderrlog::new().verbosity(3).init().ok();
        spawn_mock_esplora_server();
    }

    #[test]
    fn test1_verify() -> Result<()> {
        let asset = Asset::load(PathBuf::from("test/asset-b1405e.json"))?;
        let chain = ChainQuery::new("http://localhost:58713".to_string());

        verify_asset_issuance_tx(&chain, &asset)?;
        Ok(())
    }
}
