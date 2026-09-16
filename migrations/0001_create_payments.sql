CREATE TABLE payments (
    id UUID PRIMARY KEY,

    reference TEXT NOT NULL UNIQUE,

    amount NUMERIC(20, 7) NOT NULL,

    asset TEXT NOT NULL
        CHECK (asset = 'XLM'),

    destination TEXT NOT NULL,

    memo TEXT,

    status TEXT NOT NULL,

    stellar_tx_hash TEXT UNIQUE,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_payments_status
ON payments(status);

CREATE INDEX idx_payments_reference
ON payments(reference);

CREATE INDEX idx_payments_tx_hash
ON payments(stellar_tx_hash);
