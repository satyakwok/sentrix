# API Reference

Complete list of Sentrix REST + JSON-RPC endpoints.

Base URL: `https://testnet-rpc.sentriscloud.com` (testnet) or `https://sentrix-rpc.sentriscloud.com` (mainnet).

---

## REST Endpoints

### Public (no auth)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Node info (name, version, chain_id, links) |
| GET | `/health` | Health check (`{"status":"ok"}`) |
| GET | `/metrics` | Prometheus-format metrics (block height, validators, mempool, uptime) |
| GET | `/chain/info` | Chain stats (height, supply, validators, mempool, etc.) |
| GET | `/chain/blocks?page=0&limit=20` | Paginated block list (newest first, max 100) |
| GET | `/chain/blocks/{index}` | Single block by height |
| GET | `/chain/validate` | Full chain validation (slow, auth recommended) |
| GET | `/chain/state-root/{height}` | State root hash at given height |
| GET | `/accounts/{address}/balance` | Account balance (sentri + SRX) |
| GET | `/accounts/{address}/nonce` | Account nonce |
| GET | `/validators` | List all validators (address, name, status, blocks produced) |
| GET | `/mempool` | Current mempool contents |
| GET | `/transactions?page=0&limit=20` | Latest transactions (paginated) |
| GET | `/transactions/{txid}` | Transaction detail by txid |
| GET | `/tokens` | List all deployed SRX-20 tokens |
| GET | `/tokens/{contract}` | Token info (name, symbol, supply, owner) |
| GET | `/tokens/{contract}/balance/{address}` | Token balance for address |
| GET | `/tokens/{contract}/holders` | Token holder list (sorted by balance) |
| GET | `/tokens/{contract}/trades?limit=20&offset=0` | Token trade history |
| GET | `/richlist` | Top accounts by SRX balance |
| GET | `/address/{address}/history?limit=20&offset=0` | Address transaction history |
| GET | `/address/{address}/info` | Address info (balance, nonce, tx count, is_contract) |
| GET | `/address/{address}/proof` | Merkle inclusion proof from state trie |
| GET | `/staking/validators` | DPoS validator set (Voyager) |
| GET | `/staking/delegations/{address}` | Delegations for address |
| GET | `/staking/unbonding/{address}` | Unbonding entries for address |
| GET | `/epoch/current` | Current epoch info |
| GET | `/epoch/history` | Epoch history |
| GET | `/stats/daily` | Daily tx + block count (last 14 days, for charts) |
| GET | `/admin/log` | Admin operation audit trail (requires API key) |

### Write (requires `X-API-Key` header, rate-limited 10 req/min)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/transactions` | Submit a signed native SRX transaction |
| POST | `/tokens/deploy` | Deploy a new SRX-20 token (signed tx) |
| POST | `/tokens/{contract}/transfer` | Token transfer (signed tx) |
| POST | `/tokens/{contract}/burn` | Token burn (signed tx) |
| POST | `/rpc` | JSON-RPC 2.0 dispatcher (single + batch) |

### Explorer (HTML)

| Path | Page |
|------|------|
| `/explorer` | Home (stats, charts, recent blocks + txs) |
| `/explorer/blocks` | Block list |
| `/explorer/transactions` | Transaction list |
| `/explorer/validators` | Validator table |
| `/explorer/tokens` | Token list |
| `/explorer/richlist` | Rich list |
| `/explorer/mempool` | Mempool contents |
| `/explorer/block/{index}` | Block detail |
| `/explorer/tx/{txid}` | Transaction detail (with EVM badges for type + status) |
| `/explorer/address/{address}` | Address page (balance, history) |
| `/explorer/validator/{address}` | Validator detail |
| `/explorer/token/{contract}` | Token detail |

---

## JSON-RPC Methods

POST to `/rpc` with `Content-Type: application/json`.

### Ethereum-compatible

| Method | Description |
|--------|-------------|
| `eth_chainId` | Chain ID (hex) |
| `eth_blockNumber` | Latest block number (hex) |
| `eth_getBalance` | Account balance in wei (hex) |
| `eth_getTransactionCount` | Account nonce (hex) |
| `eth_getCode` | Contract bytecode at address |
| `eth_getStorageAt` | Contract storage slot value |
| `eth_call` | Read-only EVM execution (no tx, no gas cost) |
| `eth_estimateGas` | Estimate gas for a tx |
| `eth_gasPrice` | Current gas price (hex) |
| `eth_sendRawTransaction` | Submit RLP-encoded signed Ethereum tx |
| `eth_getTransactionByHash` | Tx detail by hash |
| `eth_getTransactionReceipt` | Tx receipt (status, gasUsed, blockNumber) |
| `eth_getBlockByNumber` | Block by number |
| `eth_getBlockByHash` | Block by hash |
| `net_version` | Network ID (string) |
| `net_listening` | Always `true` |

### Sentrix-specific

| Method | Description |
|--------|-------------|
| `sentrix_sendTransaction` | Submit a pre-signed Sentrix native transaction |
| `sentrix_getBalance` | Balance in SRX (float string) |

### Batch requests

Send an array of JSON-RPC objects. Max batch size: 100.

```json
[
  {"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1},
  {"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":2}
]
```

---

## Rate Limits

| Scope | Limit | Window |
|-------|-------|--------|
| Global (all endpoints) | 60 req/IP | 60s |
| Write endpoints (POST /transactions, /tokens/*, /rpc) | 10 req/IP | 60s |
| Body size | 1 MiB max | per request |
| Batch RPC | 100 items max | per request |
| Concurrency | 500 simultaneous | per node |

---

## Authentication

POST endpoints require `X-API-Key` header when `SENTRIX_API_KEY` env var is set on the node. If not set, all requests are allowed (development mode).

```bash
curl -X POST http://localhost:8545/transactions \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key-here" \
  -d '{"transaction": { ... }}'
```

---

## Error Format

REST errors:
```json
{"success": false, "error": "error message"}
```

JSON-RPC errors:
```json
{"jsonrpc":"2.0","error":{"code":-32602,"message":"invalid params"},"id":1}
```

Standard JSON-RPC error codes: -32700 (parse), -32600 (invalid request), -32601 (method not found), -32602 (invalid params), -32603 (internal).
