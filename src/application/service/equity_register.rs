//! Cap-table registration: register a share class + register a shareholder (hand-authored, user-owned).
//!
//! An `impl EquityWriteService` chunk over the vocabulary in [`super::equity_write_service`]. Per the
//! module's 4-layer rule this file holds no SQL — the inserts live on `ShareClassRepository` /
//! `ShareholderRepository`, which take the caller's pool so the insert rides the company scope.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{NewShareClassRow, NewShareholderRow};

use super::equity_write_service::{EquityError, EquityWriteService, NewShareClass, NewShareholder};

impl EquityWriteService {
    pub async fn register_share_class(&self, c: NewShareClass) -> Result<Uuid, EquityError> {
        if c.par_value < Decimal::ZERO {
            return Err(EquityError::Invalid("par value must be non-negative".into()));
        }
        let id = Uuid::new_v4();
        // RLS scope (ADR-0008): the company is on the DTO — bind it for the insert so it runs with
        // `app.company_id` set (a bare-pool insert is rejected by the fence's WITH CHECK).
        company_scope::with_company_scope(
            Some(c.company_id),
            self.share_classes.insert_share_class(&self.pool, &NewShareClassRow {
                id,
                company_id: c.company_id,
                code: &c.code,
                name: &c.name,
                par_value: c.par_value,
                currency: &c.currency,
                share_capital_account_id: c.share_capital_account_id,
                share_premium_account_id: c.share_premium_account_id,
            }),
        )
        .await?;
        Ok(id)
    }

    pub async fn register_shareholder(&self, s: NewShareholder) -> Result<Uuid, EquityError> {
        let id = Uuid::new_v4();
        // RLS scope (ADR-0008): company on the DTO — see `register_share_class`.
        company_scope::with_company_scope(
            Some(s.company_id),
            self.shareholders.insert_shareholder(&self.pool, &NewShareholderRow {
                id,
                company_id: s.company_id,
                party_id: s.party_id,
                name: &s.name,
                holder_type: &s.holder_type,
            }),
        )
        .await?;
        Ok(id)
    }
}
