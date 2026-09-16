ALTER TABLE payments
    ADD COLUMN onchain_recorded_at TIMESTAMPTZ,
    ADD COLUMN onchain_tx_hash TEXT;
