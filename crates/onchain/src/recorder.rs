use async_trait::async_trait;
use stellar_pay_core::Payment;
use thiserror::Error;

// === Errors

#[derive(Debug, Error)]
pub enum OnChainError {
    #[error("on-chain recording failed: {0}")]
    Failed(String),
}

// === Recorder trait

// Best-effort audit trail seam. Horizon verification stays authoritative,
// so a recorder failure must never fail reconciliation.
#[async_trait]
pub trait OnChainRecorder: Send + Sync {
    async fn record_payment(&self, payment: &Payment) -> Result<(), OnChainError>;
}

// === Null implementation

// Default for v0.1: exercises the seam without a real Soroban RPC client.
// The real implementation (XDR construction, simulate/sign/submit/poll
// against `STELLAR_SOROBAN_RPC_URL`) is an immediate fast-follow once a
// Soroban RPC client approach is chosen.
pub struct NullOnChainRecorder;

impl NullOnChainRecorder {
    pub fn new() -> Self {
        return NullOnChainRecorder;
    }
}

impl Default for NullOnChainRecorder {
    fn default() -> Self {
        return NullOnChainRecorder::new();
    }
}

#[async_trait]
impl OnChainRecorder for NullOnChainRecorder {
    async fn record_payment(&self, payment: &Payment) -> Result<(), OnChainError> {
        tracing::info!(payment_id = %payment.id, "on-chain recording skipped (NullOnChainRecorder)");
        return Ok(());
    }
}

// === Tests

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_pay_core::Payment;

    #[tokio::test]
    async fn null_recorder_always_succeeds() {
        let recorder = NullOnChainRecorder::new();
        let payment = Payment::new(
            "10".to_string(),
            "ref-1".to_string(),
            "GDEST".to_string(),
            None,
        );

        assert!(recorder.record_payment(&payment).await.is_ok());
    }
}
