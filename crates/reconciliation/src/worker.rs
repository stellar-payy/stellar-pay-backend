use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use stellar_pay_core::{Payment, PaymentEvent, PaymentStatus};
use stellar_pay_onchain::OnChainRecorder;
use stellar_pay_payments::PaymentRepository;
use stellar_pay_stellar::StellarClient;
use stellar_pay_webhooks::WebhookService;

use crate::matching::amounts_match;

// === Reconciliation worker

pub struct ReconciliationWorker {
    repository: Arc<dyn PaymentRepository>,
    stellar_client: Arc<dyn StellarClient>,
    webhook_service: Arc<WebhookService>,
    onchain_recorder: Arc<dyn OnChainRecorder>,
    expiry_window: Duration,
}

impl ReconciliationWorker {
    pub fn new(
        repository: Arc<dyn PaymentRepository>,
        stellar_client: Arc<dyn StellarClient>,
        webhook_service: Arc<WebhookService>,
        onchain_recorder: Arc<dyn OnChainRecorder>,
        expiry_window: Duration,
    ) -> Self {
        return ReconciliationWorker {
            repository,
            stellar_client,
            webhook_service,
            onchain_recorder,
            expiry_window,
        };
    }

    pub async fn run_once(&self) -> anyhow::Result<()> {
        let pending = self.repository.list_pending_for_reconciliation().await?;

        for payment in pending {
            match payment.status {
                PaymentStatus::Pending => self.process_pending(payment).await?,
                PaymentStatus::Detected => self.process_detected(payment).await?,
                _ => {}
            }
        }

        return Ok(());
    }

    // Logs and continues past errors rather than crashing the process.
    pub async fn run_loop(&self, interval: Duration) {
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;

            if let Err(err) = self.run_once().await {
                tracing::error!(?err, "reconciliation pass failed");
            }
        }
    }

    async fn process_pending(&self, payment: Payment) -> anyhow::Result<()> {
        let age = Utc::now().signed_duration_since(payment.created_at);
        let expiry = chrono::Duration::from_std(self.expiry_window)
            .unwrap_or_else(|_| chrono::Duration::zero());

        if age >= expiry {
            let updated = self
                .repository
                .update_status(payment.id, PaymentStatus::Expired, None)
                .await?;
            self.emit(PaymentEvent::PaymentExpired { payment: updated })
                .await;
            return Ok(());
        }

        let Some(memo) = payment.memo.clone() else {
            return Ok(());
        };

        let found = self
            .stellar_client
            .find_payment(&payment.destination, &memo, payment.created_at)
            .await?;

        let Some(found) = found else {
            return Ok(());
        };

        if !amounts_match(&payment.amount, &found.amount) {
            tracing::warn!(payment_id = %payment.id, "memo matched but amount did not, leaving payment pending");
            return Ok(());
        }

        let updated = self
            .repository
            .update_status(
                payment.id,
                PaymentStatus::Detected,
                Some(found.transaction_hash),
            )
            .await?;

        self.emit(PaymentEvent::PaymentDetected { payment: updated })
            .await;

        return Ok(());
    }

    async fn process_detected(&self, payment: Payment) -> anyhow::Result<()> {
        let Some(tx_hash) = payment.stellar_tx_hash.clone() else {
            return Ok(());
        };

        let transaction = self.stellar_client.get_transaction(&tx_hash).await?;

        if transaction.successful {
            let updated = self
                .repository
                .update_status(payment.id, PaymentStatus::Confirmed, None)
                .await?;

            // Best-effort: Horizon verification stays authoritative, so a
            // recording failure here must not fail the reconciliation pass.
            if let Err(err) = self.onchain_recorder.record_payment(&updated).await {
                tracing::warn!(?err, payment_id = %updated.id, "on-chain recording failed, continuing");
            }

            self.emit(PaymentEvent::PaymentConfirmed { payment: updated })
                .await;
        } else {
            let updated = self
                .repository
                .update_status(payment.id, PaymentStatus::Failed, None)
                .await?;
            self.emit(PaymentEvent::PaymentFailed { payment: updated })
                .await;
        }

        return Ok(());
    }

    async fn emit(&self, event: PaymentEvent) {
        if let Err(err) = self.webhook_service.enqueue_event(&event).await {
            tracing::error!(?err, "failed to enqueue webhook event");
        }
    }
}
