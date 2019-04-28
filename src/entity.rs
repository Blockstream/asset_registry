use bitcoin_hashes::hex::ToHex;
use failure::ResultExt;
use reqwest;

use crate::asset::Asset;
use crate::errors::Result;
use crate::util::is_valid_domain;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AssetEntity {
    #[serde(rename = "domain")]
    DomainName(String),
}

/*
pub enum EntityLinkProof {
    Url(String),
}
*/

impl AssetEntity {
    pub fn verify_link(asset: &Asset) -> Result<()> {
        match asset.entity() {
            AssetEntity::DomainName(domain) => verify_domain_link(asset, domain),
        }
    }
}

fn verify_domain_link(asset: &Asset, domain: &str) -> Result<()> {
    ensure!(is_valid_domain(domain), "invalid domain name");

    // TODO normalize domain name
    // TODO tor proxy for accessing onion

    // require tls for non-onion hosts, assume http for onion ones
    let protocol = if &domain[domain.len() - 6..] == ".onion" {
        "http"
    } else {
        "https"
    };

    let asset_id = asset.id().to_hex();
    let page_url = format!(
        "{}://{}/.well-known/liquid-asset-proof-{}",
        protocol, domain, asset_id
    );
    let expected_body = format!(
        "Authorize linking the domain name {} to the Liquid asset {}",
        domain, asset_id
    );

    debug!(
        "verifying domain name {} for {}: GET {}",
        domain, asset_id, page_url
    );

    let body = reqwest::get(&page_url)
        .context(format!("failed fetching {}", page_url))?
        .error_for_status()?
        .text()
        .context("invalid page contents")?;

    ensure!(body.trim_end() == expected_body, "page contents mismatch");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn init() {
        stderrlog::new().verbosity(3).init(); // .unwrap();
    }

    #[test]
    fn test_verify_domain_link() {
        init();

        let asset = Asset::load(PathBuf::from("test/db/asset.json")).unwrap();
        // expects https://test.dev/ to forward requests to a local web server
        verify_domain_link(&asset, "test.dv").expect("failed verifying domain name");
    }
}
