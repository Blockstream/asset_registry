use std::collections::HashMap;
use std::fs;
use std::path;
use std::sync::RwLock;

use bitcoin_hashes::{hex::ToHex, sha256d, Hash};
use failure::ResultExt;
use secp256k1::Secp256k1;
use serde_json::Value;

use crate::errors::{OptionExt, Result};

base64_serde_type!(Base64, base64::STANDARD);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AssetEntity {
    #[serde(rename = "domain")]
    DomainName(String),
}

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
    entity_url: Option<String>,
    entity_proof: Option<String>,

    #[serde(with = "Base64")]
    signature: Vec<u8>,
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

            entity: AssetEntity::DomainName("foo.com".to_string()),
            //entity_identifier: "foo.com".to_string(),
            entity_url: Some("https://foo.com/".to_string()),
            entity_proof: Some("https://foo.com/.well-known/liquid-issuer.proof".to_string()),

            signature: vec![123, 90],
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
        // XXX how should updates be verified? should we require a sequence number or other form of anti-replay?
        self.verify_sig(asset)
    }

    fn verify_sig(&self, asset: &Asset) -> Result<()> {
        let contract: Value =
            serde_json::from_str(&asset.contract).context("invalid contract json")?;

        let pubkey = contract["issuer_pubkey"]
            .as_str()
            .or_err("missing required contract.issuer_pubkey")?;
        let pubkey = secp256k1::PublicKey::from_slice(&hex::decode(pubkey)?)?;

        let msg = hash_for_sig(asset)?;
        let msg = secp256k1::Message::from_slice(&msg.into_inner())?;

        Ok(())

        //let signature = secp256k1::Signature::from_compact(&asset.signature)?;

        //Ok(self.ec.verify(&msg, &signature, &pubkey).context("signature veritification failed")?)
    }
}

fn hash_for_sig(asset: &Asset) -> Result<sha256d::Hash> {
    let data = serde_json::to_string(&(
        "elements-asset-assoc",
        0, // version number for msg format
        &asset.asset_id,
        &asset.name,
        &asset.ticker,
        &asset.precision,
        &asset.entity,
    ))?;
    Ok(sha256d::Hash::hash(data.as_bytes()))
}
