# Liquid Asset Registry

## Running the server

```
$ cargo run --features 'cli server' --bin server -- -vv --db-path /path/to/db --addr 127.0.0.1:3000 --esplora-url https://blockstream.info/liquid/api/
```

## Using the CLI
```basg
$ cargo run --bin liquid-asset-registry -- --help

asset_registry 0.1.0

USAGE:
    liquid-asset-registry [FLAGS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Increase verbosity (up to 3 times)

SUBCOMMANDS:
    contract-json     print contract json in canonical serialization (sorted)
    help              Prints this message or the help of the given subcommand(s)
    register-asset    Send asset to registry
    verify-asset      Verify asset associations
```

Or build the executable:
```
$ cargo build --release
$ ./target/release/liquid-asset-registry --help
```

### Registering an asset

Get your contract hash:
```
$ liquid-asset-registry contract-json --hash '{"version":0,"issuer_pubkey":"<your-hex-encoded-pubkey>","name":"Foo Coin","ticker":"FOO",precision:2,"entity":{"domain":"mydomain.com"}}'
025d983cc774da665f412ccc6ccf51cb017671c2cb0d3c32d10d50ffdf0a57de
```

(You may also run `contract-json` without `--hash` to only canonicalize the JSON with lexicographically sorted keys,
then hash it yourself -- as a single SHA-256, but with *its bytes reversed*.)

Issue the asset on liquid using `rawissueasset` with your hash as the `contract_hash` parameter,
wait for the issuance transaction to confirm, then submit the asset to the registry:

```
$ liquid-asset-registry register-asset --asset-id <asset-id> --contract <contract-json>
```

### Verifying an asset

Verifies that the contract json is committed in the issuance transaction,
that that issuance transaction was confirmed,
and the domain ownership proof.

```
$ liquid-asset-registry verify-asset '<asset-json>' '<another-asset-json>' ..
```

For example:
```
$ curl https://assets.blockstream.info/<asset-id> > asset.json
$ liquid-asset-registry verify-asset "$(cat asset.json)"
```

## Testing

```
$ cargo test --features 'cli server client'  -- --test-threads 1
```

## Development

You may enable the `dev` feature to have domain proofs checked against
`http://127.0.0.1:58712/.well-known/liquid-asset-proof-<asset-id>`
instead of the real server.

Make sure to enable all the features for `cargo check`:

```
$ cargo check --features 'cli server client'
```
