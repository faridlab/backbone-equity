use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "dividend_status", rename_all = "snake_case")]
pub enum DividendStatus {
    Declared,
    Paid,
}

impl std::fmt::Display for DividendStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Declared => write!(f, "declared"),
            Self::Paid => write!(f, "paid"),
        }
    }
}

impl FromStr for DividendStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "declared" => Ok(Self::Declared),
            "paid" => Ok(Self::Paid),
            _ => Err(format!("Unknown DividendStatus variant: {}", s)),
        }
    }
}

impl Default for DividendStatus {
    fn default() -> Self {
        Self::Declared
    }
}
