//! Thin lightwalletd proxy for Zplash.
//! Translates REST calls to gRPC. Never sees key material.

mod proto {
    tonic::include_proto!("cash.z.wallet.sdk.rpc");
}

use axum::{
    extract::Query,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use proto::compact_tx_streamer_client::CompactTxStreamerClient;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tonic::transport::{Channel, ClientTlsConfig};
use tower_http::cors::CorsLayer;

const LWD_URL: &str = "https://zec.rocks:443";
const PARAMS_BASE_URL: &str = "https://download.z.cash/downloads";

async fn connect() -> Result<CompactTxStreamerClient<Channel>, String> {
    let tls = ClientTlsConfig::new().with_webpki_roots();
    let ch = Channel::from_shared(LWD_URL.to_string())
        .map_err(|e| e.to_string())?
        .tls_config(tls)
        .map_err(|e| e.to_string())?
        .connect()
        .await
        .map_err(|e| e.to_string())?;
    Ok(CompactTxStreamerClient::new(ch))
}

// GET /api/latest
async fn latest_block() -> Result<Json<serde_json::Value>, AppErr> {
    let mut c = connect().await?;
    let resp = c.get_latest_block(proto::ChainSpec {}).await.map_err(AppErr::from)?.into_inner();
    Ok(Json(serde_json::json!({ "height": resp.height })))
}

// GET /api/tree?height=123
#[derive(Deserialize)]
struct HeightParam { height: u64 }

async fn tree_state(Query(p): Query<HeightParam>) -> Result<Json<serde_json::Value>, AppErr> {
    let mut c = connect().await?;
    let resp = c
        .get_tree_state(proto::BlockId { height: p.height, hash: vec![] })
        .await.map_err(AppErr::from)?
        .into_inner();
    Ok(Json(serde_json::json!({
        "height": resp.height,
        "saplingTree": resp.sapling_tree,
    })))
}

// GET /api/blocks?start=100&end=200
#[derive(Deserialize)]
struct RangeParam { start: u64, end: u64 }

#[derive(Serialize)]
struct CompactOutputJson {
    cmu: String,
    epk: String,
    ciphertext: String,
}

#[derive(Serialize)]
struct CompactSpendJson {
    nf: String,
}

#[derive(Serialize)]
struct CompactTxJson {
    spends: Vec<CompactSpendJson>,
    outputs: Vec<CompactOutputJson>,
}

#[derive(Serialize)]
struct CompactBlockJson {
    height: u64,
    vtx: Vec<CompactTxJson>,
}

async fn block_range(Query(p): Query<RangeParam>) -> Result<Json<Vec<CompactBlockJson>>, AppErr> {
    let mut c = connect().await?;
    let range = proto::BlockRange {
        start: Some(proto::BlockId { height: p.start, hash: vec![] }),
        end: Some(proto::BlockId { height: p.end, hash: vec![] }),
    };
    let mut stream = c.get_block_range(range).await.map_err(AppErr::from)?.into_inner();
    let mut blocks = Vec::new();
    while let Some(block) = stream.message().await.map_err(AppErr::from)? {
        let vtx = block
            .vtx
            .into_iter()
            .map(|tx| CompactTxJson {
                spends: tx
                    .spends
                    .into_iter()
                    .map(|s| CompactSpendJson {
                        nf: hex::encode(&s.nf),
                    })
                    .collect(),
                outputs: tx
                    .outputs
                    .into_iter()
                    .map(|o| CompactOutputJson {
                        cmu: hex::encode(&o.cmu),
                        epk: hex::encode(&o.epk),
                        ciphertext: hex::encode(&o.ciphertext),
                    })
                    .collect(),
            })
            .collect();
        blocks.push(CompactBlockJson { height: block.height, vtx });
    }
    Ok(Json(blocks))
}

async fn sapling_params(filename: &str) -> Result<impl IntoResponse, AppErr> {
    let url = format!("{}/{}", PARAMS_BASE_URL, filename);
    let response = reqwest::get(&url).await.map_err(AppErr::from)?;
    let status = response.status();
    if !status.is_success() {
        return Err(AppErr(format!("Failed to fetch {}: {}", filename, status)));
    }

    let bytes = response.bytes().await.map_err(AppErr::from)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CACHE_CONTROL, "public, max-age=86400, immutable"),
        ],
        bytes,
    ))
}

async fn spend_params() -> Result<impl IntoResponse, AppErr> {
    sapling_params("sapling-spend.params").await
}

async fn output_params() -> Result<impl IntoResponse, AppErr> {
    sapling_params("sapling-output.params").await
}

// POST /api/broadcast  body: { "txHex": "..." }
#[derive(Deserialize)]
struct BroadcastReq { #[serde(rename = "txHex")] tx_hex: String }

async fn broadcast(Json(req): Json<BroadcastReq>) -> Result<Json<serde_json::Value>, AppErr> {
    let data = hex::decode(&req.tx_hex).map_err(|e| AppErr(e.to_string()))?;
    let mut c = connect().await?;
    let resp = c
        .send_transaction(proto::RawTransaction { data, height: 0 })
        .await.map_err(AppErr::from)?
        .into_inner();
    if resp.error_code != 0 {
        return Err(AppErr(format!("Rejected: {} (code {})", resp.error_message, resp.error_code)));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

// Error type
struct AppErr(String);
impl<E: std::fmt::Display> From<E> for AppErr {
    fn from(e: E) -> Self { AppErr(e.to_string()) }
}
impl IntoResponse for AppErr {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": self.0 }))).into_response()
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/api/latest", get(latest_block))
        .route("/api/tree", get(tree_state))
        .route("/api/blocks", get(block_range))
        .route("/api/params/spend", get(spend_params))
        .route("/api/params/output", get(output_params))
        .route("/api/broadcast", axum::routing::post(broadcast))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    eprintln!("Proxy listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
