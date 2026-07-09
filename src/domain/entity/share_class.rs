use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for ShareClass
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShareClassId(pub Uuid);

impl ShareClassId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ShareClassId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ShareClassId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ShareClassId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ShareClassId> for Uuid {
    fn from(id: ShareClassId) -> Self { id.0 }
}

impl AsRef<Uuid> for ShareClassId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ShareClassId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ShareClass {
    pub id: Uuid,
    pub company_id: Uuid,
    pub code: String,
    pub name: String,
    pub par_value: Decimal,
    pub currency: String,
    pub share_capital_account_id: Uuid,
    pub share_premium_account_id: Uuid,
    pub is_active: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl ShareClass {
    /// Create a builder for ShareClass
    pub fn builder() -> ShareClassBuilder {
        ShareClassBuilder::default()
    }

    /// Create a new ShareClass with required fields
    pub fn new(company_id: Uuid, code: String, name: String, par_value: Decimal, currency: String, share_capital_account_id: Uuid, share_premium_account_id: Uuid, is_active: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            code,
            name,
            par_value,
            currency,
            share_capital_account_id,
            share_premium_account_id,
            is_active,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> ShareClassId {
        ShareClassId(self.id)
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
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.code = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "par_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.par_value = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "share_capital_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.share_capital_account_id = v; }
                }
                "share_premium_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.share_premium_account_id = v; }
                }
                "is_active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_active = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for ShareClass {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "ShareClass"
    }
}

impl backbone_core::PersistentEntity for ShareClass {
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

impl backbone_orm::EntityRepoMeta for ShareClass {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("share_capital_account_id".to_string(), "uuid".to_string());
        m.insert("share_premium_account_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["code", "name", "currency"]
    }
}

/// Builder for ShareClass entity
///
/// Provides a fluent API for constructing ShareClass instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ShareClassBuilder {
    company_id: Option<Uuid>,
    code: Option<String>,
    name: Option<String>,
    par_value: Option<Decimal>,
    currency: Option<String>,
    share_capital_account_id: Option<Uuid>,
    share_premium_account_id: Option<Uuid>,
    is_active: Option<bool>,
}

impl ShareClassBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the code field (required)
    pub fn code(mut self, value: String) -> Self {
        self.code = Some(value);
        self
    }

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the par_value field (required)
    pub fn par_value(mut self, value: Decimal) -> Self {
        self.par_value = Some(value);
        self
    }

    /// Set the currency field (required)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the share_capital_account_id field (required)
    pub fn share_capital_account_id(mut self, value: Uuid) -> Self {
        self.share_capital_account_id = Some(value);
        self
    }

    /// Set the share_premium_account_id field (required)
    pub fn share_premium_account_id(mut self, value: Uuid) -> Self {
        self.share_premium_account_id = Some(value);
        self
    }

    /// Set the is_active field (default: `true`)
    pub fn is_active(mut self, value: bool) -> Self {
        self.is_active = Some(value);
        self
    }

    /// Build the ShareClass entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<ShareClass, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let code = self.code.ok_or_else(|| "code is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let par_value = self.par_value.ok_or_else(|| "par_value is required".to_string())?;
        let currency = self.currency.ok_or_else(|| "currency is required".to_string())?;
        let share_capital_account_id = self.share_capital_account_id.ok_or_else(|| "share_capital_account_id is required".to_string())?;
        let share_premium_account_id = self.share_premium_account_id.ok_or_else(|| "share_premium_account_id is required".to_string())?;

        Ok(ShareClass {
            id: Uuid::new_v4(),
            company_id,
            code,
            name,
            par_value,
            currency,
            share_capital_account_id,
            share_premium_account_id,
            is_active: self.is_active.unwrap_or(true),
            metadata: AuditMetadata::default(),
        })
    }
}
