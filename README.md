# stellar-pay-backend

Open-source Rust payment infrastructure for accepting, monitoring, verifying,
and reconciling XLM payments on Stellar.

Merchants create a payment through the API, the payer sends XLM to a shared
platform destination address with a short correlation memo, a background
reconciliation worker matches the payment against Horizon and confirms it,
and a background delivery worker signs and posts webhook notifications for
each status change.

## Scope

| Area | Status |
| --- | --- |
| Payment lifecycle (create, detect, confirm, fail, expire) | v0.1, in this repo |
| Horizon-based memo matching and reconciliation | v0.1, in this repo |
| Signed webhook delivery with fixed backoff | v0.1, in this repo |
| REST API (`crates/api`) and CLI (`crates/cli`) | v0.1, in this repo |
| On-chain audit trail (Soroban payment registry) | seam only, `crates/onchain`'s `NullOnChainRecorder`; real Soroban RPC call is a fast-follow |
| Per-payment destination accounts | out of scope, single shared destination account only |
| Merchant dashboard | separate repo, `stellar-pay-frontend` |
| On-chain payment registry contract | separate repo, `stellar-pay-contract` |

## Workspace layout

```
crates/
  core            domain types (Payment, PaymentStatus, PaymentEvent), no I/O
  stellar         Horizon client (StellarClient trait, HorizonClient, MockStellarClient)
  payments        PaymentRepository, PaymentService, amount validation, idempotency
  webhooks        WebhookEventRepository, signing, fixed backoff, delivery worker
  reconciliation  ReconciliationWorker: expiry, memo matching, status transitions
  onchain         OnChainRecorder seam, NullOnChainRecorder
  api             axum HTTP API (stellar-pay-api)
  cli             clap CLI: serve / migrate (stellar-pay-cli)
```

## Quickstart

```bash
cp .env.example .env
docker compose up -d
cargo run -p stellar-pay-cli -- migrate
cargo run -p stellar-pay-api
```

The API binds to `0.0.0.0:3000` by default (override with `BIND_ADDRESS`).

Equivalently, `cargo run -p stellar-pay-cli -- serve` starts the same server
through the CLI, since the CLI's `serve` subcommand calls directly into
`stellar_pay_api::run_server`.

## API examples

Create a payment (uses `STELLAR_PAYMENT_ADDRESS` as the destination since
none is given, and generates a memo since none is given):

```bash
curl -X POST localhost:3000/v1/payments \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: order-42' \
  -d '{"amount": "10.5000000", "reference": "order-42"}'
```

A successful response is a `201` with a `pending` payment (status is
lowercase, asset is `"XLM"`).

List payments:

```bash
curl 'localhost:3000/v1/payments?status=pending&page=1&per_page=20'
```

Fetch one payment:

```bash
curl localhost:3000/v1/payments/<id>
```

Fetch just its status:

```bash
curl localhost:3000/v1/payments/<id>/status
```

Fetch its webhook delivery history:

```bash
curl localhost:3000/v1/payments/<id>/webhook-events
```

See `docs/api.md` for the full endpoint reference and webhook payload/signature
format.

## Development

```bash
cargo build --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

One idempotency test is Postgres-backed and `#[ignore]`d by default. Run it
with infrastructure up:

```bash
docker compose up -d
cargo run -p stellar-pay-cli -- migrate
cargo test -- --ignored
```

See `docs/architecture.md` for the payment lifecycle and memo-matching design,
and `CONTRIBUTING.md` for coding conventions.
