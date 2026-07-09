use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "share_txn_type", rename_all = "snake_case")]
pub enum ShareTxnType {
    Issue,
    TransferIn,
    TransferOut,
    Buyback,
}

impl std::fmt::Display for ShareTxnType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Issue => write!(f, "issue"),
            Self::TransferIn => write!(f, "transfer_in"),
            Self::TransferOut => write!(f, "transfer_out"),
            Self::Buyback => write!(f, "buyback"),
        }
    }
}

impl FromStr for ShareTxnType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "issue" => Ok(Self::Issue),
            "transfer_in" => Ok(Self::TransferIn),
            "transfer_out" => Ok(Self::TransferOut),
            "buyback" => Ok(Self::Buyback),
            _ => Err(format!("Unknown ShareTxnType variant: {}", s)),
        }
    }
}

impl Default for ShareTxnType {
    fn default() -> Self {
        Self::Issue
    }
}
