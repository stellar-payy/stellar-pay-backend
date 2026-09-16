# API reference

Base URL: `http://localhost:3000` (or wherever `BIND_ADDRESS` binds).

All responses are JSON. Errors are `{"error": "<message>"}` with an
appropriate HTTP status code.

## Endpoints

| Method | Path | Description |
| --- | --- | --- |
| GET | `/health` | Liveness check |
| POST | `/v1/payments` | Create a payment |
| GET | `/v1/payments` | List payments, paginated |
| GET | `/v1/payments/{id}` | Fetch a single payment |
| GET | `/v1/payments/{id}/status` | Fetch just a payment's status |
| GET | `/v1/payments/{id}/webhook-events` | List webhook delivery attempts for a payment |

## `GET /health`

```json
{ "status": "ok" }
```

## `POST /v1/payments`

Request body:

```json
{
  "amount": "10.5000000",
  "reference": "order-42",
  "destination": null,
  "memo": null
}
```

- `amount`: required. A decimal string, `> 0`, at most 7 fractional digits.
- `reference`: required. Must be unique across all payments.
- `destination`: optional. Defaults to `STELLAR_PAYMENT_ADDRESS`.
- `memo`: optional. Defaults to a generated 16 hex character correlation
  code (Stellar `MEMO_TEXT` caps at 28 bytes, so a full UUID does not fit).

Optional header:

- `Idempotency-Key`: if supplied, a repeated request with the same key
  returns the original payment instead of creating a new one or conflicting
  on `reference`.

Response: `201 Created`

```json
{
  "id": "b3f1c2b0-...-...",
  "reference": "order-42",
  "amount": "10.5000000",
  "asset": "XLM",
  "destination": "GXXXX...",
  "memo": "a1b2c3d4e5f60718",
  "status": "pending",
  "stellar_tx_hash": null,
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z"
}
```

`status` is always lowercase (`pending`, `detected`, `confirmed`, `failed`,
`expired`). `asset` is always `"XLM"`.

Error cases: `400` invalid amount, `409` duplicate reference.

## `GET /v1/payments`

Query parameters:

- `page`: optional, default `1`.
- `per_page`: optional, default `20`, clamped to `[1, 100]`.
- `status`: optional, one of `pending`, `detected`, `confirmed`, `failed`,
  `expired`.

Response:

```json
{
  "data": [ /* PaymentResponse[] */ ],
  "page": 1,
  "per_page": 20,
  "total": 42
}
```

## `GET /v1/payments/{id}`

Response: a single `PaymentResponse` (same shape as the create response).

Error cases: `404` if the payment does not exist.

## `GET /v1/payments/{id}/status`

Response:

```json
{ "id": "b3f1c2b0-...-...", "status": "confirmed" }
```

## `GET /v1/payments/{id}/webhook-events`

Response:

```json
{
  "data": [
    {
      "id": "e1a2...",
      "payment_id": "b3f1c2b0-...-...",
      "event_type": "payment.confirmed",
      "status": "delivered",
      "attempts": 1,
      "next_attempt_at": null,
      "delivered_at": "2026-01-01T00:05:00Z",
      "created_at": "2026-01-01T00:04:50Z"
    }
  ]
}
```

`status` is one of `pending`, `delivered`, `failed`.

## Webhooks

Each `PaymentEvent` (`payment.created`, `payment.detected`,
`payment.confirmed`, `payment.failed`, `payment.expired`) is delivered as a
signed `POST` to `WEBHOOK_URL`:

```json
{
  "id": "envelope-uuid",
  "type": "payment.confirmed",
  "created_at": "2026-01-01T00:05:00Z",
  "data": { "payment": { /* PaymentResponse-shaped payment */ } }
}
```

The request carries:

```
X-Stellar-Pay-Signature: sha256=<hex hmac-sha256 of the raw request body>
```

Verify it by computing `HMAC-SHA256(WEBHOOK_SECRET, raw_body)`, hex-encoding
it, and comparing in constant time against the value after `sha256=`.
`stellar_pay_webhooks::verify_signature` implements exactly this.

Delivery retries on a fixed backoff schedule of `1m, 5m, 15m, 1h, 6h` after
a non-2xx response or a request error. After the schedule is exhausted, the
event is marked `failed` and is not retried further; its history remains
visible through `GET /v1/payments/{id}/webhook-events`.
