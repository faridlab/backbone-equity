//! Transfer shares between two holders (hand-authored, user-owned) — an ownership change, NO GL.
//!
//! An `impl EquityWriteService` chunk over the vocabulary in [`super::equity_write_service`]. Bounds the
//! outgoing quantity against the sender's live holding under a per-(class,holder) advisory lock, so the
//! register can't go negative.
//!
//! Per the module's 4-layer rule this file holds no SQL — the position lock, the holding read, and the
//! two transfer-leg inserts live on `ShareTransactionRepository`, which takes this service's transaction
//! so the read + the paired write commit together.

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::NewTransferLegRow;

use super::equity_events::{EquityEvent, EquityEventSink};
use super::equity_write_service::{stage, EquityError, EquityWriteService, TransferShares};

impl EquityWriteService {
    /// Transfer shares between two holders — an ownership change, NO GL. Bounds the outgoing quantity against
    /// the sender's live holding under a per-(class,holder) advisory lock, so the register can't go negative.
    pub async fn transfer_shares(
        &self,
        t: TransferShares,
        events: &dyn EquityEventSink,
    ) -> Result<Uuid, EquityError> {
        if t.quantity <= Decimal::ZERO {
            return Err(EquityError::Invalid("quantity must be positive".into()));
        }
        if t.from_shareholder_id == t.to_shareholder_id {
            return Err(EquityError::Invalid(
                "cannot transfer to the same holder".into(),
            ));
        }
        // Propagate the ambient request scope onto our own tx (relay-only): the bounds check and the
        // movement inserts run on a fresh connection the composing service's fence does not decorate.
        // Unfenced deployments have no ambient scope and skip this entirely.
        let mut tx = self.pool.begin().await?;
        if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
            backbone_orm::org_scope::bind_org_scope_on(&mut *tx, &scope).await?;
        }
        // Serialize concurrent removals from this (class, holder): the holding is a SUM with no single row to
        // lock, so an advisory xact lock is the guard.
        self.transactions
            .lock_position(&mut tx, t.share_class_id, t.from_shareholder_id)
            .await?;
        let held = self
            .transactions
            .holding(&mut tx, t.share_class_id, t.from_shareholder_id)
            .await?;
        if t.quantity > held {
            return Err(EquityError::InsufficientShares {
                held,
                requested: t.quantity,
            });
        }
        let group = Uuid::new_v4();
        for (holder, ttype, cp) in [
            (t.from_shareholder_id, "transfer_out", t.to_shareholder_id),
            (t.to_shareholder_id, "transfer_in", t.from_shareholder_id),
        ] {
            self.transactions
                .insert_transfer_leg(
                    &mut tx,
                    &NewTransferLegRow {
                        id: Uuid::new_v4(),
                        share_class_id: t.share_class_id,
                        shareholder_id: holder,
                        txn_type: ttype,
                        quantity: t.quantity,
                        counterparty_shareholder_id: cp,
                        transfer_group_id: group,
                        txn_date: t.txn_date,
                    },
                )
                .await?;
        }
        let event = EquityEvent::SharesTransferred {
            transfer_group_id: group,
            share_class_id: t.share_class_id,
            from_shareholder_id: t.from_shareholder_id,
            to_shareholder_id: t.to_shareholder_id,
            quantity: t.quantity,
        };
        stage(
            &mut tx,
            "SharesTransferred",
            "ShareTransaction",
            group,
            &event,
        )
        .await?;
        tx.commit().await?;
        events.publish(&event);
        Ok(group)
    }
}
