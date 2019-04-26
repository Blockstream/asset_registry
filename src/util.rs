use idna::uts46;
use regex::RegexSet;

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
}
