use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "holder_type", rename_all = "snake_case")]
pub enum HolderType {
    Individual,
    Entity,
}

impl std::fmt::Display for HolderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Individual => write!(f, "individual"),
            Self::Entity => write!(f, "entity"),
        }
    }
}

impl FromStr for HolderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "individual" => Ok(Self::Individual),
            "entity" => Ok(Self::Entity),
            _ => Err(format!("Unknown HolderType variant: {}", s)),
        }
    }
}

impl Default for HolderType {
    fn default() -> Self {
        Self::Individual
    }
}
