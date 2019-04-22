use std::path;
use std::fs;
use std::collections::HashMap;
use std::sync::RwLock;
//use std::str::FromStr;

use bitcoin_hashes::{sha256d, hex::FromHex, hex::ToHex};
//use secp256k1::PublicKey;
//use serde::{de, ser, Serializer, Deserializer};

use crate::errors::{OptionExt, Result as EResult};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AssetEntity {
    DomainName,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset {
    asset_id: sha256d::Hash,
    issuance_txid: sha256d::Hash,

    contract: String,
    name: String,
    ticker: Option<String>,
    precision: Option<u8>,

    //#[serde(serialize_with = "ser_pubkey", deserialize_with = "deser_pubkey")]
    //issuer_pubkey: PublicKey,

    entity_type: AssetEntity,
    entity_identifier: String,
    entity_url: Option<String>,
    entity_proof: Option<String>,
}

/*
fn ser_pubkey<S>(key: PublicKey, s: S) -> Result<S::Ok, S::Error>
where S: Serializer
{
    s.serialize_str(&hex::encode(&key.serialize()))
}

fn deser_pubkey<'de, D>(deserializer: D) -> Result<PublicKey, D::Error>
where D: Deserializer<'de>
{
    let keystr = String::deserialize(deserializer)?;
    Ok(PublicKey::from_str(keystr)?)
}*/

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

    pub fn load(path: path::PathBuf) -> EResult<Asset> {
        let contents = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&contents)?)
    }

    pub fn id(&self) -> &sha256d::Hash {
        &self.asset_id
    }
}

#[derive(Debug)]
pub struct AssetRegistry {
    directory: path::PathBuf,
    assets_map: RwLock<HashMap<sha256d::Hash, Asset>>,
}

impl AssetRegistry {
    pub fn load(directory: &path::Path) -> EResult<AssetRegistry> {
        let files = fs::read_dir(&directory)?;
        let assets_map = files.map(|entry| {
            let entry = entry?;
            let asset_id = sha256d::Hash::from_hex(entry.file_name().to_str().req()?)?;
            let asset = Asset::load(entry.path())?;
            Ok((asset_id, asset))
        }).collect::<EResult<HashMap<sha256d::Hash, Asset>>>()?;

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

    pub fn write(&self, asset: Asset) -> EResult<()> {
        let mut assets = self.assets_map.write().unwrap();

        let path = self.directory.join(asset.asset_id.to_hex());
        fs::write(path, serde_json::to_string(&asset)?)?;

        assets.insert(asset.asset_id, asset);
        Ok(())
    }
}
