use std::collections::HashMap;

use bitcoin_hashes::hex::ToHex;
use elements::AssetId;
use reqwest::{Client as ReqClient, StatusCode};

use crate::asset::Asset;
use crate::errors::{Result, ResultExt};

pub struct Client {
    registry_url: String,
    rclient: ReqClient,
}

impl Client {
    // TODO use reqwest::Url
    pub fn new(registry_url: String) -> Self {
        Client {
            registry_url,
            rclient: ReqClient::new(),
        }
    }

    pub fn get(&self, asset_id: &AssetId) -> Result<Option<Asset>> {
        let resp = self
            .rclient
            .get(&format!("{}/{}", self.registry_url, asset_id.to_hex()))
            .send()
            .context("failed fetching asset from registry")?;

        if resp.status() == StatusCode::NOT_FOUND {
            Ok(None)
        } else {
            Ok(Some(
                resp.error_for_status()
                    .context("failed fetching asset from registry")?
                    .json()
                    .context("failed deserializing asset map from registry")?,
            ))
        }
    }

    pub fn index(&self) -> Result<HashMap<AssetId, Asset>> {
        Ok(self
            .rclient
            .get(&self.registry_url)
            .send()
            .context("failed fetching assets from registry")?
            .error_for_status()
            .context("failed fetching assets from registry")?
            .json()
            .context("failed deserializing asset map from registry")?)
    }

    pub fn register(&self, asset: &Asset) -> Result<()> {
        self.rclient
            .post(&self.registry_url)
            .json(asset)
            .send()
            .context("failed sending asset to registry")?
            .error_for_status()
            .context("failed sending asset to registry")?;
        Ok(())
    }
}
