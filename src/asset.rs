use std::{fs, path};

use bitcoin_hashes::{hex::ToHex, sha256, sha256d, Hash};
use elements::{AssetId, OutPoint};
use failure::ResultExt;
use regex::Regex;
use secp256k1::Secp256k1;
use serde_json::Value;
use structopt::StructOpt;

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
    pub asset_id: AssetId,
    pub contract: String,

    pub issuance_txid: sha256d::Hash,
    pub issuance_prevout: OutPoint,

    //#[serde(with = "Base64")]
    //issuer_pubkey: [u8; 33],
    #[serde(flatten)]
    pub fields: AssetFields,

    #[serde(with = "Base64")]
    pub signature: Vec<u8>,
}

// Fields selected freely by the issuer
// Also used directly by structopt to parse CLI args
#[derive(Debug, Serialize, Deserialize, Clone, StructOpt)]
pub struct AssetFields {
    #[structopt(long, help = "Asset name (5-16 characters)")]
    pub name: String,

    #[structopt(long, help = "Asset ticker (alphanumeric, 3-5 chars)")]
    pub ticker: Option<String>,

    #[structopt(long, help = "Asset decimal precision (up to 8)")]
    pub precision: Option<u8>,

    // Domain name is currently the only entity type,
    // translate the --domain CLI arg into AssetEntity::DomainName
    #[structopt(
        name = "domain",
        long,
        help = "Domain name to associate with the asset",
        parse(from_str = "parse_domain_entity")
    )]
    pub entity: AssetEntity,
}

fn parse_domain_entity(domain: &str) -> AssetEntity {
    AssetEntity::DomainName(domain.to_string())
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

    pub fn id(&self) -> &AssetId {
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

        verify_asset_commitment(self).context("failed verifying issuance commitment")?;
        verify_asset_sig(self).context("failed verifying signature")?;

        AssetEntity::verify_link(self).context("failed verifying linked entity")?;

        Ok(())
    }
}

fn verify_asset_commitment(asset: &Asset) -> Result<()> {
    let contract_hash = sha256::Hash::hash(&asset.contract.as_bytes());
    let entropy = AssetId::generate_asset_entropy(asset.issuance_prevout, contract_hash);
    let asset_id = AssetId::from_entropy(entropy);

    ensure!(asset.asset_id == asset_id, "invalid asset commitment");
    Ok(())
}

fn verify_asset_sig(asset: &Asset) -> Result<()> {
    let contract: Value = serde_json::from_str(&asset.contract).context("invalid contract json")?;

    let pubkey = contract["issuer_pubkey"]
        .as_str()
        .or_err("missing required contract.issuer_pubkey")?;
    let pubkey = hex::decode(pubkey).context("invalid contract.issuer_pubkey hex")?;

    let msg = format_sig_msg(&asset.asset_id, &asset.fields);

    verify_bitcoin_msg(&EC, &pubkey, &asset.signature, &msg)?;

    Ok(())
}

pub fn format_sig_msg(asset_id: &AssetId, fields: &AssetFields) -> String {
    serde_json::to_string(&(
        "elements-asset-assoc",
        0, // version number for msg format
        asset_id.to_hex(),
        fields,
    ))
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin_hashes::hex::ToHex;
    use std::path::PathBuf;

    #[test]
    fn test0_init() {
        stderrlog::new().verbosity(3).init();
    }

    #[test]
    fn test1_asset_load() -> Result<()> {
        let asset = Asset::load(PathBuf::from("test/db/asset.json")).unwrap();
        assert_eq!(
            asset.asset_id.to_hex(),
            "9a51761132b7399d34819c2c5d03af71794ff3aa0f78a434ddf20605545c86f2"
        );
        assert_eq!(asset.fields.ticker, Some("FOO".to_string()));
        Ok(())
    }

    #[test]
    fn test2_verify_asset_sig() -> Result<()> {
        let asset = Asset::load(PathBuf::from("test/db/asset.json")).unwrap();
        verify_asset_sig(&asset)?;
        Ok(())
    }
}
