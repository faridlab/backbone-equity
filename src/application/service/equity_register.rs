//! Cap-table registration: register a share class + register a shareholder (hand-authored, user-owned).
//!
//! An `impl EquityWriteService` chunk over the vocabulary in [`super::equity_write_service`]. Per the
//! module's 4-layer rule this file holds no SQL — the inserts live on `ShareClassRepository` /
//! `ShareholderRepository`, which ride the request-dedicated connection when the composing service
//! bound one; the database fence (if any) decides what the insert may write.

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{NewShareClassRow, NewShareholderRow};

use super::equity_write_service::{EquityError, EquityWriteService, NewShareClass, NewShareholder};

impl EquityWriteService {
    pub async fn register_share_class(&self, c: NewShareClass) -> Result<Uuid, EquityError> {
        if c.par_value < Decimal::ZERO {
            return Err(EquityError::Invalid(
                "par value must be non-negative".into(),
            ));
        }
        let id = Uuid::new_v4();
        self.share_classes
            .insert_share_class(
                &self.pool,
                &NewShareClassRow {
                    id,
                    code: &c.code,
                    name: &c.name,
                    par_value: c.par_value,
                    currency: &c.currency,
                    share_capital_account_id: c.share_capital_account_id,
                    share_premium_account_id: c.share_premium_account_id,
                },
            )
            .await?;
        Ok(id)
    }

    pub async fn register_shareholder(&self, s: NewShareholder) -> Result<Uuid, EquityError> {
        let id = Uuid::new_v4();
        self.shareholders
            .insert_shareholder(
                &self.pool,
                &NewShareholderRow {
                    id,
                    party_id: s.party_id,
                    name: &s.name,
                    holder_type: &s.holder_type,
                },
            )
            .await?;
        Ok(id)
    }
}
