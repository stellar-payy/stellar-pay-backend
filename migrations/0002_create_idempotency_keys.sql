CREATE TABLE idempotency_keys (
    key TEXT PRIMARY KEY,

    payment_id UUID NOT NULL
        REFERENCES payments(id),

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
