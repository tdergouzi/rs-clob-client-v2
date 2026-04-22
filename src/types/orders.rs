use alloy_primitives::B256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::primitives::{OrderType, Side};

// ============================================================================
// Order Types & Parameters
// ============================================================================

/// V2 limit-order input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLimitOrder {
    #[serde(rename = "tokenID")]
    pub token_id: String,
    pub price: f64,
    pub size: f64,
    pub side: Side,

    /// Expiration (Unix seconds). Not covered by the EIP-712 signature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration: Option<u64>,

    /// Client timestamp (Unix ms). Defaults to `Date.now()` equivalent when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,

    /// Order metadata (`bytes32`), defaults to zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<B256>,

    /// Builder marker (`bytes32`), defaults to zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builder: Option<B256>,
}

/// V2 market-order input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMarketOrder {
    #[serde(rename = "tokenID")]
    pub token_id: String,

    /// When `None`, the client calculates the market price from the book.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,

    /// BUY: USDC amount. SELL: share amount.
    pub amount: f64,

    pub side: Side,

    /// FOK or FAK. Defaults to FOK when `None`.
    #[serde(rename = "orderType", skip_serializing_if = "Option::is_none")]
    pub order_type: Option<OrderType>,

    /// Client timestamp (Unix ms). Defaults to `Date.now()` equivalent when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,

    /// Order metadata (`bytes32`), defaults to zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<B256>,

    /// Builder marker (`bytes32`), defaults to zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builder: Option<B256>,
}

/// Order payload for cancellation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderPayload {
    pub order_id: String,
}

/// Order market cancel parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderMarketCancelParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
}

/// Arguments for posting multiple orders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostOrdersArgs {
    pub order: serde_json::Value,
    pub order_type: OrderType,
}

/// Open order information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenOrder {
    pub id: String,
    pub status: String,
    pub owner: String,
    pub maker_address: String,
    pub market: String,
    pub asset_id: String,
    pub side: String,
    pub original_size: String,
    pub size_matched: String,
    pub price: String,
    pub associate_trades: Vec<String>,
    pub outcome: String,
    pub created_at: u64,
    pub expiration: String,
    pub order_type: String,
}

/// Open orders response
pub type OpenOrdersResponse = Vec<OpenOrder>;

/// Open order parameters for filtering
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenOrderParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
}

/// Maker order information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakerOrder {
    pub order_id: String,
    pub owner: String,
    pub maker_address: String,
    pub matched_amount: String,
    pub price: String,
    pub fee_rate_bps: String,
    pub asset_id: String,
    pub outcome: String,
    pub side: Side,
}

// ============================================================================
// Order Scoring
// ============================================================================

/// Order scoring parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderScoringParams {
    pub order_id: String,
}

/// Order scoring response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderScoring {
    pub scoring: bool,
}

/// Orders scoring parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrdersScoringParams {
    pub order_ids: Vec<String>,
}

/// Orders scoring response
pub type OrdersScoring = HashMap<String, bool>;

