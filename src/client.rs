use base64::prelude::{Engine, BASE64_STANDARD as BASE64};
use elements::{issuance::ContractHash, AssetId};
use reqwest::{blocking::Client as ReqClient, StatusCode, Url};
use serde_json::Value;

use crate::asset::{Asset, AssetRequest};
use crate::errors::{Result, ResultExt};

pub struct Client {
    registry_url: Url,
    rclient: ReqClient,
}

impl Client {
    pub fn new(registry_url: Url) -> Self {
        Client {
            registry_url,
            rclient: ReqClient::new(),
        }
    }

    pub fn get(&self, asset_id: &AssetId) -> Result<Option<Asset>> {
        let resp = self
            .rclient
            .get(self.registry_url.join(&asset_id.to_string())?)
            .send()
            .context("failed fetching asset from registry")?;

        if resp.status() == StatusCode::NOT_FOUND {
            Ok(None)
        } else {
            Ok(Some(
                resp.error_for_status()
                    .context("failed fetching asset from registry")?
                    .json()
                    .context("failed parsing asset from registry")?,
            ))
        }
    }

    /*
    pub fn index(&self) -> Result<HashMap<AssetId, Asset>> {
        Ok(self
            .rclient
            .get(self.registry_url.join("/"))
            .send()
            .context("failed fetching assets from registry")?
            .error_for_status()
            .context("failed fetching assets from registry")?
            .json()
            .context("failed deserializing asset map from registry")?)
    }
    */

    pub fn register(&self, asset: &AssetRequest) -> Result<Asset> {
        Ok(self
            .rclient
            .post(self.registry_url.join("/")?)
            .json(asset)
            .send()
            .context("failed sending asset to registry")?
            .error_for_status()
            .context("failed sending asset to registry")?
            .json()
            .context("failed parsing asset from registry")?)
    }

    pub fn delete(&self, asset_id: &AssetId, signature: &[u8]) -> Result<()> {
        self.rclient
            .delete(self.registry_url.join(&asset_id.to_string())?)
            .json(&json!({ "signature": BASE64.encode(signature) }))
            .send()
            .context("failed sending deletion request to registry")?
            .error_for_status()
            .context("asset deletion failed")?;
        Ok(())
    }

    pub fn validate_contract(&self, contract: &Value, contract_hash: &ContractHash) -> Result<()> {
        let resp = self
            .rclient
            .post(self.registry_url.join("/contract/validate")?)
            .json(&json!({ "contract": contract, "contract_hash": contract_hash }))
            .send()
            .context("failed sending validation request to registry")?;

        if resp.status() != StatusCode::OK {
            bail!("validation failed: {}", resp.text()?);
        }
        Ok(())
    }
}
