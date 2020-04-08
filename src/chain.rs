use reqwest::{blocking::Client as ReqClient, StatusCode};
use serde_json::Value;

use bitcoin::{BlockHash, Txid};
use bitcoin_hashes::{hex::ToHex, Hash};
use elements::{encode::deserialize, issuance::ContractHash, AssetId, Transaction};

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

            Some(deserialize(&hex::decode(hex.trim())?)?)
        })
    }

    pub fn get_tx_status(&self, txid: &Txid) -> Result<Option<BlockId>> {
        let status: Value = self
            .rclient
            .get(&format!("{}/tx/{}/status", self.api_url, txid.to_hex()))
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
            .get(&format!("{}/asset/{}", self.api_url, asset_id.to_hex()))
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
        txin.asset_issuance.asset_entropy == asset.contract_hash()?.into_inner(),
        "issuance entropy does not match contract hash"
    );

    // this is already verified as part of verify_asset_commitment, but we double-check here as a
    // sanity check
    let entropy = AssetId::generate_asset_entropy(
        txin.previous_output,
        ContractHash::from_inner(txin.asset_issuance.asset_entropy),
    );
    ensure!(
        AssetId::from_entropy(entropy) == asset.asset_id,
        "asset id mismatch"
    );

    debug!(
        "verified on-chain issuance of asset {}, tx input {:?}",
        asset.asset_id.to_hex(),
        asset.issuance_txin,
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
    use std::sync::Once;

    static SPAWN_ONCE: Once = Once::new();

    // a server that identifies as "test.dev" and verifies any requested asset id
    pub fn spawn_mock_esplora_server() {
        SPAWN_ONCE.call_once(|| {
            let config = r::config::Config::build(r::config::Environment::Development)
                .port(58713)
                .finalize()
                .unwrap();
            let rocket = r::custom(config).mount(
                "/",
                routes![tx_hex_handler, tx_status_handler, asset_handler],
            );

            std::thread::spawn(|| rocket.launch());
        })
    }

    #[get("/tx/<_txid>/hex")]
    fn tx_hex_handler(_txid: String) -> Result<String> {
        Ok(fs::read_to_string("test/committed-issuance-tx.hex")?)
    }

    #[get("/tx/<_txid>/status")]
    fn tx_status_handler(_txid: String) -> JsonValue {
        JsonValue::from(json!({
            "confirmed": true,
            "block_height": 999,
            "block_hash": "6ef1b8ac6cfacae9493e8d214d5ddd70322abe39bc0ab82727849b47bfb1fce6",
            "block_time": 1556733700
        }))
    }

    #[get("/asset/<_asset_id>")]
    fn asset_handler(_asset_id: String) -> JsonValue {
        JsonValue::from(json!({
            // some fields unnecessary for testing ommitted
             "issuance_txin": {
                 "txid": "9b75a545ff42c403839b0be69c1047144dc3e778c0d937d85c71538f169eebb5",
                 "vin": 0
             },
             "issuance_prevout": {
                 "txid": "c1854811ffe022a023e42769a703d434a40cb3dc16407e1a47aa6279d6cd48b4",
                 "vout": 2
             },
        }))
    }

    #[test]
    fn test0_init() {
        stderrlog::new().verbosity(3).init().ok();

        spawn_mock_esplora_server();
    }

    #[test]
    fn test1_verify() -> Result<()> {
        let asset = Asset::load(PathBuf::from("test/asset-committed.json"))?;
        let chain = ChainQuery::new("http://localhost:58713".to_string());

        verify_asset_issuance_tx(&chain, &asset)?;
        Ok(())
    }
}
