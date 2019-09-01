use std::fmt;

use bitcoin::consensus::encode::{serialize, VarInt};
use bitcoin_hashes::{hex::ToHex, sha256d, Hash};
use failure::ResultExt;
use idna::uts46;
use regex::RegexSet;
use secp256k1::Secp256k1;

use crate::errors::Result;

static MSG_SIGN_PREFIX: &'static [u8] = b"\x18Bitcoin Signed Message:\n";

#[derive(Serialize, Deserialize, Clone)]
pub struct TxInput {
    pub txid: sha256d::Hash,
    pub vin: usize,
}

impl fmt::Debug for TxInput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TxInput {}:{}", self.txid.to_hex(), self.vin)
    }
}

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

// Utility to transform booleans into Options
pub trait BoolOpt: Sized {
    fn as_option(self) -> Option<()>;
}
impl BoolOpt for bool {
    #[inline]
    fn as_option(self) -> Option<()> {
        if self {
            Some(())
        } else {
            None
        }
    }
}

// Domain name validation code extracted from https://github.com/rushmorem/publicsuffix/blob/master/src/lib.rs,
// MIT, Copyright (c) 2016 Rushmore Mushambi
// (with some changes annotated with "shesek" comments)

lazy_static! {
    // Regex for matching domain name labels
    static ref DOMAIN_LABEL: RegexSet = {
        RegexSet::new(vec![
            r"^[[:alnum:]]+$",
            r"^[[:alnum:]]+[[:alnum:]-]*[[:alnum:]]+$",
        ]).unwrap()
    };
}

pub fn verify_domain_name(domain: &str) -> Result<()> {
    ensure!(!domain.starts_with('.'), "cannot start with a dot");
    ensure!(
        idna_to_ascii(domain) == Some(domain.to_string()),
        "should be provided in ASCII/Punycode form, not IDNA Unicode"
    );
    ensure!(
        domain.to_lowercase() == domain,
        "should be provided in lower-case"
    );
    ensure!(domain.len() <= 255, "must be up to 255 characters");

    let mut labels: Vec<&str> = domain.split('.').collect();
    // strip of the first dot from a domain to support fully qualified domain names
    if domain.ends_with(".") {
        labels.pop();
    }
    // a domain must not have more than 127 labels
    ensure!(labels.len() <= 127, "must not have more than 127 labels");

    // shesek: a domain must have at least two parts (prevents accessing localhost)
    ensure!(labels.len() > 1, "must have at least two labels");

    labels.reverse();
    for (i, label) in labels.iter().enumerate() {
        // the tld must not be a number
        if i == 0 && label.parse::<f64>().is_ok() {
            bail!("the tld must not be a number");
        }
        // any label must only contain allowed characters
        ensure!(
            DOMAIN_LABEL.is_match(label),
            "must only contain allowed characters"
        );
    }
    Ok(())
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
    fn test_verify_domain_name() {
        assert!(verify_domain_name("foo.com").is_ok());
        assert!(verify_domain_name("foO.com").is_err());
        assert!(verify_domain_name(">foo.com").is_err());
        assert!(verify_domain_name("δοκιμή.com").is_err());
        assert!(verify_domain_name("xn--jxalpdlp.com").is_ok());
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
