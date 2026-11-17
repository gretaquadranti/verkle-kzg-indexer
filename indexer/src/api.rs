use axum::{Router, Json, extract::{Path, Query, State}, routing::get, http::StatusCode};
use std::sync::Arc;
use tokio::sync::Mutex;
use verkle_project::VerkleTree;
use crate::utils;
use crate::db;
use ark_ec::CurveGroup;
use ark_serialize::CanonicalSerialize;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub tree: Arc<Mutex<VerkleTree>>,
    pub db: Arc<PgPool>,
}

// converte un punto della curva in stringa esadecimale (48 byte compressi = 96 caratteri)
fn point_hex(p: &ark_bls12_381::G1Projective) -> String {
    let mut b = Vec::new();
    p.into_affine().serialize_compressed(&mut b).unwrap();
    hex::encode(b)
}

// ---------------------------------------------------------------
// ENDPOINT /query
// esempio: /query?fee_recipient=0xabc...&min_tx=5&limit=10
// tutti i parametri sono opzionali

#[derive(Deserialize)]
pub struct QueryParams {
    fee_recipient: Option<String>,
    from_ts: Option<i64>,
    to_ts: Option<i64>,
    min_tx: Option<i32>,
    limit: Option<i64>,
}

#[derive(Serialize)]
pub struct QueryItem {
    block_number: i64,
    block_hash: String,
    parent_hash: String,
    fee_recipient: String,
    timestamp: i64,
    gas_used: i64,
    gas_limit: i64,
    size: i64,
    tx_count: i32,
    digest: String,
}

#[derive(Serialize)]
pub struct QueryResponse {
    count: usize,
    results: Vec<QueryItem>,
}

pub async fn query_handler(
    Query(p): Query<QueryParams>,
    State(state): State<AppState>,
) -> Result<Json<QueryResponse>, (StatusCode, String)> {

    let fee_recipient = p.fee_recipient.map(|m| m.to_lowercase());
    let limit = p.limit.unwrap_or(20).clamp(1, 50);

    // il filtro lo esegue PostgreSQL
    let rows = db::query_blocks(
        &state.db,
        fee_recipient.as_deref(),
        p.from_ts,
        p.to_ts,
        p.min_tx,
        limit,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut results = Vec::new();
    for row in rows {
        let key = utils::block_number_to_key(row.number);
        let expected = utils::record_to_value(&row);
        let digest = hex::encode(&expected[..32]);

        results.push(QueryItem {
            block_number: row.number,
            block_hash: row.hash,
            parent_hash: row.parent_hash,
            fee_recipient: row.fee_recipient,
            timestamp: row.timestamp,
            gas_used: row.gas_used,
            gas_limit: row.gas_limit,
            size: row.size,
            tx_count: row.transactions_count,
            digest
        });
    }

    Ok(Json(QueryResponse {
        count: results.len(),
        results,
    }))
}

// ---------------------------------------------------------------
// ENDPOINT /proof/:n/json
// esporta la prova di appartenenza del blocco n (32 passi) per la verifica indipendente

#[derive(Serialize)]
pub struct StepJson {
    commitment: String,
    index: usize,
    value: String,
    witness: String,
}

#[derive(Serialize)]
pub struct ProofJson {
    block_number: i64,
    key: String,
    value: String,
    root: String,
    steps: Vec<StepJson>,
}

pub async fn get_proof_json(
    Path(n): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<ProofJson>, (StatusCode, String)> {
    let key = utils::block_number_to_key(n);
    let tree = state.tree.lock().await;

    let root = tree.get_root_commitment()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "tree vuoto".to_string()))?;
    let proof = tree.prove(&key)
        .ok_or((StatusCode::NOT_FOUND, "blocco non nel tree".to_string()))?;

    Ok(Json(ProofJson {
        block_number: n,
        key: hex::encode(key),
        value: hex::encode(proof.value),
        root: point_hex(&root.inner),
        steps: proof.steps.iter().map(|s| StepJson {
            commitment: point_hex(&s.commitment.inner),
            index: s.index,
            value: hex::encode(s.value),
            witness: point_hex(&s.witness),
        }).collect(),
    }))
}

// ---------------------------------------------------------------
// costruisce gli endpoint
pub fn build_router(tree: Arc<Mutex<VerkleTree>>, db: Arc<PgPool>) -> Router {

    let state = AppState { tree, db };

    Router::new()
        .route("/query", get(query_handler))
        .route("/proof/:n/json", get(get_proof_json))
        .with_state(state)
}