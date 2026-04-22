mod common;

use common::create_test_client_with_wallet;
use rs_clob_client_v2::types::{OrderType, Side, TradeParams, UserLimitOrder, UserMarketOrder};

const TOKEN: &str = "112838095111461683880944516726938163688341306245473734071798778736646352193304";
const ORDER_ID: &str = "0xb2414d76eb52b85f0e756951532bf0c47eddc686b852484c28d8219401537f89";

const LIMIT_BUY_PRICE: f64 = 0.95;
const LIMIT_BUY_SIZE: f64 = 5.0;
const LIMIT_SELL_PRICE: f64 = 0.4;
const LIMIT_SELL_SIZE: f64 = 5.0;

#[tokio::test]
async fn test_create_market_buy_order() {
    let mut client = create_test_client_with_wallet();
    let creds = client
        .create_or_derive_api_key(None)
        .await
        .expect("Failed to create or derive API key");
    client.set_api_creds(creds);

    // Create and post a market buy order for the equivalent of 5 USDC for the market price
    let response = client
        .create_and_post_market_order(
            &UserMarketOrder {
                token_id: TOKEN.to_string(),
                amount: 5.0,
                side: Side::Buy,
                price: None,
                fee_rate_bps: None,
                nonce: None,
                taker: None,
                order_type: Some(OrderType::Fok), // or FAK
            },
            None,
            OrderType::Fok, // or FAK
        )
        .await
        .expect("Failed to create and post market order");

    // Assertions
    assert!(
        response.is_object(),
        "Create and post response should be a valid JSON object"
    );

    println!("Create and Post Response: {:#?}", response);
}

#[tokio::test]
async fn test_create_market_sell_order() {
    let mut client = create_test_client_with_wallet();
    let creds = client
        .create_or_derive_api_key(None)
        .await
        .expect("Failed to create or derive API key");
    client.set_api_creds(creds);

    // Create the order and send it to the server in a single step
    let response = client
        .create_and_post_market_order(
            &UserMarketOrder {
                token_id: TOKEN.to_string(),
                amount: LIMIT_SELL_SIZE, // SHARES
                side: Side::Sell,
                price: None,
                fee_rate_bps: None,
                nonce: None,
                taker: None,
                order_type: None,
            },
            None,
            OrderType::Fok, // or FAK
        )
        .await
        .expect("Failed to create and post market sell order");

    // Assertions
    assert!(
        response.is_object(),
        "Create and post response should be a valid JSON object"
    );

    println!("Create and Post Response: {:#?}", response);
}

#[tokio::test]
async fn test_create_limit_buy_order() {
    let mut client = create_test_client_with_wallet();
    let creds = client
        .create_or_derive_api_key(None)
        .await
        .expect("Failed to create or derive API key");
    client.set_api_creds(creds);

    // Create the order and send it to the server in a single step
    let response = client
        .create_and_post_limit_order(
            &UserLimitOrder {
                token_id: TOKEN.to_string(),
                price: LIMIT_BUY_PRICE,
                size: LIMIT_BUY_SIZE, // SHARES
                side: Side::Buy,
                fee_rate_bps: None,
                nonce: None,
                expiration: None,
                taker: None,
            },
            None,
            OrderType::Gtc,
        )
        .await
        .expect("Failed to create and post limit order");

    // Assertions
    assert!(
        response.is_object(),
        "Create and post response should be a valid JSON object"
    );

    println!("Create and Post Response: {:#?}", response); // 0xf58d1851dbd249d6d26f60f64f30a5cfa58e80950a4a24e14398348a91f6cbf6
}

#[tokio::test]
async fn test_create_limit_sell_order() {
    let mut client = create_test_client_with_wallet();
    let creds = client
        .create_or_derive_api_key(None)
        .await
        .expect("Failed to create or derive API key");
    client.set_api_creds(creds);

    // Create the order and send it to the server in a single step
    let response = client
        .create_and_post_limit_order(
            &UserLimitOrder {
                token_id: TOKEN.to_string(),
                price: LIMIT_SELL_PRICE,
                size: LIMIT_SELL_SIZE, // SHARES
                side: Side::Sell,
                fee_rate_bps: None,
                nonce: None,
                expiration: None,
                taker: None,
            },
            None,
            OrderType::Gtc,
        )
        .await
        .expect("Failed to create and post limit sell order");

    // Assertions
    assert!(
        response.is_object(),
        "Create and post response should be a valid JSON object"
    );

    println!("Create and Post Response: {:#?}", response);
}

#[tokio::test]
async fn test_get_trades() {
    let mut client = create_test_client_with_wallet();
    let creds = client
        .create_or_derive_api_key(None)
        .await
        .expect("Failed to create or derive API key");
    client.set_api_creds(creds);

    let params = Some(TradeParams {
        id: None,
        market: None,
        asset_id: None,
        maker_address: Some("0x1e6f40b325c8eb65a8ff62ff5261d51f362fdee0".to_string()),
        before: None,
        after: None,
    });

    // Get trades
    let trades = client
        .get_trades(params)
        .await
        .expect("Failed to fetch order");

    // Assertions
    assert!(!trades.len() > 0, "Trades should not be empty");

    println!("Trades: {:#?}", trades[0]);
}

#[tokio::test]
async fn test_get_open_order() {
    let mut client = create_test_client_with_wallet();
    let creds = client
        .create_or_derive_api_key(None)
        .await
        .expect("Failed to create or derive API key");
    client.set_api_creds(creds);

    let order_id = ORDER_ID; // Market sell order
    let order = client
        .get_open_order(order_id)
        .await
        .expect("Failed to fetch order");

    println!("{:#?}", order);
}

#[tokio::test]
async fn test_cancel_order() {
    let mut client = create_test_client_with_wallet();
    let creds = client
        .create_or_derive_api_key(None)
        .await
        .expect("Failed to create or derive API key");
    client.set_api_creds(creds);

    let order_id = "0xdc034d11f56d803cd5cceb71482d92cc6aa9abea4cbf8b6b47d65cb845a80f71"; // Limit sell order
    let response = client
        .cancel_order(order_id)
        .await
        .expect("Failed to cancel order");

    println!("{:#?}", response);
}
