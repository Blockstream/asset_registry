use std::fmt;

use base64::prelude::{Engine, BASE64_STANDARD as BASE64};
use bitcoin::hashes::Hash;
use bitcoin::hex::{DisplayHex, FromHex};
use bitcoin::secp256k1::{self, ecdsa, Secp256k1};
use bitcoin::sign_message::signed_msg_hash;
use elements::{OutPoint, Txid};
use regex::RegexSet;
use serde::{Deserialize, Deserializer, Serializer};

use crate::errors::{OptionExt, Result, ResultExt};

#[derive(Serialize, Deserialize, Clone)]
pub struct TxInput {
    pub txid: Txid,
    pub vin: usize,
}

impl fmt::Debug for TxInput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TxInput {}:{}", self.txid, self.vin)
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
    let signature = ecdsa::Signature::from_compact(&signature)?;
    let msg_hash = signed_msg_hash(msg);
    let msg_secp = secp256k1::Message::from_digest(msg_hash.to_byte_array());

    Ok(ec
        .verify_ecdsa(&msg_secp, &signature, &pubkey)
        .context("signature veritification failed")?)
}

pub fn verify_pubkey(pubkey: &[u8]) -> Result<()> {
    secp256k1::PublicKey::from_slice(pubkey)?;
    Ok(())
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
        idna_to_ascii(domain)? == domain.to_string(),
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
    ensure!(labels.len() <= 127, "must not have more than 127 labels");

    // prevents using "localhost"
    ensure!(labels.len() > 1, "must have at least two labels");

    labels.reverse();
    for (i, label) in labels.iter().enumerate() {
        if i == 0 && label.parse::<f64>().is_ok() {
            bail!("the tld must not be a number");
        }
        ensure!(
            DOMAIN_LABEL.is_match(label),
            "must only contain allowed characters"
        );
    }
    Ok(())
}

fn idna_to_ascii(domain: &str) -> Result<String> {
    Ok(idna::domain_to_ascii(domain)
        .ok()
        .or_err("invalid domain")?)
}

/// Deserializes a base64 string to a `Vec<u8>`.
pub fn serde_from_base64<'de, D>(deserializer: D) -> std::result::Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    String::deserialize(deserializer).and_then(|string| {
        BASE64
            .decode(&string)
            .map_err(|err| Error::custom(err.to_string()))
    })
}

/// Deserializes a hex string to a `Vec<u8>`.
pub fn serde_from_hex<'de, D>(deserializer: D) -> std::result::Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    String::deserialize(deserializer)
        .and_then(|string| Vec::from_hex(&string).map_err(|err| Error::custom(err.to_string())))
}

/// Serializes a Vec<u8> into a hex string.
pub fn serde_to_hex<T, S>(buffer: &T, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    T: AsRef<[u8]>,
    S: Serializer,
{
    serializer.serialize_str(&buffer.as_ref().to_lower_hex_string())
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
            Vec::from_hex("026be637f97bc191c27522577bd6fe284b54404321652fcc4eb62aa0f4cfd6d172")?;
        let signature = BASE64.decode("H7719XlaZJT6H4HrD9KXga7yfd0MR8lSKc34TN/u0nhpecU9bVfaUDcpJtOFodfxf+IyFIE5V2A9878mM5bWvbE=")?;

        verify_bitcoin_msg(&ec, &pubkey, &signature, &msg)?;

        Ok(())
    }
}

// A serde remote type to retain the JSON serialization format used by prior
// releases of rust-{bitcoin,elements} - a JSON object with `txid` and `vout`
// fields, rather than the "txid:vout" string format used by newer releases.
// This is needed for compatibility with the asset registry JSON files.
// See https://serde.rs/remote-derive.html
#[derive(Serialize, Deserialize)]
#[serde(remote = "OutPoint")]
pub struct OutPointSerializer {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPointSerializer {
    pub fn from_value(val: serde_json::Value) -> serde_json::Result<OutPoint> {
        #[derive(Serialize, Deserialize)]
        struct Wrapper(#[serde(with = "OutPointSerializer")] OutPoint);

        Ok(serde_json::from_value::<Wrapper>(val)?.0)
    }
}
