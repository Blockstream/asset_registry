use std::{fs, path};

use bitcoin_hashes::{hex::FromHex, hex::ToHex, sha256, Hash};
use elements::{AssetId, OutPoint};
use failure::ResultExt;
use regex::Regex;
use secp256k1::Secp256k1;
use serde_json::Value;
#[cfg(feature = "cli")]
use structopt::StructOpt;

use crate::chain::{verify_asset_issuance_tx, ChainQuery};
use crate::entity::{verify_asset_link, AssetEntity};
use crate::errors::{OptionExt, Result};
use crate::util::{verify_bitcoin_msg, verify_domain_name, verify_pubkey, TxInput};

lazy_static! {
    static ref EC: Secp256k1<secp256k1::VerifyOnly> = Secp256k1::verification_only();
    static ref RE_NAME: Regex = Regex::new(r"^[[:ascii:]]{5,255}$").unwrap();
    static ref RE_TICKER: Regex = Regex::new(r"^[a-zA-Z]{3,5}$").unwrap();
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset {
    pub asset_id: AssetId,
    pub contract: Value,

    pub issuance_txin: TxInput,
    pub issuance_prevout: OutPoint,

    #[serde(flatten)]
    pub fields: AssetFields,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

// Fields selected freely by the issuer
// Also used directly by structopt to parse CLI args
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "cli", derive(StructOpt))]
pub struct AssetFields {
    #[cfg_attr(
        feature = "cli",
        structopt(long, help = "Asset name (5-255 ascii characters)")
    )]
    pub name: String,

    #[cfg_attr(
        feature = "cli",
        structopt(long, help = "Asset ticker (3-5 alphanumeric characters)")
    )]
    pub ticker: Option<String>,

    #[cfg_attr(
        feature = "cli",
        structopt(long, default_value = "0", help = "Asset decimal precision (up to 8)")
    )]
    #[serde(default = "default_precision")]
    pub precision: u8,

    // Domain name is currently the only entity type,
    // translate the --domain CLI arg into AssetEntity::DomainName
    #[cfg_attr(
        feature = "cli",
        structopt(
            name = "domain",
            long,
            help = "Domain name to associate with the asset",
            parse(from_str = parse_domain_entity)
        )
    )]
    pub entity: AssetEntity,
}

impl AssetFields {
    fn from_contract(contract: &Value) -> Result<Self> {
        Ok(serde_json::from_value(contract.clone())?)
    }
}

#[cfg(feature = "cli")]
fn parse_domain_entity(domain: &str) -> AssetEntity {
    AssetEntity::DomainName(domain.to_string())
}

#[cfg(feature = "cli")]
fn default_precision() -> u8 {
    0
}

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

    pub fn verify(&self, chain: Option<&ChainQuery>) -> Result<()> {
        // XXX version as top level field?
        ensure!(
            self.contract["version"].as_u64() == Some(0),
            "unknown version"
        );

        ensure!(self.fields.precision <= 8, "precision out of range");
        ensure!(RE_NAME.is_match(&self.fields.name), "invalid name");

        if let Some(ticker) = &self.fields.ticker {
            ensure!(RE_TICKER.is_match(ticker), "invalid ticker");
        }

        verify_pubkey(&self.issuer_pubkey()?).context("invalid issuer public key")?;

        let AssetEntity::DomainName(domain) = &self.fields.entity;
        verify_domain_name(domain).context("invalid domain name")?;

        verify_asset_commitment(self).context("failed verifying issuance commitment")?;

        verify_asset_fields(self).context("failed verifying asset fields")?;

        if let Some(chain) = chain {
            verify_asset_issuance_tx(chain, self).context("failed verifying on-chain issuance")?;
            // XXX keep block id?
        }

        verify_asset_link(self).context("failed verifying linked entity")?;

        Ok(())
    }

    pub fn verify_deletion(&self, signature: &[u8]) -> Result<()> {
        // TODO verify domain ownership proof was removed
        verify_bitcoin_msg(
            &EC,
            &self.issuer_pubkey()?,
            &signature,
            &format_deletion_sig_msg(self),
        )
    }

    pub fn contract_hash(&self) -> Result<sha256::Hash> {
        // json keys are sorted lexicographically
        let contract_str = serde_json::to_string(&self.contract)?;
        Ok(sha256::Hash::hash(&contract_str.as_bytes()))
    }

    pub fn issuer_pubkey(&self) -> Result<Vec<u8>> {
        let pubkey_hex = self.contract["issuer_pubkey"]
            .as_str()
            .or_err("missing issuer_pubkey")?;

        Ok(hex::decode(pubkey_hex).context("invalid issuer_pubkey hex")?)
    }

    pub fn from_request(req: AssetRequest, chain: &ChainQuery) -> Result<Self> {
        let mut asset_data = chain
            .get_asset(&req.asset_id)?
            .or_err("asset id not found")?;

        let fields =
            AssetFields::from_contract(&req.contract).context("invalid contract fields")?;

        let issuance_txin = serde_json::from_value(asset_data["issuance_txin"].take())?;
        let issuance_prevout = serde_json::from_value(asset_data["issuance_prevout"].take())?;

        Ok(Asset {
            asset_id: req.asset_id,
            contract: req.contract,
            fields,
            issuance_txin,
            issuance_prevout,
            signature: None,
        })
    }
}

#[cfg_attr(feature = "cli", derive(StructOpt))]
#[derive(Debug, Serialize, Deserialize)]
pub struct AssetRequest {
    #[cfg_attr(
        feature = "cli",
        structopt(
            long = "asset-id",
            help = "The asset-id",
            parse(try_from_str = AssetId::from_hex)
        )
    )]
    pub asset_id: AssetId,

    #[cfg_attr(
        feature = "cli",
        structopt(
            long = "contract",
            help = "The issuance contract",
            parse(try_from_str = serde_json::from_str)
        )
    )]
    pub contract: Value,
}

// Verify the asset id commits to the provided contract and prevout
fn verify_asset_commitment(asset: &Asset) -> Result<()> {
    let contract_hash = asset.contract_hash()?;
    let entropy = AssetId::generate_asset_entropy(asset.issuance_prevout, contract_hash);
    let asset_id = AssetId::from_entropy(entropy);

    ensure!(asset.asset_id == asset_id, "invalid asset commitment");

    debug!(
        "verified asset commitment, asset id {} commits to prevout {:?} and contract hash {} ({:?})",
        asset_id.to_hex(),
        asset.issuance_prevout,
        contract_hash.to_hex(),
        asset.contract,
    );
    Ok(())
}

// Verify the asset fields
fn verify_asset_fields(asset: &Asset) -> Result<()> {
    match &asset.signature {
        Some(signature) => {
            // If a signature is provided, verify that it signs over the fields
            verify_asset_fields_sig(
                &asset.issuer_pubkey()?,
                signature,
                &asset.asset_id,
                &asset.fields,
            )
        }
        None => {
            // Otherwise, verify that the fields match the commited contract
            ensure!(
                asset.fields == AssetFields::from_contract(&asset.contract)?,
                "fields mismatch commitment"
            );
            Ok(())
        }
    }
}

fn verify_asset_fields_sig(
    pubkey: &[u8],
    signature: &str,
    asset_id: &AssetId,
    fields: &AssetFields,
) -> Result<()> {
    let signature = base64::decode(signature).context("invalid signature base64")?;
    let msg = format_fields_sig_msg(asset_id, fields);

    verify_bitcoin_msg(&EC, &pubkey, &signature, &msg)?;

    debug!(
        "verified asset signature, issuer pubkey {} signed fields {:?}",
        hex::encode(pubkey),
        fields,
    );
    Ok(())
}

fn format_fields_sig_msg(asset_id: &AssetId, fields: &AssetFields) -> String {
    serde_json::to_string(&(
        "liquid-asset-fields",
        0, // version number for msg format
        asset_id.to_hex(),
        fields,
    ))
    .unwrap()
}

fn format_deletion_sig_msg(asset: &Asset) -> String {
    serde_json::to_string(&(
        "liquid-asset-deletion",
        0, // version number for deletion request
        asset.asset_id.to_hex(),
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
        stderrlog::new().verbosity(3).init().ok();
    }

    #[test]
    fn test1_asset_load() -> Result<()> {
        let asset = Asset::load(PathBuf::from("test/asset-signed.json")).unwrap();
        assert_eq!(
            asset.asset_id.to_hex(),
            "9a51761132b7399d34819c2c5d03af71794ff3aa0f78a434ddf20605545c86f2"
        );
        assert_eq!(asset.fields.ticker, Some("FOO".to_string()));
        Ok(())
    }

    #[test]
    fn test2_verify_asset_sig() -> Result<()> {
        let asset = Asset::load(PathBuf::from("test/asset-signed.json")).unwrap();
        verify_asset_fields_sig(
            &asset.issuer_pubkey().unwrap(),
            asset.signature.as_ref().unwrap(),
            &asset.asset_id,
            &asset.fields,
        )?;
        Ok(())
    }
}
