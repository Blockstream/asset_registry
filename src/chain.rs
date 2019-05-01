use bitcoin::consensus::encode::deserialize;
use bitcoin_hashes::{hex::ToHex, sha256, sha256d, Hash};
use elements::{AssetId, Transaction};
use reqwest::{Client as ReqClient, StatusCode};
use serde_json::Value;

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
    pub block_hash: sha256d::Hash,
    pub block_time: u32,
}

impl ChainQuery {
    pub fn new(api_url: String) -> Self {
        ChainQuery {
            api_url,
            rclient: ReqClient::new(),
        }
    }

    pub fn get_tx(&self, txid: &sha256d::Hash) -> Result<Option<Transaction>> {
        let resp = self
            .rclient
            .get(&format!("{}/tx/{}/hex", self.api_url, txid.to_hex()))
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
            debug!("tx hex: {}--", hex);
            Some(deserialize(&hex::decode(hex.trim())?)?)
        })
    }

    pub fn get_tx_status(&self, txid: &sha256d::Hash) -> Result<Option<BlockId>> {
        let status: Value = self
            .rclient
            .get(&format!("{}/tx/{}/status", self.api_url, txid.to_hex()))
            .send()
            .context("failed fetching tx status")?
            .error_for_status()
            .context("failed fetching tx status")?
            .json()?;

        debug!("tx status: {:?}", status);

        Ok(if status["confirmed"].as_bool().unwrap_or(false) {
            Some(serde_json::from_value(status)?)
        } else {
            None
        })
    }
}

pub fn verify_asset_issuance_tx(chain: &ChainQuery, asset: &Asset) -> Result<BlockId> {
    let tx = chain
        .get_tx(&asset.issuance_tx.txid)?
        .or_err("issuance transaction not found")?;
    let txin = tx
        .input
        .get(asset.issuance_tx.vin)
        .or_err("issuance transaction missing input")?;
    let blockid = chain
        .get_tx_status(&asset.issuance_tx.txid)?
        .or_err("issuance transaction unconfirmed")?;

    ensure!(
        tx.txid() == asset.issuance_tx.txid,
        "issuance txid mismatch"
    );
    ensure!(txin.has_issuance(), "input has no issuance");
    ensure!(
        txin.previous_output == asset.issuance_prevout,
        "issuance prevout mismatch"
    );
    ensure!(
        txin.asset_issuance.asset_entropy == asset.contract_hash()?.into_inner(),
        "issuance entropy does not match contract hash"
    );

    let entropy = AssetId::generate_asset_entropy(
        txin.previous_output,
        sha256::Hash::from_inner(txin.asset_issuance.asset_entropy),
    );
    ensure!(
        AssetId::from_entropy(entropy) == asset.asset_id,
        "asset id mismatch"
    );

    debug!(
        "verified on-chain issuance of asset {}, tx {}:{}",
        asset.asset_id.to_hex(),
        asset.issuance_tx.txid.to_hex(),
        asset.issuance_tx.vin
    );

    Ok(blockid)
}

// needs to be run with --test-threads 1
#[cfg(test)]
pub mod tests {
    use super::*;
    use rocket as r;
    use rocket_contrib::json::JsonValue;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Once, ONCE_INIT};

    static SPAWN_ONCE: Once = ONCE_INIT;

    // a server that identifies as "test.dev" and verifies any requested asset id
    pub fn spawn_mock_esplora_api() {
        SPAWN_ONCE.call_once(|| {
            let config = r::config::Config::build(r::config::Environment::Development)
                .port(58713)
                .finalize()
                .unwrap();
            let rocket = r::custom(config).mount("/", routes![tx_hex_handler, tx_status_handler]);

            std::thread::spawn(|| rocket.launch());
        })
    }

    #[get("/tx/<_txid>/hex")]
    fn tx_hex_handler(_txid: String) -> Result<String> {
        Ok(fs::read_to_string("test/issuance-tx.hex")?)
    }

    #[get("/tx/<_txid>/status")]
    fn tx_status_handler(_txid: String) -> JsonValue {
        json!({
            "confirmed": true,
            "block_height": 999,
            "block_hash": "6ef1b8ac6cfacae9493e8d214d5ddd70322abe39bc0ab82727849b47bfb1fce6",
            "block_time": 1556733700
        })
    }

    #[test]
    fn test0_init() {
        #[allow(unused_must_use)]
        stderrlog::new().verbosity(4).init();

        spawn_mock_esplora_api();
    }

    #[test]
    fn test1_verify() -> Result<()> {
        let asset = Asset::load(PathBuf::from("test/db/asset.json"))?;
        let chain = ChainQuery::new("http://localhost:58713".to_string());

        verify_asset_issuance_tx(&chain, &asset)?;
        Ok(())
    }
}
