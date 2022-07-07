use std::{str::FromStr, sync::Arc};

use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use serde::{Deserialize, Serialize};
use tracing::error;
use trade_common::league::League;

use crate::store::{Offer, Store, StoreQuery};

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestBody {
    sell: Option<String>,
    buy: Option<String>,
    seller_account: Option<String>,
    stash_id: Option<String>,
    league: Option<String>,
    limit: Option<u32>,
}

#[tracing::instrument(skip(store))]
pub(crate) async fn handle_search(
    Json(payload): Json<RequestBody>,
    Extension(store): Extension<Arc<Store>>,
    // Extension(mut metrics): Extension<impl ApiMetrics>,
) -> Result<Json<QueryResponse>, QueryEmptyResultError> {
    // metrics.inc_search_requests();

    let league = {
        match &payload.league {
            None => Ok(League::Challenge),
            Some(str) => League::from_str(str).map_err(|_| QueryEmptyResultError {}),
        }
    }?;

    let query = StoreQuery {
        sell: payload.sell,
        buy: payload.buy,
        seller_account: payload.seller_account,
        stash_id: payload.stash_id,
        limit: payload.limit,
    };

    match store.query(league, query).await {
        Ok(offers) => Ok(Json(QueryResponse::new(offers))),
        Err(e) => {
            error!("{:?}", e);
            Err(QueryEmptyResultError::new())
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct QueryEmptyResultError {}

impl QueryEmptyResultError {
    pub fn new() -> Self {
        Self {}
    }
}

impl IntoResponse for QueryEmptyResultError {
    fn into_response(self) -> axum::response::Response {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct QueryResponse {
    count: usize,
    offers: Vec<Offer>,
}

impl QueryResponse {
    fn new(offers: Vec<Offer>) -> Self {
        Self {
            count: offers.len(),
            offers,
        }
    }
}
