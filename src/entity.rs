use std::fmt;

use bitcoin_hashes::hex::ToHex;
use failure::ResultExt;
use reqwest;

use crate::asset::Asset;
use crate::errors::Result;
use crate::util::verify_domain_name;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AssetEntity {
    #[serde(rename = "domain")]
    DomainName(String),
}

impl fmt::Display for AssetEntity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AssetEntity::DomainName(domain) => write!(f, "domain:{}", domain),
        }
    }
}

pub fn verify_asset_link(asset: &Asset) -> Result<()> {
    match asset.entity() {
        AssetEntity::DomainName(domain) => verify_domain_link(asset, domain),
    }
}

fn verify_domain_link(asset: &Asset, domain: &str) -> Result<()> {
    verify_domain_name(domain).context("invalid domain name")?;

    // TODO tor proxy for accessing onion

    let asset_id = asset.id().to_hex();

    let expected_body = format!(
        "Authorize linking the domain name {} to the Liquid asset {}",
        domain, asset_id
    );

    let page_url = if cfg!(any(test, feature = "dev")) {
        // use a hard-coded verification page in testing and development modes
        format!(
            "http://127.0.0.1:58712/.well-known/liquid-asset-proof-{}",
            asset_id
        )
    } else {
        // require tls for non-onion hosts, assume http for onion ones
        let protocol = if domain.ends_with(".onion") {
            "http"
        } else {
            "https"
        };

        format!(
            "{}://{}/.well-known/liquid-asset-proof-{}",
            protocol, domain, asset_id
        )
    };

    debug!(
        "verifying domain name {} for {}: GET {}",
        domain, asset_id, page_url
    );

    let body = reqwest::get(&page_url)
        .context(format!("failed fetching {}", page_url))?
        .error_for_status()?
        .text()
        .context("invalid page contents")?;

    ensure!(
        body.trim_end() == expected_body,
        "verification page contents mismatch"
    );

    debug!("verified domain link {} for {}", domain, asset_id);

    Ok(())
}

// needs to be run with --test-threads 1
#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::util::BoolOpt;
    use rocket as r;
    use std::path::PathBuf;
    use std::sync::{Once, ONCE_INIT};

    static SPAWN_ONCE: Once = ONCE_INIT;

    // a server that identifies as "test.dev" and verifies any requested asset id
    pub fn spawn_mock_verifier_server() {
        SPAWN_ONCE.call_once(|| {
            let config = r::config::Config::build(r::config::Environment::Development)
                .port(58712)
                .finalize()
                .unwrap();
            let rocket = r::custom(config).mount("/", routes![verify_handler]);

            std::thread::spawn(|| rocket.launch());
        })
    }

    #[get("/.well-known/<page>")]
    fn verify_handler(page: String) -> Option<String> {
        page.starts_with("liquid-asset-proof-")
            .as_option()
            .map(|_| {
                format!(
                    "Authorize linking the domain name test.dev to the Liquid asset {}",
                    &page[19..]
                )
            })
    }

    #[test]
    fn test0_init() {
        stderrlog::new().verbosity(3).init().ok();
        spawn_mock_verifier_server();
    }

    #[test]
    fn test1_verify_domain_link() {
        let asset = Asset::load(PathBuf::from("test/asset-signed.json")).unwrap();
        // expects https://test.dev/ to forward requests to a local web server
        verify_domain_link(&asset, "test.dev").expect("failed verifying domain name");
    }
}
