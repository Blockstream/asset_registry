use std::collections::HashMap;
use std::fs;
use std::path;
use std::sync::RwLock;

use bitcoin_hashes::{hex::ToHex, sha256d};
use failure::ResultExt;
use regex::Regex;
use secp256k1::Secp256k1;
use serde_json::Value;

use crate::entity::AssetEntity;
use crate::errors::{OptionExt, Result};
use crate::util::verify_bitcoin_msg;

lazy_static! {
    static ref EC: Secp256k1<secp256k1::VerifyOnly> = Secp256k1::verification_only();
    // XXX what characters should be allowed in the name?
    static ref RE_NAME: Regex = Regex::new(r"^[\w ]{5,16}$").unwrap();
    static ref RE_TICKER: Regex = Regex::new(r"^[A-Z]{3,5}$").unwrap();
}

base64_serde_type!(Base64, base64::STANDARD);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset {
    pub asset_id: sha256d::Hash,
    pub issuance_txid: sha256d::Hash,
    pub contract: String,

    //#[serde(with = "Base64")]
    //issuer_pubkey: [u8; 33],
    #[serde(flatten)]
    pub fields: AssetFields,

    #[serde(with = "Base64")]
    pub signature: Vec<u8>,
}

/// Fields selected freely by the issuer
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssetFields {
    pub name: String,
    pub ticker: Option<String>,
    // TODO verify range
    pub precision: Option<u8>,
    pub entity: AssetEntity,
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

    pub fn name(&self) -> &str {
        &self.fields.name
    }

    pub fn entity(&self) -> &AssetEntity {
        &self.fields.entity
    }

    pub fn verify(&self) -> Result<()> {
        // TODO verify asset_id, issuance_txid, associated contract_hash and wrapped issuer_pubkey
        // TODO verify H(contract) is committed to in the asset entropy
        // TODO verify top-level issuer_pubkey matches contract
        // XXX how should updates be verified? should we require a sequence number or other form of anti-replay?

        ensure!(RE_NAME.is_match(&self.fields.name), "invalid name");
        if let Some(ticker) = &self.fields.ticker {
            ensure!(RE_TICKER.is_match(&ticker), "invalid ticker");
        }
        if let Some(precision) = self.fields.precision {
            ensure!((0 < precision && precision <= 8), "precision out of range");
        }

        verify_asset_sig(self).context("failed verifying signature")?;

        AssetEntity::verify_link(self).context("failed verifying linked entity")?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct AssetRegistry {
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
        asset.verify()?;

        let mut assets = self.assets_map.write().unwrap();

        let path = self
            .directory
            .join(format!("{}.json", asset.asset_id.to_hex()));
        fs::write(path, serde_json::to_string(&asset)?)?;

        assets.insert(asset.asset_id, asset);
        Ok(())
    }
}

fn verify_asset_sig(asset: &Asset) -> Result<()> {
    let contract: Value = serde_json::from_str(&asset.contract).context("invalid contract json")?;

    let pubkey = contract["issuer_pubkey"]
        .as_str()
        .or_err("missing required contract.issuer_pubkey")?;
    let pubkey = hex::decode(pubkey)?;

    let msg = format_sig_msg(&asset.asset_id, &asset.fields);

    verify_bitcoin_msg(&EC, &pubkey, &asset.signature, &msg)?;

    Ok(())
}

pub fn format_sig_msg(asset_id: &sha256d::Hash, fields: &AssetFields) -> String {
    serde_json::to_string(&(
        "elements-asset-assoc",
        0, // version number for msg format
        asset_id,
        fields,
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
        verify_asset_sig(&asset)?;
        Ok(())
    }
}
