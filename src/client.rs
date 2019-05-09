use bitcoin_hashes::hex::ToHex;
use elements::AssetId;
use reqwest::{Client as ReqClient, StatusCode, Url};

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
            .get(self.registry_url.join(&asset_id.to_hex())?)
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
}
