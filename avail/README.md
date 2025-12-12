# Configuration

The indexer reads configuration from:
- `CONFIG` env var pointing to a JSON file (optional)
- Environment variables (highest priority)
- Defaults (if available)

## Configuration Parameters
- `db_url` (required): Postgres connection string.
- `avail_url` (optional): Avail RPC URL. Defaults to `https://mainnet-rpc.avail.so/rpc` when not provided.
- `table_name` (optional): Main Table name. Defaults to `avail_table`.
- `send_message_table_name` (optional): Send Message Table name. Defaults to `avail_send_message_table`.
- `execute_table_name` (optional): Execute Table name. Defaults to `avail_execute_table`.
- `block_height` (optional): Start from this block height. If missing, uses the latest stored block height from the DB.


## config.json example
```json
{
  "db_url": "postgres://user:pass@host:5432/dbname",
  "avail_url": "https://mainnet-rpc.avail.so/rpc",
  "table_name": "avail_indexer",
  "send_message_table_name": "avail_indexer_send_message",
  "execute_table_name": "avail_indexer_execute",
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
SEND_MESSAGE_TABLE_NAME=avail_indexer_send_message \
EXECUTE_TABLE_NAME=avail_indexer_execute \
BLOCK_HEIGHT=1903463 \
cargo run
```

## Database Tables

### Main Table (`table_name`)
```
- id: BIGINT PRIMARY KEY
- block_height: INTEGER NOT NULL
- block_hash: TEXT NOT NULL
- block_timestamp: TIMESTAMPTZ NOT NULL
- ext_index: INTEGER NOT NULL
- ext_hash: TEXT NOT NULL
- signature_address: TEXT (nullable)
- pallet_id: SMALLINT NOT NULL
- variant_id: SMALLINT NOT NULL
- ext_success: BOOL (nullable)
- ext_call: TEXT NOT NULL
```

### Send Message Table (`send_message_table_name`)
```
- id: BIGINT PRIMARY KEY REFERENCES main table id
- type: TEXT NOT NULL
- amount: TEXT (nullable)
- to: TEXT NOT NULL
```

### Execute Table (`execute_table_name`)
```
- id: BIGINT PRIMARY KEY REFERENCES main table id
- type: TEXT NOT NULL
- amount: TEXT (nullable)
- to: TEXT NOT NULL
- slot: BIGINT NOT NULL
- message_id: NUMERIC(78) NOT NULL
```
