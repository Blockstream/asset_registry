use std::collections::HashMap;
use std::fs;
use std::path;
use std::sync::RwLock;
//use std::str::FromStr;

use bitcoin_hashes::{hex::FromHex, hex::ToHex, sha256d, Hash};
use secp256k1::PublicKey;
use serde_json::Value;
//use serde::{de, ser, Serializer, Deserializer};

use crate::errors::{OptionExt, Result};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AssetEntity {
    DomainName,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset {
    asset_id: sha256d::Hash,
    issuance_txid: sha256d::Hash,
    contract: String,

    //#[serde(serialize_with = "ser_pubkey", deserialize_with = "deser_pubkey")]
    //issuer_pubkey: PublicKey,
    name: String,
    ticker: Option<String>,
    precision: Option<u8>,

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
            entity_proof: Some("https://foo.com/.well-known/liquid-issuer.proof".to_string()),
        }
    }

    pub fn load(path: path::PathBuf) -> Result<Asset> {
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
        let mut assets = self.assets_map.write().unwrap();

        let path = self.directory.join(format!("{}.json", asset.asset_id.to_hex()));
        fs::write(path, serde_json::to_string(&asset)?)?;

        assets.insert(asset.asset_id, asset);
        Ok(())
    }

    pub fn verify(&self, asset: &Asset, signature: &str) -> Result<()> {
        // TODO verify asset_id, issuance_txid, associated contract_hash and wrapped issuer_pubkey
        // TODO verify H(contract) is committed to in the asset entropy
        // TODO verify online entity link
        verify_asset_data_sig(asset, signature)
    }
}

fn verify_asset_data_sig(asset: &Asset, signature: &str) -> Result<()> {
    let contract: Value = serde_json::from_str(&asset.contract)?;

    let pubkey = contract["issuer_pubkey"].as_str().or_err("missing required contract.issuer_pubkey")?;
    let pubkey = hex::decode(pubkey)?;
    let pubkey = PublicKey::from_slice(&pubkey)?;

    let msg = hash_for_sig(asset)?;
    let msg = secp256k1::Message::from_slice(&msg.into_inner())?;

    let signature = base64::decode(&signature)?;
    let signature = secp256k1::Signature::from_compact(&signature)?;

    Ok(secp256k1::Secp256k1::verification_only().verify(&msg, &signature, &pubkey)?)
}

fn hash_for_sig(asset: &Asset) -> Result<sha256d::Hash> {
    let data = serde_json::to_string(&(
        "elements-asset-assoc",
        &asset.asset_id,
        &asset.name,
        &asset.ticker,
        &asset.precision,
        &asset.entity_type,
        &asset.entity_identifier,
    ))?;
    Ok(sha256d::Hash::from_slice(data.as_bytes())?)
}
