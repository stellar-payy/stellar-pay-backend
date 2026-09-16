CREATE TABLE webhook_events (
    id UUID PRIMARY KEY,

    payment_id UUID NOT NULL
        REFERENCES payments(id),

    event_type TEXT NOT NULL,

    payload JSONB NOT NULL,

    status TEXT NOT NULL DEFAULT 'pending',

    attempts INTEGER NOT NULL DEFAULT 0,

    next_attempt_at TIMESTAMPTZ,

    delivered_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_webhook_events_payment_id
ON webhook_events(payment_id);

CREATE INDEX idx_webhook_events_status
ON webhook_events(status);
