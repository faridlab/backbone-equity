// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// The hand-written equity SQL's parameter/projection types. Every repository listed here is declared
// `user_owned` in metaphor.codegen.yaml — see the write service, which orchestrates them.
pub use dividend_repository::{DividendBasisRow, DividendPaymentRow, NewDividendRow};
pub use share_class_repository::{NewShareClassRow, ShareClassDetailRow};
pub use shareholder_repository::NewShareholderRow;
pub use share_transaction_repository::{
    HoldingRow, NewBuybackTxnRow, NewIssueTxnRow, NewTransferLegRow,
};
