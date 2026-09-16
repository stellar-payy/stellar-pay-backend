# Architecture

## Crate dependency graph

```
stellar-pay-core
    ^
    |-- stellar-pay-stellar
    |-- stellar-pay-payments  --(depends on)--> stellar-pay-core
    |-- stellar-pay-webhooks  --(depends on)--> stellar-pay-core
    |-- stellar-pay-onchain   --(depends on)--> stellar-pay-core

stellar-pay-reconciliation --> stellar-pay-payments, stellar-pay-stellar,
                                stellar-pay-webhooks, stellar-pay-onchain

stellar-pay-api            --> stellar-pay-payments, stellar-pay-stellar,
                                stellar-pay-webhooks, stellar-pay-onchain,
                                stellar-pay-reconciliation

stellar-pay-cli            --> stellar-pay-api
```

`stellar-pay-core` has no database or async awareness (no `async-trait`, no
`sqlx`), so it stays a dependency-light domain model reusable by every
crate. `PaymentRepository` lives in `stellar-pay-payments`, not `core`, for
the same reason.

## Component overview

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

## Payment lifecycle

```
Pending --(memo + amount match found on Horizon)--> Detected
Pending --(reconciliation expiry window elapsed)--> Expired
Detected --(Horizon transaction marked successful)--> Confirmed
Detected --(Horizon transaction marked unsuccessful)--> Failed
```

`PaymentStatus::can_transition_to` in `crates/core/src/payment.rs` is the
single source of truth for this state machine. `PaymentRepository::update_status`
derives the valid "from" states for a target status from that method and
performs an atomic `UPDATE ... WHERE status = ANY(valid_from_states)
RETURNING *`, so there is no read-then-write race between two workers (or a
worker and an API request) racing the same payment.

Each transition emits a `PaymentEvent` and enqueues a webhook. On
`Confirmed`, the reconciliation worker also calls `OnChainRecorder::record_payment`
best-effort; a failure there is logged but never fails the reconciliation
pass, since Horizon verification stays authoritative.

## Memo-based matching and its limitations

`STELLAR_PAYMENT_ADDRESS` is a single shared destination account, so
payments cannot be correlated by `(destination, amount, time-window)` alone:
two payers could send the same amount around the same time. Instead,
`create_payment` generates a short correlation code (16 hex characters, half
of a UUID v4) when the caller omits `memo`, since Stellar's `MEMO_TEXT`
caps at 28 bytes and a full UUID does not fit.

`StellarClient::find_payment(destination, memo, since)` looks up recent
payments to the destination account and reads each candidate transaction's
memo to find a match. `amounts_match` (in `stellar-pay-reconciliation`) is a
defense-in-depth check layered on top: a payer who reuses or guesses a memo
but sends the wrong amount is not auto-confirmed.

Known limitations:

- Communicating the generated memo to the payer is outside this repo (a
  checkout UI concern).
- A per-payment unique destination account would remove the need for memo
  matching entirely, but requires account generation/funding and is a
  materially different, more complex design, out of scope for v0.1.
- `HorizonClient::find_payment` fetches one page of recent payments; a very
  high-volume destination account could need cursor-based pagination
  through Horizon in a future revision.

## Reconciliation and webhook delivery

Both run as background `tokio::spawn` tasks started from `run_server`
(`crates/api/src/startup.rs`), not as separate scalable worker processes.
This is a deliberate "one process, batteries included" v0.1 choice. Each
loop logs and continues past errors rather than crashing the process.

## On-chain audit trail seam

`crates/onchain` defines the `OnChainRecorder` trait and ships
`NullOnChainRecorder`, which logs at `info` level and returns `Ok(())`. This
exercises the seam (config, trait boundary, call site in the reconciliation
worker) without a real Soroban RPC client dependency. The real
implementation, building the `record_payment` Soroban contract call with the
exact stroops conversion documented in `stellar-pay-contract`'s
`docs/integration.md`, is the documented immediate next step once that
contract is deployed and a Soroban RPC client approach is chosen.
