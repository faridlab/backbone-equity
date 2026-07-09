use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::DividendStatus;
use super::AuditMetadata;

/// Strongly-typed ID for Dividend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DividendId(pub Uuid);

impl DividendId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for DividendId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for DividendId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for DividendId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<DividendId> for Uuid {
    fn from(id: DividendId) -> Self { id.0 }
}

impl AsRef<Uuid> for DividendId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for DividendId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Dividend {
    pub id: Uuid,
    pub company_id: Uuid,
    pub share_class_id: Uuid,
    pub declaration_date: NaiveDate,
    pub payment_date: Option<NaiveDate>,
    pub per_share_amount: Decimal,
    pub shares_outstanding: Decimal,
    pub total_amount: Decimal,
    pub status: DividendStatus,
    pub retained_earnings_account_id: Uuid,
    pub dividend_payable_account_id: Uuid,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Dividend {
    /// Create a builder for Dividend
    pub fn builder() -> DividendBuilder {
        DividendBuilder::default()
    }

    /// Create a new Dividend with required fields
    pub fn new(company_id: Uuid, share_class_id: Uuid, declaration_date: NaiveDate, per_share_amount: Decimal, shares_outstanding: Decimal, total_amount: Decimal, status: DividendStatus, retained_earnings_account_id: Uuid, dividend_payable_account_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            share_class_id,
            declaration_date,
            payment_date: None,
            per_share_amount,
            shares_outstanding,
            total_amount,
            status,
            retained_earnings_account_id,
            dividend_payable_account_id,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> DividendId {
        DividendId(self.id)
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

    /// Get the current status
    pub fn status(&self) -> &DividendStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the payment_date field (chainable)
    pub fn with_payment_date(mut self, value: NaiveDate) -> Self {
        self.payment_date = Some(value);
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
                "declaration_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.declaration_date = v; }
                }
                "payment_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.payment_date = v; }
                }
                "per_share_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.per_share_amount = v; }
                }
                "shares_outstanding" => {
                    if let Ok(v) = serde_json::from_value(value) { self.shares_outstanding = v; }
                }
                "total_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_amount = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "retained_earnings_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.retained_earnings_account_id = v; }
                }
                "dividend_payable_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.dividend_payable_account_id = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Dividend {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Dividend"
    }
}

impl backbone_core::PersistentEntity for Dividend {
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

impl backbone_orm::EntityRepoMeta for Dividend {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("share_class_id".to_string(), "uuid".to_string());
        m.insert("retained_earnings_account_id".to_string(), "uuid".to_string());
        m.insert("dividend_payable_account_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "dividend_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
}

/// Builder for Dividend entity
///
/// Provides a fluent API for constructing Dividend instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct DividendBuilder {
    company_id: Option<Uuid>,
    share_class_id: Option<Uuid>,
    declaration_date: Option<NaiveDate>,
    payment_date: Option<NaiveDate>,
    per_share_amount: Option<Decimal>,
    shares_outstanding: Option<Decimal>,
    total_amount: Option<Decimal>,
    status: Option<DividendStatus>,
    retained_earnings_account_id: Option<Uuid>,
    dividend_payable_account_id: Option<Uuid>,
}

impl DividendBuilder {
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

    /// Set the declaration_date field (required)
    pub fn declaration_date(mut self, value: NaiveDate) -> Self {
        self.declaration_date = Some(value);
        self
    }

    /// Set the payment_date field (optional)
    pub fn payment_date(mut self, value: NaiveDate) -> Self {
        self.payment_date = Some(value);
        self
    }

    /// Set the per_share_amount field (required)
    pub fn per_share_amount(mut self, value: Decimal) -> Self {
        self.per_share_amount = Some(value);
        self
    }

    /// Set the shares_outstanding field (required)
    pub fn shares_outstanding(mut self, value: Decimal) -> Self {
        self.shares_outstanding = Some(value);
        self
    }

    /// Set the total_amount field (required)
    pub fn total_amount(mut self, value: Decimal) -> Self {
        self.total_amount = Some(value);
        self
    }

    /// Set the status field (default: `DividendStatus::default()`)
    pub fn status(mut self, value: DividendStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the retained_earnings_account_id field (required)
    pub fn retained_earnings_account_id(mut self, value: Uuid) -> Self {
        self.retained_earnings_account_id = Some(value);
        self
    }

    /// Set the dividend_payable_account_id field (required)
    pub fn dividend_payable_account_id(mut self, value: Uuid) -> Self {
        self.dividend_payable_account_id = Some(value);
        self
    }

    /// Build the Dividend entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Dividend, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let share_class_id = self.share_class_id.ok_or_else(|| "share_class_id is required".to_string())?;
        let declaration_date = self.declaration_date.ok_or_else(|| "declaration_date is required".to_string())?;
        let per_share_amount = self.per_share_amount.ok_or_else(|| "per_share_amount is required".to_string())?;
        let shares_outstanding = self.shares_outstanding.ok_or_else(|| "shares_outstanding is required".to_string())?;
        let total_amount = self.total_amount.ok_or_else(|| "total_amount is required".to_string())?;
        let retained_earnings_account_id = self.retained_earnings_account_id.ok_or_else(|| "retained_earnings_account_id is required".to_string())?;
        let dividend_payable_account_id = self.dividend_payable_account_id.ok_or_else(|| "dividend_payable_account_id is required".to_string())?;

        Ok(Dividend {
            id: Uuid::new_v4(),
            company_id,
            share_class_id,
            declaration_date,
            payment_date: self.payment_date,
            per_share_amount,
            shares_outstanding,
            total_amount,
            status: self.status.unwrap_or(DividendStatus::default()),
            retained_earnings_account_id,
            dividend_payable_account_id,
            metadata: AuditMetadata::default(),
        })
    }
}
