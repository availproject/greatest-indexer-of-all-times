# Configuration

The indexer reads configuration from:
- `CONFIG` env var pointing to a JSON file (optional)
- Environment variables (highest priority)
- Defaults (if available)

## Configuration Parameters
- `db_url` (required): Postgres connection string.
- `avail_url` (optional): Avail RPC URL. Defaults to `https://mainnet-rpc.avail.so/rpc` when not provided.
- `table_name` (optional): Table name. Defaults to `avail_table`.
- `block_height` (optional): Start from this block height. If missing, uses the latest stored block height from the DB.


## config.json example
```json
{
  "db_url": "postgres://user:pass@host:5432/dbname",
  "avail_url": "https://mainnet-rpc.avail.so/rpc",
  "table_name": "avail_indexer",
  "block_height": 1903463
}
```

```bash
CONFIG=./config.json cargo run
```

## Env-only example
```bash
DB_URL=postgres://user:pass@host:5432/dbname \
AVAIL_URL=https://mainnet-rpc.avail.so/rpc \
TABLE_NAME=avail_indexer \
BLOCK_HEIGHT=1903463 \
cargo run
```

# What we store
- One row per extrinsic, keyed by `id BIGINT` (block_height << 32 | ext_index).
- Columns: `block_height`, `block_hash`, `block_timestamp`, `ext_index`, `ext_hash`, optional `signature_address`, `pallet_id`, `variant_id`, optional `ext_success`, and JSON/text payload `ext_call`.
- On `id` conflict, the row is upserted (existing row is updated with new values).
