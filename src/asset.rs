use std::collections::HashMap;
use std::fs;
use std::path;
use std::sync::RwLock;

use bitcoin_hashes::{hex::ToHex, sha256d};
use failure::ResultExt;
use secp256k1::Secp256k1;
use serde_json::Value;

use crate::entity::AssetEntity;
use crate::errors::{OptionExt, Result};
use crate::util::verify_bitcoin_msg;

base64_serde_type!(Base64, base64::STANDARD);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset {
    asset_id: sha256d::Hash,
    issuance_txid: sha256d::Hash,
    contract: String,

    //#[serde(with = "Base64")]
    //issuer_pubkey: [u8; 33],
    name: String,
    ticker: Option<String>,
    precision: Option<u8>,

    entity: AssetEntity,

    #[serde(with = "Base64")]
    signature: Vec<u8>,
}

/*
struct AssetSignature {
    version: u32,
    timestamp: u32,
    seq: u32,
    #[serde(with = "Base64")]
    signature: Vec<u8>,
}
*/

impl Asset {
    pub fn load(path: path::PathBuf) -> Result<Asset> {
        let contents = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&contents)?)
    }

    pub fn id(&self) -> &sha256d::Hash {
        &self.asset_id
    }

    pub fn entity(&self) -> &AssetEntity {
        &self.entity
    }
}

#[derive(Debug)]
pub struct AssetRegistry {
    ec: Secp256k1<secp256k1::VerifyOnly>,
    directory: path::PathBuf,
    assets_map: RwLock<HashMap<sha256d::Hash, Asset>>,
}

impl AssetRegistry {
    pub fn load(directory: &path::Path) -> Result<AssetRegistry> {
        let files = fs::read_dir(&directory)?;
        let assets_map = files
            .map(|entry| {
                let entry = entry?;
                let asset = Asset::load(entry.path())?;
                Ok((asset.asset_id, asset))
            })
            .collect::<Result<HashMap<sha256d::Hash, Asset>>>()?;

        Ok(AssetRegistry {
            ec: Secp256k1::verification_only(),
            directory: directory.to_path_buf(),
            assets_map: RwLock::new(assets_map),
        })
    }

    pub fn list(&self) -> HashMap<sha256d::Hash, Asset> {
        let assets = self.assets_map.read().unwrap();
        assets.clone() // TODO avoid clone
    }

    pub fn get(&self, asset_id: &sha256d::Hash) -> Option<Asset> {
        let assets = self.assets_map.read().unwrap();
        assets.get(asset_id).cloned() // TODO avoid clone
    }

    pub fn write(&self, asset: Asset) -> Result<()> {
        self.verify(&asset)?;

        let mut assets = self.assets_map.write().unwrap();

        let path = self
            .directory
            .join(format!("{}.json", asset.asset_id.to_hex()));
        fs::write(path, serde_json::to_string(&asset)?)?;

        assets.insert(asset.asset_id, asset);
        Ok(())
    }

    fn verify(&self, asset: &Asset) -> Result<()> {
        // TODO verify asset_id, issuance_txid, associated contract_hash and wrapped issuer_pubkey
        // TODO verify H(contract) is committed to in the asset entropy
        // TODO verify online entity link
        // TODO verify top-level issuer_pubkey matches contract
        // XXX how should updates be verified? should we require a sequence number or other form of anti-replay?
        verify_asset_sig(&self.ec, asset)?;
        AssetEntity::verify_link(asset)?;
        Ok(())
    }
}

fn verify_asset_sig(ec: &Secp256k1<secp256k1::VerifyOnly>, asset: &Asset) -> Result<()> {
    let contract: Value = serde_json::from_str(&asset.contract).context("invalid contract json")?;

    let pubkey = contract["issuer_pubkey"]
        .as_str()
        .or_err("missing required contract.issuer_pubkey")?;
    let pubkey = hex::decode(pubkey)?;

    let msg = format_sig_msg(asset);
    debug!("msg: {}", msg);

    verify_bitcoin_msg(ec, &pubkey, &asset.signature, &msg)?;

    Ok(())
}

fn format_sig_msg(asset: &Asset) -> String {
    serde_json::to_string(&(
        "elements-asset-assoc",
        0, // version number for msg format
        &asset.asset_id,
        &asset.name,
        &asset.ticker,
        &asset.precision,
        &asset.entity,
    ))
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn init() {
        stderrlog::new().verbosity(3).init(); // .unwrap();
    }

    #[test]
    fn test_verify_asset_sig() -> Result<()> {
        init();

        let asset = Asset::load(PathBuf::from("test/db/asset.json")).unwrap();

        let ec = Secp256k1::verification_only();

        verify_asset_sig(&ec, &asset)?;

        Ok(())
    }
}
