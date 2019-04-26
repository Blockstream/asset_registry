use bitcoin::consensus::encode::{serialize, VarInt};
use bitcoin_hashes::{sha256d, Hash};
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
}
