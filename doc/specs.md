# Asset Metadata Registry

To associate an issued asset with its metadata on the Liquid asset registry, the asset contract JSON must be created in advance and have its hash committed to in the asset issuance transaction. The format for the contract and the issuance process are described below.

## Contract JSON fields

Required fields:

- `version`: currently `0`
- `issuer_pubkey`: the hex-encoded public key of the issuer
- `name`: 1-255 ASCII characters
- `entity`: the online entity linked to this asset. currently only supports (sub)domain names in the form of a nested object with `{"domain":"foobar.com"}`

Optional fields:

- `ticker`: 3-24 characters consisting of `a-z`, `A-Z`, `.` and `-`.
  If provided, has to be unique within the `entity` (domain name) namespace.
- `precision`: number of digits after the decimal point, i.e. 0 for non-divisible assets or 8 for BTC-like. defaults to 0.
- `collection`: 1-255 ASCII characters

Example:

```json
{
  "version": 0,
  "issuer_pubkey": "037c7db0528e8b7b58e698ac104764f6852d74b5a7335bffcdad0ce799dd7742ec",
  "name": "Foo Coin",
  "ticker": "FOO",
  "entity": { "domain": "foo-coin.com" },
  "precision": 8
}
```

## Contract hash

The contract hash is the (single) sha256 hash of the contract json document, *canonicalized to have its keys sorted lexicographically*.

The canonicalization can be done with Perl like so: `$ perl -e 'use JSON::PP; my $js = JSON::PP->new; $js->canonical(1); print $js->encode($js->decode($ARGV[0]))' '{"version":0,"name":"FOO",...}'`

Or with Python like so: `$ python -c 'import json,sys; sys.stdout.write(json.dumps(json.loads(sys.argv[1]), sort_keys=True, separators=(",",":")))' '{"version":0,"name":"FOO",...}'`

Or with JavaScript using the [json-stable-stringify](https://www.npmjs.com/package/json-stable-stringify) library.

The resulting sha256 hash needs to be reversed to match the format expected by elementsd, similarly to the reverse encoding of txids and blockhashes as originally implemented for bitcoin by satoshi.
This can be done in a unix environment like so: `echo "<contract hash> | fold -w2 | tac | tr -d "\n"`.

All together:

```
$ CONTRACT='{"version":0,"ticker":"FOO","name":"Foo Coin"}'
$ CONTRACT_HASH=$(python -c 'import json,sys; sys.stdout.write(json.dumps(json.loads(sys.argv[1]), sort_keys=True, separators=(",",":")))' "$CONTRACT" | sha256sum | head -c64)
$ CONTRACT_HASH_REV=$(echo $CONTRACT_HASH | fold -w2 | tac | tr -d "\n")
$ echo $CONTRACT_HASH_REV
```

This can also be done using the asset registry CLI utility: `$ liquid-asset-registry contract-json --hash '<contract-json>'`

## Domain ownership proof

To verify you control the `entity` domain name, you'll need to make a file on your webserver available at `https://<domain>/.well-known/liquid-asset-proof-<asset-id>`, with the following contents:

```
Authorize linking the domain name <domain> to the Liquid asset <asset-id>
```

Note that serving the file with `https` is required, except for `.onion` hidden services.

## Issuing & Registering assets

Prepare your contract json and get your contract hash. You can verify their validity before issuing the asset using the validation endpoint:

```bash
$ curl https://assets.blockstream.info/contract/validate -H 'Content-Type: application/json' \
       -d '{"contract": <your-contract-json>, "contract_hash": "<your-contract-hash>"}'
```

If everything seems good, issue the asset using elementsd's `issueasset` with your hash as the `contract_hash` parameter and take note of the resulting `asset_id`. Add the domain ownership proof (as described above) and, once the issuance transaction confirms, submit the asset to the registry:

```bash
$ elements-cli issueasset 10 0 true $CONTRACT_HASH_REV

$ curl https://assets.blockstream.info/ -H 'Content-Type: application/json' \
       -d '{"asset_id": "<asset-id>", "contract": <contract-json>}'
```

This can also be done using the asset registry CLI utility: `$ liquid-asset-registry register-asset --asset-id <asset-id> --contract <contract-json>`

## Deleting assets metadata

Note: deleting an asset only removes its metadata from the registry, not the asset itself.

To delete an asset, sign the following message using your `issuer_pubkey`:

```
remove <asset-id> from registry
```

Then submit the deletion request with the base64-encoded signature, like so:

```
$  curl -X DELETE https://assets.blockstream.info/<asset-id> -H 'Content-Type: application/json' \
    -d '{"signature":"<base64-encoded-signature>"}'
```