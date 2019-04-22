use std::path;
use std::fs;
use std::collections::HashMap;

use bitcoin_hashes::{sha256d, hex::FromHex};

use crate::errors::{OptionExt, Result, Error};

#[derive(Debug, Serialize, Deserialize)]
pub enum AssetEntity {
    DomainName,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Asset {
    asset_id: sha256d::Hash,
    issuance_txid: sha256d::Hash,

    contract: String,
    name: String,
    ticker: Option<String>,
    precision: Option<u8>,

    entity_type: AssetEntity,
    entity_identifier: String,
    entity_url: Option<String>,
    entity_proof: Option<String>,
}

impl Asset {
    pub fn new() -> Self {
        Asset {
            asset_id: sha256d::Hash::default(),
            issuance_txid: sha256d::Hash::default(),

            contract: "{\"issuer_pubkey\":\"aabb\"}".to_string(),
            name: "Foo Coin".to_string(),
            ticker: Some("FOO".to_string()),
            precision: Some(8),

            entity_type: AssetEntity::DomainName,
            entity_identifier: "foo.com".to_string(),
            entity_url: Some("https://foo.com/".to_string()),
            entity_proof: Some("https://foo.com/.well-known/liquid-issuer.proof".to_string())
        }
    }
    pub fn load(path: path::PathBuf) -> Result<Asset> {
        let contents = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&contents)?)
    }
}

#[derive(Debug)]
pub struct AssetRegistry {
    directory: path::PathBuf,
    assets_map: HashMap<sha256d::Hash, Asset>,
}

impl AssetRegistry {
    pub fn load(directory: &path::Path) -> Result<AssetRegistry> {
        let files = fs::read_dir(&directory)?;
        let assets_map = files.map(|entry| {
            let entry = entry?;
            let asset_id = sha256d::Hash::from_hex(entry.file_name().to_str().req()?)?;
            let asset = Asset::load(entry.path())?;
            Ok((asset_id, asset))
        }).collect::<Result<HashMap<sha256d::Hash, Asset>>>()?;

        Ok(AssetRegistry {
            directory: directory.to_path_buf(),
            assets_map,
        })
    }

    pub fn assets(&self) -> &HashMap<sha256d::Hash, Asset> {
        &self.assets_map
    }

    pub fn get(&self, asset_id: &sha256d::Hash) -> Option<&Asset> {
        self.assets_map.get(asset_id)
    }
}
