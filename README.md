# Liquid Asset Registry

## Documentation:

- [Asset registry specification](doc/specs.md)

- [Using the Rust CLI (`liquid-asset-registry`)](doc/cli.md)


## Testing

Uses rocket for mock http servers, which requires nightly.

```
$ cargo +nightly test --features 'cli server client'  -- --test-threads 1
```

## Development

You may enable the `dev` feature to have domain proofs checked against a local server running at
`http://127.0.0.1:58712/.well-known/liquid-asset-proof-<asset-id>`
instead of the real domain name.

Make sure to enable all the features for `cargo check`:

```
$ cargo check --features 'cli server client'
```
