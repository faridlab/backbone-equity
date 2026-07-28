//! Equity lifecycle events (hand-authored, user-owned). Emitted after a register movement / dividend leg
//! commits; staged in the transactional outbox in the SAME tx as the state change so they survive a crash
//! between commit and the in-proc publish. A `notification`/reporting consumer subscribes to them.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EquityEvent {
    /// New shares issued to a holder — the cap table grew and a capital journal posted.
    SharesIssued {
        transaction_id: Uuid,
        company_id: Uuid,
        share_class_id: Uuid,
        shareholder_id: Uuid,
        quantity: Decimal,
        amount: Decimal,
    },
    /// A dividend was declared on a class — the payable is booked, cash not yet out.
    DividendDeclared {
        dividend_id: Uuid,
        company_id: Uuid,
        share_class_id: Uuid,
        total_amount: Decimal,
    },
    /// A declared dividend was paid — the payable is settled.
    DividendPaid {
        dividend_id: Uuid,
        company_id: Uuid,
        total_amount: Decimal,
    },
}

impl EquityEvent {
    /// Every variant carries the tenant the event belongs to. Extracted BEFORE serialization because
    /// serde wraps an enum as `{"VariantName": { ... }}` — a top-level `payload.get("company_id")` on
    /// the serialized value returns `None` (the top-level keys are variant names, not fields), which
    /// would make every equity outbox stage fail the ADR-0011 fence. Typed access has no such hazard.
    pub fn company_id(&self) -> Uuid {
        match self {
            Self::SharesIssued { company_id, .. }
            | Self::DividendDeclared { company_id, .. }
            | Self::DividendPaid { company_id, .. } => *company_id,
        }
    }
}

/// Where equity publishes its lifecycle events (in-process). Durability is the outbox's job, not the sink's.
pub trait EquityEventSink: Send + Sync {
    fn publish(&self, event: &EquityEvent);
}

/// A no-op sink that just logs — the default when no consumer is wired.
pub struct LoggingSink;
impl EquityEventSink for LoggingSink {
    fn publish(&self, _event: &EquityEvent) {}
}
