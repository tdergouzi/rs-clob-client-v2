use crate::client::ClobClient;
use crate::constants::{END_CURSOR, INITIAL_CURSOR};
use crate::endpoints::endpoints;
use crate::errors::{ClobError, ClobResult};
use crate::headers::create_l2_headers;
use crate::order_builder::{calculate_buy_market_price, calculate_sell_market_price};
use crate::types::*;
use rs_order_utils::v2::SignedOrder;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Upper bound on how long a matched order waits for its settlement hashes.
const RESOLVE_TRADES_TIMEOUT: Duration = Duration::from_secs(30);
/// Delay between two sweeps over the still-unresolved trades.
const RESOLVE_TRADES_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Treats both `""` and an all-zero hash as "not settled on-chain yet".
fn is_empty_hash(hash: &str) -> bool {
    let digits = hash.trim().trim_start_matches("0x");
    digits.is_empty() || digits.bytes().all(|b| b == b'0')
}

/// Reads the trade IDs of a post-order response that still needs its settlement
/// hashes backfilled. Empty when the server already supplied hashes or the order
/// did not match. The server has used several spellings for both fields.
fn pending_trade_ids(response: &serde_json::Value) -> Vec<String> {
    let has_hashes = [
        "transactionsHashes",
        "transactionHashes",
        "transaction_hashes",
    ]
    .iter()
    .find_map(|key| response.get(*key))
    .and_then(|value| value.as_array())
    .is_some_and(|hashes| !hashes.is_empty());
    if has_hashes {
        return Vec::new();
    }

    ["tradeIDs", "tradeIds", "trade_ids"]
        .iter()
        .find_map(|key| response.get(*key))
        .and_then(|value| value.as_array())
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Extracts the settlement hashes for the given trade IDs out of resolved
/// trades. Trades that failed execution never contribute a hash.
fn transaction_hashes_for(trade_ids: &[String], resolved_trades: &[Trade]) -> Vec<String> {
    trade_ids
        .iter()
        .filter_map(|id| resolved_trades.iter().find(|trade| trade.id == *id))
        .filter(|trade| !trade.status.eq_ignore_ascii_case("FAILED"))
        .map(|trade| trade.transaction_hash.clone())
        .filter(|hash| !is_empty_hash(hash))
        .collect()
}

/// Writes resolved settlement hashes onto a post-order response, under both
/// spellings so callers reading either key keep working.
fn set_transaction_hashes(response: &mut serde_json::Value, hashes: Vec<String>) {
    let Some(object) = response.as_object_mut() else {
        return;
    };
    let value = serde_json::Value::from(hashes);
    object.insert("transactionsHashes".to_string(), value.clone());
    object.insert("transactionHashes".to_string(), value);
}

impl ClobClient {
    // ===================================
    // L1 Auth Methods
    // ===================================

    /// Creates a signed limit order
    ///
    /// # Arguments
    ///
    /// * `user_order` - Order parameters (token_id, price, size, side, etc.)
    /// * `options` - Optional CreateOrderOptions (tick_size, neg_risk)
    ///
    /// # Returns
    ///
    /// A JSON representation of the signed order ready for posting
    pub async fn create_limit_order(
        &self,
        user_limit_order: &UserLimitOrder,
        options: Option<CreateOrderOptions>,
    ) -> ClobResult<serde_json::Value> {
        self.can_l1_auth()?;

        let token_id = &user_limit_order.token_id;

        let tick_size = if let Some(opts) = &options {
            opts.tick_size
        } else {
            self.get_tick_size(token_id).await?
        };

        let neg_risk = if let Some(opts) = &options {
            opts.neg_risk.unwrap_or(false)
        } else {
            self.get_neg_risk(token_id).await?
        };

        let create_options = CreateOrderOptions {
            tick_size,
            neg_risk: Some(neg_risk),
        };

        let order_builder = self
            .order_builder
            .as_ref()
            .ok_or(ClobError::L1AuthUnavailable)?;

        let signed_order = order_builder
            .build_limit_order(user_limit_order, &create_options)
            .await?;
        self.signed_order_to_json(signed_order)
    }

    /// Creates a signed market order
    ///
    /// # Arguments
    ///
    /// * `user_market_order` - Market order parameters (token_id, amount, side, etc.)
    /// * `options` - Optional CreateOrderOptions (tick_size, neg_risk)
    ///
    /// # Returns
    ///
    /// A JSON representation of the signed order ready for posting
    pub async fn create_market_order(
        &self,
        user_market_order: &UserMarketOrder,
        options: Option<CreateOrderOptions>,
    ) -> ClobResult<serde_json::Value> {
        self.can_l1_auth()?;

        let token_id = &user_market_order.token_id;

        let tick_size = if let Some(opts) = &options {
            opts.tick_size
        } else {
            self.get_tick_size(token_id).await?
        };

        let neg_risk = if let Some(opts) = &options {
            opts.neg_risk.unwrap_or(false)
        } else {
            self.get_neg_risk(token_id).await?
        };

        let create_options = CreateOrderOptions {
            tick_size,
            neg_risk: Some(neg_risk),
        };

        let mut order = user_market_order.clone();

        // Calculate market price if not provided
        if order.price.is_none() {
            let price = self
                .calculate_market_price(
                    token_id,
                    order.side,
                    order.amount,
                    order.order_type.unwrap_or(OrderType::Fok),
                )
                .await?;
            order.price = Some(price);
        }

        let order_builder = self
            .order_builder
            .as_ref()
            .ok_or(ClobError::L1AuthUnavailable)?;

        let signed_order = order_builder
            .build_market_order(&order, &create_options)
            .await?;
        self.signed_order_to_json(signed_order)
    }

    // ===================================
    // L2 Auth Methods
    // ===================================

    /// Creates and posts a limit order in one call
    ///
    /// # Arguments
    ///
    /// * `user_order` - Order parameters, the size is in shares both for buy and sell
    /// * `options` - Optional CreateOrderOptions
    /// * `order_type` - GTC, FOK, FAK, or GTD
    /// 
    ///
    /// # Returns
    ///
    /// API response with order status
    pub async fn create_and_post_limit_order(
        &self,
        user_limit_order: &UserLimitOrder,
        options: Option<CreateOrderOptions>,
        order_type: OrderType,
    ) -> ClobResult<serde_json::Value> {
        let order = self.create_limit_order(user_limit_order, options).await?;
        self.post_order(order, order_type).await
    }

    /// Creates and posts a market order in one call
    ///
    /// # Arguments
    ///
    /// * `user_market_order` - Market order parameters
    /// * `options` - Optional CreateOrderOptions
    /// * `order_type` - Typically FOK or FAK
    ///
    /// # Returns
    ///
    /// API response with order status
    pub async fn create_and_post_market_order(
        &self,
        user_market_order: &UserMarketOrder,
        options: Option<CreateOrderOptions>,
        order_type: OrderType,
    ) -> ClobResult<serde_json::Value> {
        let order = self.create_market_order(user_market_order, options).await?;
        self.post_order(order, order_type).await
    }

    /// Gets all trade history with automatic pagination
    /// Note: The trades history only includes trades that have been executed, does not include limit orders
    pub async fn get_trades(&self, params: Option<TradeParams>) -> ClobResult<Vec<Trade>> {
        self.can_l2_auth()?;

        let mut results = Vec::new();
        let mut next_cursor = INITIAL_CURSOR.to_string();

        while next_cursor != END_CURSOR {
            let response = self
                .get_trades_paginated(params.clone(), Some(next_cursor.clone()))
                .await?;
            next_cursor = response.next_cursor;
            results.extend(response.data);
        }

        Ok(results)
    }

    /// Gets trades with pagination support
    pub async fn get_trades_paginated(
        &self,
        params: Option<TradeParams>,
        cursor: Option<String>,
    ) -> ClobResult<TradesPaginatedResponse> {
        self.can_l2_auth()?;

        let wallet = self.wallet.as_ref().ok_or(ClobError::L1AuthUnavailable)?;
        let creds = self.creds.as_ref().ok_or(ClobError::L2AuthNotAvailable)?;

        let endpoint_path = endpoints::GET_TRADES;
        let timestamp = if self.use_server_time {
            Some(self.get_server_time().await?)
        } else {
            None
        };

        let headers = create_l2_headers(wallet, creds, "GET", endpoint_path, None, timestamp)
            .await?
            .to_headers();

        let mut query_params = HashMap::new();

        // Add cursor
        query_params.insert(
            "next_cursor".to_string(),
            cursor.unwrap_or_else(|| INITIAL_CURSOR.to_string()),
        );

        // Add user params
        if let Some(p) = params {
            if let Some(id) = p.id {
                query_params.insert("id".to_string(), id);
            }
            if let Some(market) = p.market {
                query_params.insert("market".to_string(), market);
            }
            if let Some(asset_id) = p.asset_id {
                query_params.insert("asset_id".to_string(), asset_id);
            }
            if let Some(maker) = p.maker_address {
                query_params.insert("maker_address".to_string(), maker);
            }
            if let Some(before) = p.before {
                query_params.insert("before".to_string(), before.to_string());
            }
            if let Some(after) = p.after {
                query_params.insert("after".to_string(), after.to_string());
            }
        }

        self.http_client
            .get(endpoint_path, Some(headers), Some(query_params))
            .await
    }

    /// Gets an open order by ID
    pub async fn get_open_order(&self, order_id: &str) -> ClobResult<OpenOrder> {
        self.can_l2_auth()?;

        let wallet = self.wallet.as_ref().ok_or(ClobError::L1AuthUnavailable)?;
        let creds = self.creds.as_ref().ok_or(ClobError::L2AuthNotAvailable)?;

        let endpoint_path = format!("{}{}", endpoints::GET_ORDER, order_id);
        let timestamp = if self.use_server_time {
            Some(self.get_server_time().await?)
        } else {
            None
        };

        let headers = create_l2_headers(wallet, creds, "GET", &endpoint_path, None, timestamp)
            .await?
            .to_headers();

        self.http_client
            .get(&endpoint_path, Some(headers), None)
            .await
    }

    /// Gets open orders for the user
    pub async fn get_open_orders(
        &self,
        params: Option<OpenOrderParams>,
    ) -> ClobResult<OpenOrdersResponse> {
        self.can_l2_auth()?;

        let wallet = self.wallet.as_ref().ok_or(ClobError::L1AuthUnavailable)?;
        let creds = self.creds.as_ref().ok_or(ClobError::L2AuthNotAvailable)?;

        let endpoint_path = endpoints::GET_OPEN_ORDERS;
        let timestamp = if self.use_server_time {
            Some(self.get_server_time().await?)
        } else {
            None
        };

        let headers = create_l2_headers(wallet, creds, "GET", endpoint_path, None, timestamp)
            .await?
            .to_headers();

        let mut query_params = HashMap::new();

        if let Some(p) = params {
            if let Some(id) = p.id {
                query_params.insert("id".to_string(), id);
            }
            if let Some(market) = p.market {
                query_params.insert("market".to_string(), market);
            }
            if let Some(asset_id) = p.asset_id {
                query_params.insert("asset_id".to_string(), asset_id);
            }
        }

        self.http_client
            .get(
                endpoint_path,
                Some(headers),
                (!query_params.is_empty()).then_some(query_params),
            )
            .await
    }

    /// Polls the given trades until every one reaches a terminal execution
    /// outcome (it carries a settlement transaction hash, or its status is
    /// FAILED and never will) or the polling window elapses. Best-effort:
    /// returns whatever resolved in time and never fails.
    async fn wait_for_resolved_trades(&self, trade_ids: &[String]) -> Vec<Trade> {
        let mut ids: Vec<&String> = Vec::new();
        for id in trade_ids {
            if !id.is_empty() && !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids.is_empty() {
            return Vec::new();
        }

        let mut resolved: Vec<Option<Trade>> = vec![None; ids.len()];
        let deadline = Instant::now() + RESOLVE_TRADES_TIMEOUT;

        loop {
            for (index, id) in ids.iter().enumerate() {
                if resolved[index].is_some() {
                    continue;
                }
                let params = TradeParams {
                    id: Some((*id).clone()),
                    ..Default::default()
                };
                let Ok(page) = self.get_trades_paginated(Some(params), None).await else {
                    continue;
                };
                resolved[index] = page.data.into_iter().find(|trade| {
                    trade.id == **id
                        && (trade.status.eq_ignore_ascii_case("FAILED")
                            || !is_empty_hash(&trade.transaction_hash))
                });
            }

            if resolved.iter().all(Option::is_some) || Instant::now() >= deadline {
                return resolved.into_iter().flatten().collect();
            }
            tokio::time::sleep(RESOLVE_TRADES_POLL_INTERVAL).await;
        }
    }

    /// Posts an order to the exchange
    ///
    /// When the order matches, the response carries the settlement transaction
    /// hashes of its fills under `transactionsHashes` / `transactionHashes`,
    /// resolved on a best-effort basis: the CLOB executes matches
    /// asynchronously and answers with `tradeIDs`, so the client polls those
    /// trades for up to 30s. If a hash never arrives the field is left out and
    /// the fill can still be followed through `tradeIDs`.
    pub async fn post_order(
        &self,
        order: serde_json::Value,
        order_type: OrderType,
    ) -> ClobResult<serde_json::Value> {
        self.can_l2_auth()?;

        let wallet = self.wallet.as_ref().ok_or(ClobError::L1AuthUnavailable)?;
        let creds = self.creds.as_ref().ok_or(ClobError::L2AuthNotAvailable)?;

        // Prepare order payload
        let order_payload = self.order_to_json(order, order_type)?;
        let body = serde_json::to_string(&order_payload)?;

        // Create L2 headers with body
        let endpoint_path = endpoints::POST_ORDER;
        let timestamp = if self.use_server_time {
            Some(self.get_server_time().await?)
        } else {
            None
        };

        let headers =
            create_l2_headers(wallet, creds, "POST", endpoint_path, Some(&body), timestamp).await?;

        // Inject builder headers if available
        let final_headers = if self.can_builder_auth() {
            match self
                ._generate_builder_headers(headers.clone(), "POST", endpoint_path, Some(&body))
                .await?
            {
                Some(builder_headers) => builder_headers.to_headers(),
                None => headers.to_headers(),
            }
        } else {
            headers.to_headers()
        };

        // Make request
        let mut response: serde_json::Value = self
            .http_client
            .post(
                endpoint_path,
                Some(final_headers),
                Some(order_payload),
                None,
            )
            .await?;

        let trade_ids = pending_trade_ids(&response);
        if !trade_ids.is_empty() {
            let resolved_trades = self.wait_for_resolved_trades(&trade_ids).await;
            let hashes = transaction_hashes_for(&trade_ids, &resolved_trades);
            if !hashes.is_empty() {
                set_transaction_hashes(&mut response, hashes);
            }
        }

        Ok(response)
    }

    /// Posts multiple orders to the exchange
    pub async fn post_orders(&self, orders: Vec<PostOrdersArgs>) -> ClobResult<serde_json::Value> {
        self.can_l2_auth()?;

        let wallet = self.wallet.as_ref().ok_or(ClobError::L1AuthUnavailable)?;
        let creds = self.creds.as_ref().ok_or(ClobError::L2AuthNotAvailable)?;

        // Convert each order to payload format
        let owner = &creds.key;
        let payloads: Vec<_> = orders
            .iter()
            .map(|arg| {
                serde_json::json!({
                    "order": arg.order,
                    "owner": owner,
                    "orderType": arg.order_type,
                    "deferExec": false
                })
            })
            .collect();

        let body = serde_json::to_string(&payloads)?;

        let endpoint_path = endpoints::POST_ORDERS;
        let timestamp = if self.use_server_time {
            Some(self.get_server_time().await?)
        } else {
            None
        };

        let headers =
            create_l2_headers(wallet, creds, "POST", endpoint_path, Some(&body), timestamp).await?;

        // Inject builder headers if available
        let final_headers = if self.can_builder_auth() {
            match self
                ._generate_builder_headers(headers.clone(), "POST", endpoint_path, Some(&body))
                .await?
            {
                Some(builder_headers) => builder_headers.to_headers(),
                None => headers.to_headers(),
            }
        } else {
            headers.to_headers()
        };

        let mut responses: serde_json::Value = self
            .http_client
            .post(endpoint_path, Some(final_headers), Some(payloads), None)
            .await?;

        // Resolve the whole batch against a single polling window, so a degraded
        // pipeline delays the batch by at most one timeout rather than one
        // timeout per matched order.
        let pending: Vec<Vec<String>> = responses
            .as_array()
            .map(|entries| entries.iter().map(pending_trade_ids).collect())
            .unwrap_or_default();
        let all_trade_ids: Vec<String> = pending.concat();
        if all_trade_ids.is_empty() {
            return Ok(responses);
        }

        let resolved_trades = self.wait_for_resolved_trades(&all_trade_ids).await;
        if let Some(entries) = responses.as_array_mut() {
            for (entry, trade_ids) in entries.iter_mut().zip(&pending) {
                let hashes = transaction_hashes_for(trade_ids, &resolved_trades);
                if !hashes.is_empty() {
                    set_transaction_hashes(entry, hashes);
                }
            }
        }

        Ok(responses)
    }

    /// Cancels a single order by ID
    pub async fn cancel_order(&self, order_id: &str) -> ClobResult<serde_json::Value> {
        self.can_l2_auth()?;

        let wallet = self.wallet.as_ref().ok_or(ClobError::L1AuthUnavailable)?;
        let creds = self.creds.as_ref().ok_or(ClobError::L2AuthNotAvailable)?;

        let payload = OrderPayload {
            order_id: order_id.to_string(),
        };
        let body = serde_json::to_string(&payload)?;

        let endpoint_path = endpoints::CANCEL_ORDER;
        let timestamp = if self.use_server_time {
            Some(self.get_server_time().await?)
        } else {
            None
        };

        let headers = create_l2_headers(
            wallet,
            creds,
            "DELETE",
            endpoint_path,
            Some(&body),
            timestamp,
        )
        .await?
        .to_headers();

        self.http_client
            .delete(endpoint_path, Some(headers), Some(payload), None)
            .await
    }

    /// Cancels multiple orders by IDs
    pub async fn cancel_orders(&self, order_ids: Vec<String>) -> ClobResult<serde_json::Value> {
        self.can_l2_auth()?;

        let wallet = self.wallet.as_ref().ok_or(ClobError::L1AuthUnavailable)?;
        let creds = self.creds.as_ref().ok_or(ClobError::L2AuthNotAvailable)?;

        // Polymarket DELETE /orders expects a bare JSON array of order IDs
        // (`["0x...", ...]`), not an object. Sign and send the array directly.
        let body = serde_json::to_string(&order_ids)?;

        let endpoint_path = endpoints::CANCEL_ORDERS;
        let timestamp = if self.use_server_time {
            Some(self.get_server_time().await?)
        } else {
            None
        };

        let headers = create_l2_headers(
            wallet,
            creds,
            "DELETE",
            endpoint_path,
            Some(&body),
            timestamp,
        )
        .await?
        .to_headers();

        self.http_client
            .delete(endpoint_path, Some(headers), Some(order_ids), None)
            .await
    }

    /// Cancels all open orders
    pub async fn cancel_all(&self) -> ClobResult<serde_json::Value> {
        self.can_l2_auth()?;

        let wallet = self.wallet.as_ref().ok_or(ClobError::L1AuthUnavailable)?;
        let creds = self.creds.as_ref().ok_or(ClobError::L2AuthNotAvailable)?;

        let endpoint_path = endpoints::CANCEL_ALL;
        let timestamp = if self.use_server_time {
            Some(self.get_server_time().await?)
        } else {
            None
        };

        let headers = create_l2_headers(wallet, creds, "DELETE", endpoint_path, None, timestamp)
            .await?
            .to_headers();

        self.http_client
            .delete(endpoint_path, Some(headers), None::<()>, None)
            .await
    }

    /// Cancels orders for a specific market or asset
    pub async fn cancel_market_orders(
        &self,
        params: OrderMarketCancelParams,
    ) -> ClobResult<serde_json::Value> {
        self.can_l2_auth()?;

        let wallet = self.wallet.as_ref().ok_or(ClobError::L1AuthUnavailable)?;
        let creds = self.creds.as_ref().ok_or(ClobError::L2AuthNotAvailable)?;

        let body = serde_json::to_string(&params)?;

        let endpoint_path = endpoints::CANCEL_MARKET_ORDERS;
        let timestamp = if self.use_server_time {
            Some(self.get_server_time().await?)
        } else {
            None
        };

        let headers = create_l2_headers(
            wallet,
            creds,
            "DELETE",
            endpoint_path,
            Some(&body),
            timestamp,
        )
        .await?
        .to_headers();

        self.http_client
            .delete(endpoint_path, Some(headers), Some(params), None)
            .await
    }

    // ===================================
    // Builder Auth Methods (Trades)
    // ===================================

    /// Gets builder trades with pagination
    pub async fn get_builder_trades(
        &self,
        params: Option<TradeParams>,
        cursor: Option<String>,
    ) -> ClobResult<BuilderTradesResponse> {
        self.must_builder_auth()?;

        let endpoint_path = endpoints::GET_BUILDER_TRADES;

        // Get builder headers (already a HashMap)
        let headers = self
            ._get_builder_headers("GET", endpoint_path, None)
            .await?;

        let mut query_params = HashMap::new();

        // Add cursor
        query_params.insert(
            "next_cursor".to_string(),
            cursor.unwrap_or_else(|| INITIAL_CURSOR.to_string()),
        );

        // Add user params
        if let Some(p) = params {
            if let Some(id) = p.id {
                query_params.insert("id".to_string(), id);
            }
            if let Some(market) = p.market {
                query_params.insert("market".to_string(), market);
            }
            if let Some(asset_id) = p.asset_id {
                query_params.insert("asset_id".to_string(), asset_id);
            }
        }

        self.http_client
            .get(endpoint_path, Some(headers), Some(query_params))
            .await
    }

    // ===================================
    // Public Method (Market Price Calculation)
    // ===================================

    /// Calculates market execution price from orderbook
    ///
    /// # Arguments
    ///
    /// * `token_id` - Token ID to calculate price for
    /// * `side` - Buy or Sell
    /// * `amount` - Amount in USDC (for Buy) or tokens (for Sell)
    /// * `order_type` - FOK or FAK
    ///
    /// # Returns
    ///
    /// Calculated execution price with buffer
    pub async fn calculate_market_price(
        &self,
        token_id: &str,
        side: Side,
        amount: f64,
        order_type: OrderType,
    ) -> ClobResult<f64> {
        let orderbook = self.get_order_book(token_id).await?;
        match side {
            Side::Buy => {
                if orderbook.asks.is_empty() {
                    return Err(ClobError::NoMatch);
                }
                calculate_buy_market_price(&orderbook.asks, amount, order_type)
            }
            Side::Sell => {
                if orderbook.bids.is_empty() {
                    return Err(ClobError::NoMatch);
                }
                calculate_sell_market_price(&orderbook.bids, amount, order_type)
            }
        }
    }

    // ===================================
    // Private Helper Methods
    // ===================================

    /// Converts order to JSON payload for API submission
    fn order_to_json(
        &self,
        order: serde_json::Value,
        order_type: OrderType,
    ) -> ClobResult<serde_json::Value> {
        let owner = self
            .creds
            .as_ref()
            .ok_or(ClobError::L2AuthNotAvailable)?
            .key
            .clone();

        // Wrap the order in the expected payload format
        Ok(serde_json::json!({
            "order": order,
            "owner": owner,
            "orderType": order_type,
        }))
    }

    fn signed_order_to_json(&self, signed_order: SignedOrder) -> ClobResult<serde_json::Value> {
        serde_json::to_value(&signed_order).map_err(ClobError::JsonError)
    }

    // Pre-migration orders (L2 Authentication)

    /// Auto-paginates all pre-migration (V1) orders for the authenticated user.
    pub async fn get_pre_migration_orders(&self) -> ClobResult<Vec<OpenOrder>> {
        self.can_l2_auth()?;

        let mut results = Vec::new();
        let mut next_cursor = INITIAL_CURSOR.to_string();

        while next_cursor != END_CURSOR {
            let page = self
                .get_pre_migration_orders_paginated(Some(next_cursor))
                .await?;
            results.extend(page.data);
            next_cursor = page.next_cursor;
        }

        Ok(results)
    }

    /// Fetches a single page of pre-migration orders starting at `cursor`.
    pub async fn get_pre_migration_orders_paginated(
        &self,
        cursor: Option<String>,
    ) -> ClobResult<PreMigrationOrdersPaginatedResponse> {
        self.can_l2_auth()?;

        let wallet = self.wallet.as_ref().ok_or(ClobError::L1AuthUnavailable)?;
        let creds = self.creds.as_ref().ok_or(ClobError::L2AuthNotAvailable)?;

        let endpoint_path = endpoints::GET_PRE_MIGRATION_ORDERS;
        let timestamp = if self.use_server_time {
            Some(self.get_server_time().await?)
        } else {
            None
        };

        let headers = create_l2_headers(wallet, creds, "GET", endpoint_path, None, timestamp)
            .await?
            .to_headers();

        let mut query_params = HashMap::new();
        query_params.insert(
            "next_cursor".to_string(),
            cursor.unwrap_or_else(|| INITIAL_CURSOR.to_string()),
        );

        self.http_client
            .get(endpoint_path, Some(headers), Some(query_params))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn trade(id: &str, status: &str, transaction_hash: &str) -> Trade {
        serde_json::from_value(json!({
            "id": id,
            "taker_order_id": "taker_1",
            "market": "0xmarket",
            "asset_id": "1234",
            "side": "BUY",
            "size": "12.5",
            "fee_rate_bps": "5",
            "price": "0.42",
            "status": status,
            "match_time": "1705322096",
            "last_update": "1705322130",
            "outcome": "YES",
            "bucket_index": 2,
            "owner": "owner-1",
            "maker_address": "0x2222222222222222222222222222222222222222",
            "transaction_hash": transaction_hash,
            "trader_side": "TAKER"
        }))
        .unwrap()
    }

    #[test]
    fn test_is_empty_hash() {
        assert!(is_empty_hash(""));
        assert!(is_empty_hash("0x"));
        assert!(is_empty_hash(
            "0x0000000000000000000000000000000000000000000000000000000000000000"
        ));
        assert!(!is_empty_hash("0x2369f69a"));
    }

    #[test]
    fn test_pending_trade_ids() {
        // Async pipeline: trade IDs only, hashes to be resolved.
        assert_eq!(
            pending_trade_ids(&json!({ "tradeIDs": ["trade-1", "trade-2"] })),
            vec!["trade-1", "trade-2"]
        );
        // Server already supplied hashes: nothing to resolve.
        assert!(pending_trade_ids(&json!({
            "tradeIDs": ["trade-1"],
            "transactionsHashes": ["0xabc"]
        }))
        .is_empty());
        // Unmatched order.
        assert!(pending_trade_ids(&json!({ "status": "live" })).is_empty());
    }

    #[test]
    fn test_transaction_hashes_for() {
        let ids = vec!["trade-1".to_string(), "trade-2".to_string()];
        let resolved = vec![
            trade("trade-1", "FAILED", ""),
            trade("trade-2", "MINED", "0xabc"),
        ];
        // FAILED trades and trades still awaiting broadcast contribute nothing.
        assert_eq!(transaction_hashes_for(&ids, &resolved), vec!["0xabc"]);
    }

    #[test]
    fn test_set_transaction_hashes() {
        let mut response = json!({ "orderID": "0x1", "tradeIDs": ["trade-1"] });
        set_transaction_hashes(&mut response, vec!["0xabc".to_string()]);
        assert_eq!(response["transactionsHashes"], json!(["0xabc"]));
        assert_eq!(response["transactionHashes"], json!(["0xabc"]));
    }

    #[test]
    fn test_trade_deserializes_without_transaction_hash() {
        let pending = trade("trade-1", "MATCHED", "");
        assert!(is_empty_hash(&pending.transaction_hash));
        assert!(pending.maker_orders.is_empty());
    }
}
