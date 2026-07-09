use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::ShareTxnType;
use super::AuditMetadata;

/// Strongly-typed ID for ShareTransaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShareTransactionId(pub Uuid);

impl ShareTransactionId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ShareTransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ShareTransactionId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ShareTransactionId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ShareTransactionId> for Uuid {
    fn from(id: ShareTransactionId) -> Self { id.0 }
}

impl AsRef<Uuid> for ShareTransactionId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ShareTransactionId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ShareTransaction {
    pub id: Uuid,
    pub company_id: Uuid,
    pub share_class_id: Uuid,
    pub shareholder_id: Uuid,
    pub txn_type: ShareTxnType,
    pub quantity: Decimal,
    pub price_per_share: Decimal,
    pub amount: Decimal,
    pub counterparty_shareholder_id: Option<Uuid>,
    pub transfer_group_id: Option<Uuid>,
    pub posting_reference: Option<String>,
    pub txn_date: NaiveDate,
    pub gl_posted: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl ShareTransaction {
    /// Create a builder for ShareTransaction
    pub fn builder() -> ShareTransactionBuilder {
        ShareTransactionBuilder::default()
    }

    /// Create a new ShareTransaction with required fields
    pub fn new(company_id: Uuid, share_class_id: Uuid, shareholder_id: Uuid, txn_type: ShareTxnType, quantity: Decimal, price_per_share: Decimal, amount: Decimal, txn_date: NaiveDate, gl_posted: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            share_class_id,
            shareholder_id,
            txn_type,
            quantity,
            price_per_share,
            amount,
            counterparty_shareholder_id: None,
            transfer_group_id: None,
            posting_reference: None,
            txn_date,
            gl_posted,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> ShareTransactionId {
        ShareTransactionId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.created_at.as_ref()
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.updated_at.as_ref()
    }

    /// Check if this entity is soft deleted
    pub fn is_deleted(&self) -> bool {
        self.metadata.deleted_at.is_some()
    }

    /// Check if this entity is active (not deleted)
    pub fn is_active(&self) -> bool {
        self.metadata.deleted_at.is_none()
    }

    /// Get when this entity was deleted
    pub fn deleted_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.deleted_at.as_ref()
    }

    /// Get who created this entity
    pub fn created_by(&self) -> Option<&Uuid> {
        self.metadata.created_by.as_ref()
    }

    /// Get who last updated this entity
    pub fn updated_by(&self) -> Option<&Uuid> {
        self.metadata.updated_by.as_ref()
    }

    /// Get who deleted this entity
    pub fn deleted_by(&self) -> Option<&Uuid> {
        self.metadata.deleted_by.as_ref()
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the counterparty_shareholder_id field (chainable)
    pub fn with_counterparty_shareholder_id(mut self, value: Uuid) -> Self {
        self.counterparty_shareholder_id = Some(value);
        self
    }

    /// Set the transfer_group_id field (chainable)
    pub fn with_transfer_group_id(mut self, value: Uuid) -> Self {
        self.transfer_group_id = Some(value);
        self
    }

    /// Set the posting_reference field (chainable)
    pub fn with_posting_reference(mut self, value: String) -> Self {
        self.posting_reference = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "share_class_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.share_class_id = v; }
                }
                "shareholder_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.shareholder_id = v; }
                }
                "txn_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.txn_type = v; }
                }
                "quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity = v; }
                }
                "price_per_share" => {
                    if let Ok(v) = serde_json::from_value(value) { self.price_per_share = v; }
                }
                "amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.amount = v; }
                }
                "counterparty_shareholder_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.counterparty_shareholder_id = v; }
                }
                "transfer_group_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.transfer_group_id = v; }
                }
                "posting_reference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_reference = v; }
                }
                "txn_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.txn_date = v; }
                }
                "gl_posted" => {
                    if let Ok(v) = serde_json::from_value(value) { self.gl_posted = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for ShareTransaction {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "ShareTransaction"
    }
}

impl backbone_core::PersistentEntity for ShareTransaction {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.created_at
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.created_at = Some(ts);
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.updated_at
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.updated_at = Some(ts);
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.deleted_at
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        self.metadata.deleted_at = ts;
    }
}

impl backbone_orm::EntityRepoMeta for ShareTransaction {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("share_class_id".to_string(), "uuid".to_string());
        m.insert("shareholder_id".to_string(), "uuid".to_string());
        m.insert("counterparty_shareholder_id".to_string(), "uuid".to_string());
        m.insert("transfer_group_id".to_string(), "uuid".to_string());
        m.insert("txn_type".to_string(), "share_txn_type".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("shareClass", "share_classes", "shareClassId")]
    }
}

/// Builder for ShareTransaction entity
///
/// Provides a fluent API for constructing ShareTransaction instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ShareTransactionBuilder {
    company_id: Option<Uuid>,
    share_class_id: Option<Uuid>,
    shareholder_id: Option<Uuid>,
    txn_type: Option<ShareTxnType>,
    quantity: Option<Decimal>,
    price_per_share: Option<Decimal>,
    amount: Option<Decimal>,
    counterparty_shareholder_id: Option<Uuid>,
    transfer_group_id: Option<Uuid>,
    posting_reference: Option<String>,
    txn_date: Option<NaiveDate>,
    gl_posted: Option<bool>,
}

impl ShareTransactionBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the share_class_id field (required)
    pub fn share_class_id(mut self, value: Uuid) -> Self {
        self.share_class_id = Some(value);
        self
    }

    /// Set the shareholder_id field (required)
    pub fn shareholder_id(mut self, value: Uuid) -> Self {
        self.shareholder_id = Some(value);
        self
    }

    /// Set the txn_type field (required)
    pub fn txn_type(mut self, value: ShareTxnType) -> Self {
        self.txn_type = Some(value);
        self
    }

    /// Set the quantity field (required)
    pub fn quantity(mut self, value: Decimal) -> Self {
        self.quantity = Some(value);
        self
    }

    /// Set the price_per_share field (required)
    pub fn price_per_share(mut self, value: Decimal) -> Self {
        self.price_per_share = Some(value);
        self
    }

    /// Set the amount field (required)
    pub fn amount(mut self, value: Decimal) -> Self {
        self.amount = Some(value);
        self
    }

    /// Set the counterparty_shareholder_id field (optional)
    pub fn counterparty_shareholder_id(mut self, value: Uuid) -> Self {
        self.counterparty_shareholder_id = Some(value);
        self
    }

    /// Set the transfer_group_id field (optional)
    pub fn transfer_group_id(mut self, value: Uuid) -> Self {
        self.transfer_group_id = Some(value);
        self
    }

    /// Set the posting_reference field (optional)
    pub fn posting_reference(mut self, value: String) -> Self {
        self.posting_reference = Some(value);
        self
    }

    /// Set the txn_date field (required)
    pub fn txn_date(mut self, value: NaiveDate) -> Self {
        self.txn_date = Some(value);
        self
    }

    /// Set the gl_posted field (default: `false`)
    pub fn gl_posted(mut self, value: bool) -> Self {
        self.gl_posted = Some(value);
        self
    }

    /// Build the ShareTransaction entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<ShareTransaction, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let share_class_id = self.share_class_id.ok_or_else(|| "share_class_id is required".to_string())?;
        let shareholder_id = self.shareholder_id.ok_or_else(|| "shareholder_id is required".to_string())?;
        let txn_type = self.txn_type.ok_or_else(|| "txn_type is required".to_string())?;
        let quantity = self.quantity.ok_or_else(|| "quantity is required".to_string())?;
        let price_per_share = self.price_per_share.ok_or_else(|| "price_per_share is required".to_string())?;
        let amount = self.amount.ok_or_else(|| "amount is required".to_string())?;
        let txn_date = self.txn_date.ok_or_else(|| "txn_date is required".to_string())?;

        Ok(ShareTransaction {
            id: Uuid::new_v4(),
            company_id,
            share_class_id,
            shareholder_id,
            txn_type,
            quantity,
            price_per_share,
            amount,
            counterparty_shareholder_id: self.counterparty_shareholder_id,
            transfer_group_id: self.transfer_group_id,
            posting_reference: self.posting_reference,
            txn_date,
            gl_posted: self.gl_posted.unwrap_or(false),
            metadata: AuditMetadata::default(),
        })
    }
}
