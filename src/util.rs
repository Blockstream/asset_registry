use bitcoin::consensus::encode::{serialize, VarInt};
use bitcoin::util::hash::bitcoin_merkle_root;
use bitcoin_hashes::{sha256d, Hash};
use elements::OutPoint;
use failure::ResultExt;
use idna::uts46;
use regex::RegexSet;
use secp256k1::Secp256k1;

use crate::errors::Result;

static MSG_SIGN_PREFIX: &'static [u8] = b"\x18Bitcoin Signed Message:\n";

pub fn verify_bitcoin_msg(
    ec: &Secp256k1<secp256k1::VerifyOnly>,
    pubkey: &[u8],
    signature: &[u8],
    msg: &str,
) -> Result<()> {
    let signature = if signature.len() == 65 {
        // Discard the flag byte, we assume compression and don't use recovery
        &signature[1..65]
    } else {
        signature
    };

    let pubkey = secp256k1::PublicKey::from_slice(pubkey)?;
    let signature = secp256k1::Signature::from_compact(&signature)?;
    let msg_hash = bitcoin_signed_msg_hash(msg);
    let msg_secp = secp256k1::Message::from_slice(&msg_hash.into_inner())?;

    Ok(ec
        .verify(&msg_secp, &signature, &pubkey)
        .context("signature veritification failed")?)
}

fn bitcoin_signed_msg_hash(msg: &str) -> sha256d::Hash {
    sha256d::Hash::hash(
        &[
            MSG_SIGN_PREFIX,
            &serialize(&VarInt(msg.len() as u64)),
            msg.as_bytes(),
        ]
        .concat(),
    )
}

// TODO PR into rust-elements
pub fn get_asset_tag(entropy: &sha256d::Hash) -> sha256d::Hash {
    sha256d::Hash::hash(&[entropy.into_inner(), [0u8; 32]].concat())
    //bitcoin_merkle_root(vec![entropy.clone(), sha256d::Hash::default()])
}

pub fn get_asset_entropy(prevout: &OutPoint, contract_hash: &sha256d::Hash) -> sha256d::Hash {
    // XXX should the outpoint be hashed with sha256 or double sha256?
    // let prevout_hash = sha256::Hash::hash(&serialize(prevout));
    // disguise the sha256::Hash as a sha256d::Hash, to make bitcoin_merkle_root accept it
    // let prevout_hash = sha256d::Hash::from_slice(&prevout_hash.into_inner()).unwrap();

    let prevout_hash = sha256d::Hash::hash(&serialize(prevout));
    bitcoin_merkle_root(vec![prevout_hash, contract_hash.clone()])
}

// Domain name validation code extracted from https://github.com/rushmorem/publicsuffix/blob/master/src/lib.rs
// (MIT, Copyright (c) 2016 Rushmore Mushambi)

lazy_static! {
    // Regex for matching domain name labels
    static ref DOMAIN_LABEL: RegexSet = {
        RegexSet::new(vec![
            r"^[[:alnum:]]+$",
            r"^[[:alnum:]]+[[:alnum:]-]*[[:alnum:]]+$",
        ]).unwrap()
    };
}

pub fn is_valid_domain(domain: &str) -> bool {
    // we are explicitly checking for this here before calling `domain_to_ascii`
    // because `domain_to_ascii` strips of leading dots so we won't be able to
    // check for this later
    if domain.starts_with('.') {
        return false;
    }
    // let's convert the domain to ascii early on so we can validate
    // internationalised domain names as well
    let domain = match idna_to_ascii(domain) {
        Some(domain) => domain,
        None => {
            return false;
        }
    };
    let mut labels: Vec<&str> = domain.split('.').collect();
    // strip of the first dot from a domain to support fully qualified domain names
    if domain.ends_with(".") {
        labels.pop();
    }
    // a domain must not have more than 127 labels
    if labels.len() > 127 {
        return false;
    }
    // shesek: a domain must have at least two parts (prevents accessing localhost)
    if labels.len() < 2 {
        return false;
    }
    labels.reverse();
    for (i, label) in labels.iter().enumerate() {
        // the tld must not be a number
        if i == 0 && label.parse::<f64>().is_ok() {
            return false;
        }
        // any label must only contain allowed characters
        if !DOMAIN_LABEL.is_match(label) {
            return false;
        }
    }
    true
}

fn idna_to_ascii(domain: &str) -> Option<String> {
    uts46::to_ascii(
        domain,
        uts46::Flags {
            use_std3_ascii_rules: false,
            transitional_processing: true,
            verify_dns_length: true,
        },
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin_hashes::hex::{FromHex, ToHex};

    #[test]
    fn test_is_valid_domain() {
        assert!(is_valid_domain("foo.com"));
        assert!(!is_valid_domain(">foo.com"));
        assert!(is_valid_domain("δοκιμή.com"));
    }

    #[test]
    fn test_bitcoin_msg_sign() -> Result<()> {
        let ec = Secp256k1::verification_only();

        let msg = "test";
        let pubkey =
            hex::decode("026be637f97bc191c27522577bd6fe284b54404321652fcc4eb62aa0f4cfd6d172")?;
        let signature = base64::decode("H7719XlaZJT6H4HrD9KXga7yfd0MR8lSKc34TN/u0nhpecU9bVfaUDcpJtOFodfxf+IyFIE5V2A9878mM5bWvbE=")?;

        verify_bitcoin_msg(&ec, &pubkey, &signature, &msg)?;

        Ok(())
    }

    #[test]
    fn test_asset_entropy() -> Result<()> {
        let prevout = OutPoint {
            txid: sha256d::Hash::from_hex(
                "0a93069bba360df60d77ecfff99304a9de123fecb8217348bb9d35f4a96d2fca",
            )
            .unwrap(),
            vout: 0,
        };
        let contract_hash = sha256d::Hash::default();
        let entropy = get_asset_entropy(&prevout, &contract_hash);
        let asset_id = get_asset_tag(&entropy);

        debug!(
            "prevout {:?} + contract_hash {} --> entropy {}",
            prevout,
            contract_hash.to_hex(),
            entropy.to_hex()
        );
        debug!("entropy = {}", entropy.to_hex());
        debug!("asset_id = {}", asset_id.to_hex());

        assert_eq!(
            entropy.to_hex(),
            "b8c4a6b3bb81c57e08b3c3b42d682ed287f492da6575fffd81d98893d74418b6"
        );
        assert_eq!(
            asset_id.to_hex(),
            "ff6fa9c92fd6086523e11607f6ee8ba90406ccaf738c49bf667ae5ec93733276"
        );

        Ok(())
    }

    #[test]
    fn test_asset_id() {
        let entropy = sha256d::Hash::from_hex(
            "b8c4a6b3bb81c57e08b3c3b42d682ed287f492da6575fffd81d98893d74418b6",
        )
        .unwrap();
        let asset_id = get_asset_tag(&entropy);
        debug!("entropy {} --> asset_id {}", entropy, asset_id);
        assert_eq!(
            asset_id.to_hex(),
            "ff6fa9c92fd6086523e11607f6ee8ba90406ccaf738c49bf667ae5ec93733276"
        );
    }
}
