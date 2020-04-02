# Liquid Asset Registry

## Running

```bash
$ cargo run --features 'cli server' --bin server -- -vv --db-path /path/to/db --esplora-url https://blockstream.info/liquid/api/
```

## Testing

Uses rocket for mock http servers, which requires nightly.

```bash
$ cargo +nightly test
```

## Development

You may enable the `dev` feature to have domain proofs checked against
`http://127.0.0.1:58712/.well-known/liquid-asset-proof-<asset-id>`
instead of the real server.
