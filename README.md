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

`stellar-pay-core` has no database or async awareness (no `async-trait`, no
`sqlx`), so it stays a dependency-light domain model reusable by every
crate. `PaymentRepository` lives in `stellar-pay-payments`, not `core`, for
the same reason.

## Architecture

```
                          +------------------+
     HTTP client -------> |  stellar-pay-api  |
                          +--------+---------+
                                   |
                                   v
                         +-------------------+
                         | PaymentService     |
                         | (stellar-pay-      |
                         |  payments)         |
                         +---------+---------+
                                   |
                                   v
                         +-------------------+
                         | PaymentRepository  |  <-- Postgres or in-memory
                         +-------------------+

    (background, spawned from run_server)

    +------------------------+       +----------------------+
    | ReconciliationWorker    | ----> | StellarClient        |
    | (stellar-pay-           |       | (HorizonClient)      |
    |  reconciliation)        |       +----------------------+
    |                          |
    |                          | ----> | OnChainRecorder      |
    |                          |       | (NullOnChainRecorder)|
    |                          |
    |                          | ----> | WebhookService       |
    +------------------------+       | (stellar-pay-webhooks)|
                                       +----------+-----------+
                                                  |
                                                  v
                                         merchant webhook endpoint
```

Both the reconciliation loop and the webhook delivery loop run as background
`tokio::spawn` tasks started from `run_server` (`crates/api/src/startup.rs`) —
one process, batteries included for v0.1, not separate scalable worker
processes. Each loop logs and continues past errors rather than crashing the
process.

### Payment state machine

`PaymentStatus::can_transition_to` (`crates/core/src/payment.rs`) is the
single source of truth for every valid transition. `PaymentRepository::update_status`
derives the valid "from" states for a target status from this method and
performs an atomic `UPDATE ... WHERE status = ANY(valid_from_states) RETURNING *`,
so there is no read-then-write race between two workers (or a worker and an
API request) racing the same payment.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Pending,
    Detected,
    Confirmed,
    Failed,
    Expired,
}

impl PaymentStatus {
    // Only these transitions are valid. Anything else must be rejected by
    // the repository layer before a status update is persisted.
    pub fn can_transition_to(&self, next: &PaymentStatus) -> bool {
        return matches!(
            (self, next),
            (Self::Pending, Self::Detected)
                | (Self::Pending, Self::Expired)
                | (Self::Detected, Self::Confirmed)
                | (Self::Detected, Self::Failed)
        );
    }
}
```

```
Pending --(memo + amount match found on Horizon)--> Detected
Pending --(reconciliation expiry window elapsed)--> Expired
Detected --(Horizon transaction marked successful)--> Confirmed
Detected --(Horizon transaction marked unsuccessful)--> Failed
```

### Memo-based matching

`STELLAR_PAYMENT_ADDRESS` is a single shared destination account, so
payments cannot be correlated by `(destination, amount, time-window)` alone —
two payers could send the same amount around the same time. `create_payment`
generates a short correlation code (16 hex characters, half of a UUID v4)
when the caller omits `memo`, since Stellar's `MEMO_TEXT` caps at 28 bytes
and a full UUID does not fit. `StellarClient::find_payment(destination, memo, since)`
looks up recent payments to the destination account and reads each
candidate transaction's memo to find a match; `amounts_match` is layered on
top as defense-in-depth so a payer who reuses or guesses a memo but sends
the wrong amount is not auto-confirmed.

See `docs/architecture.md` for the full crate dependency graph, on-chain
audit trail seam, and known limitations.

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
