//! In-tree reference implementation of the read contract [`EquityQueryService`] (hand-authored,
//! user-owned; survives regen). Proves the contract is realizable: each read goes through the entity
//! repository and maps the storage entity to its DTO.
//!
//! Reads respect the **ambient company scope** (RLS): the trait takes only an id — by design — so the
//! caller (composing backend-service or test) must set `app.company_id` via `backbone_orm::company_scope`
//! for the read to see the row. This mirrors how the validated write surface scopes its own reads.
//!
//! Gated behind `unstable-write-service` alongside the trait: this is a reference impl + composition
//! proof, not a deployed service. A composing service may provide its own impl.

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

use crate::domain::entity::{AuditMetadata, Dividend, ShareClass, Shareholder, ShareTransaction};
use crate::exports::{
    DividendDto, DividendId, DividendSummary, EquityQueryService, ShareClassDto, ShareClassId,
    ShareClassSummary, ShareTransactionDto, ShareTransactionId, ShareTransactionSummary,
    ShareholderDto, ShareholderId, ShareholderSummary,
};
use crate::infrastructure::persistence::{
    DividendRepository, ShareClassRepository, ShareholderRepository, ShareTransactionRepository,
};

/// Reference `EquityQueryService` over the four entity repositories.
pub struct EquityQueryServiceImpl {
    dividends: DividendRepository,
    share_classes: ShareClassRepository,
    shareholders: ShareholderRepository,
    transactions: ShareTransactionRepository,
}

impl EquityQueryServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            dividends: DividendRepository::new(pool.clone()),
            share_classes: ShareClassRepository::new(pool.clone()),
            shareholders: ShareholderRepository::new(pool.clone()),
            transactions: ShareTransactionRepository::new(pool),
        }
    }
}

fn meta(m: &AuditMetadata) -> serde_json::Value {
    serde_json::to_value(m).unwrap_or(serde_json::Value::Null)
}

#[async_trait]
impl EquityQueryService for EquityQueryServiceImpl {
    async fn get_dividend(&self, id: DividendId) -> Result<Option<DividendDto>> {
        let e: Option<Dividend> = self.dividends.find_by_id(&id.0.to_string()).await?;
        Ok(e.map(|e| DividendDto {
            id: DividendId(e.id),
            company_id: e.company_id,
            share_class_id: e.share_class_id,
            declaration_date: e.declaration_date,
            payment_date: e.payment_date,
            per_share_amount: e.per_share_amount,
            shares_outstanding: e.shares_outstanding,
            total_amount: e.total_amount,
            status: e.status,
            retained_earnings_account_id: e.retained_earnings_account_id,
            dividend_payable_account_id: e.dividend_payable_account_id,
            metadata: meta(&e.metadata),
        }))
    }

    async fn get_dividend_summary(&self, id: DividendId) -> Result<Option<DividendSummary>> {
        let e: Option<Dividend> = self.dividends.find_by_id(&id.0.to_string()).await?;
        Ok(e.map(|e| DividendSummary { id: DividendId(e.id), status: e.status }))
    }

    async fn dividend_exists(&self, id: DividendId) -> Result<bool> {
        Ok(self.dividends.exists(&id.0.to_string()).await?)
    }

    async fn get_share_class(&self, id: ShareClassId) -> Result<Option<ShareClassDto>> {
        let e: Option<ShareClass> = self.share_classes.find_by_id(&id.0.to_string()).await?;
        Ok(e.map(|e| ShareClassDto {
            id: ShareClassId(e.id),
            company_id: e.company_id,
            code: e.code,
            name: e.name,
            par_value: e.par_value,
            currency: e.currency,
            share_capital_account_id: e.share_capital_account_id,
            share_premium_account_id: e.share_premium_account_id,
            is_active: e.is_active,
            metadata: meta(&e.metadata),
        }))
    }

    async fn get_share_class_summary(&self, id: ShareClassId) -> Result<Option<ShareClassSummary>> {
        let e: Option<ShareClass> = self.share_classes.find_by_id(&id.0.to_string()).await?;
        Ok(e.map(|e| ShareClassSummary { id: ShareClassId(e.id), name: e.name }))
    }

    async fn share_class_exists(&self, id: ShareClassId) -> Result<bool> {
        Ok(self.share_classes.exists(&id.0.to_string()).await?)
    }

    async fn get_shareholder(&self, id: ShareholderId) -> Result<Option<ShareholderDto>> {
        let e: Option<Shareholder> = self.shareholders.find_by_id(&id.0.to_string()).await?;
        Ok(e.map(|e| ShareholderDto {
            id: ShareholderId(e.id),
            company_id: e.company_id,
            party_id: e.party_id,
            name: e.name,
            holder_type: e.holder_type,
            metadata: meta(&e.metadata),
        }))
    }

    async fn get_shareholder_summary(&self, id: ShareholderId) -> Result<Option<ShareholderSummary>> {
        let e: Option<Shareholder> = self.shareholders.find_by_id(&id.0.to_string()).await?;
        Ok(e.map(|e| ShareholderSummary { id: ShareholderId(e.id), name: e.name }))
    }

    async fn shareholder_exists(&self, id: ShareholderId) -> Result<bool> {
        Ok(self.shareholders.exists(&id.0.to_string()).await?)
    }

    async fn get_share_transaction(&self, id: ShareTransactionId) -> Result<Option<ShareTransactionDto>> {
        let e: Option<ShareTransaction> = self.transactions.find_by_id(&id.0.to_string()).await?;
        Ok(e.map(|e| ShareTransactionDto {
            id: ShareTransactionId(e.id),
            company_id: e.company_id,
            share_class_id: e.share_class_id,
            shareholder_id: e.shareholder_id,
            txn_type: e.txn_type,
            quantity: e.quantity,
            price_per_share: e.price_per_share,
            amount: e.amount,
            counterparty_shareholder_id: e.counterparty_shareholder_id,
            transfer_group_id: e.transfer_group_id,
            posting_reference: e.posting_reference,
            txn_date: e.txn_date,
            gl_posted: e.gl_posted,
            metadata: meta(&e.metadata),
        }))
    }

    async fn get_share_transaction_summary(
        &self,
        id: ShareTransactionId,
    ) -> Result<Option<ShareTransactionSummary>> {
        let e: Option<ShareTransaction> = self.transactions.find_by_id(&id.0.to_string()).await?;
        Ok(e.map(|e| ShareTransactionSummary { id: ShareTransactionId(e.id) }))
    }

    async fn share_transaction_exists(&self, id: ShareTransactionId) -> Result<bool> {
        Ok(self.transactions.exists(&id.0.to_string()).await?)
    }
}
