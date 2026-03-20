//! Sweep backend for Zplash (ZIP-324)
//!
//! Scans the blockchain for notes sent to an ephemeral payment address,
//! builds a Sapling sweep transaction, and broadcasts it via lightwalletd.

mod proto {
    tonic::include_proto!("cash.z.wallet.sdk.rpc");
}

use anyhow::{anyhow, Context, Result};
use axum::{http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use blake2b_simd::Params as Blake2bParams;
use incrementalmerkletree::{frontier::CommitmentTree, witness::IncrementalWitness};
use proto::compact_tx_streamer_client::CompactTxStreamerClient;
use sapling_crypto::{
    keys::PreparedIncomingViewingKey,
    note::ExtractedNoteCommitment,
    note_encryption::{CompactOutputDescription, SaplingDomain, Zip212Enforcement},
    zip32::ExtendedSpendingKey,
    Anchor, MerklePath, Node, Note, PaymentAddress,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tonic::transport::{Channel, ClientTlsConfig};
use tower_http::cors::CorsLayer;
use zcash_note_encryption::{try_compact_note_decryption, EphemeralKeyBytes};
use zcash_primitives::{
    consensus::{BlockHeight, MainNetwork, Parameters, TestNetwork},
    merkle_tree::read_commitment_tree,
    transaction::builder::{BuildConfig, Builder},
};
use zcash_proofs::prover::LocalTxProver;
use zcash_protocol::{memo::MemoBytes, value::Zatoshis};
use zcash_transparent::builder::TransparentSigningSet;
use zip32::Scope;

// ─── Constants ───

const MAINNET_LWD: &str = "https://zec.rocks:443";
const TESTNET_LWD: &str = "https://testnet.lightwalletd.com:9067";
const SCAN_BLOCKS: u64 = 500;
const FEE_ZATOSHI: u64 = 10_000;

// ─── API types ───

#[derive(Deserialize)]
struct SweepRequest {
    key: String,
    recipient: String,
    #[serde(default)]
    testnet: bool,
}

#[derive(Serialize)]
struct SweepResponse {
    txid: String,
    amount_zatoshi: u64,
    fee_zatoshi: u64,
}

// ─── Key derivation (matches the frontend) ───

fn decode_key(bech32_key: &str) -> Result<[u8; 32]> {
    let (hrp, data) = bech32::decode(bech32_key).context("Invalid bech32 key")?;
    if hrp.as_str() != "zkey" {
        return Err(anyhow!("Expected HRP 'zkey', got '{}'", hrp));
    }
    let bytes: Vec<u8> = data.iter().copied().collect();
    bytes.try_into().map_err(|_| anyhow!("Key must be 32 bytes"))
}

fn derive_spending_key(key: &[u8; 32]) -> [u8; 32] {
    let hash = Blake2bParams::new()
        .hash_length(32)
        .personal(b"Zcash_PaymentURI")
        .hash(key);
    let mut sk = [0u8; 32];
    sk.copy_from_slice(hash.as_bytes());
    sk
}

fn decode_sapling_address(addr: &str) -> Result<PaymentAddress> {
    let (hrp, data) = bech32::decode(addr).context("Invalid recipient address")?;
    let h = hrp.as_str();
    if h != "zs" && h != "ztestsapling" {
        return Err(anyhow!("Expected Sapling address (zs1... or ztestsapling1...)"));
    }
    let bytes: Vec<u8> = data.iter().copied().collect();
    let arr: [u8; 43] = bytes.try_into().map_err(|_| anyhow!("Address must be 43 bytes"))?;
    PaymentAddress::from_bytes(&arr).ok_or_else(|| anyhow!("Invalid Sapling address"))
}

// ─── Lightwalletd gRPC client ───

async fn connect_lwd(testnet: bool) -> Result<CompactTxStreamerClient<Channel>> {
    let url = if testnet { TESTNET_LWD } else { MAINNET_LWD };
    tracing::info!("Connecting to {}", url);
    let tls = ClientTlsConfig::new().with_webpki_roots();
    let channel = Channel::from_shared(url.to_string())?
        .tls_config(tls)?
        .connect()
        .await
        .context("Failed to connect to lightwalletd")?;
    Ok(CompactTxStreamerClient::new(channel))
}

async fn get_latest_height(c: &mut CompactTxStreamerClient<Channel>) -> Result<u64> {
    Ok(c.get_latest_block(proto::ChainSpec {}).await?.into_inner().height)
}

async fn get_tree_state(c: &mut CompactTxStreamerClient<Channel>, h: u64) -> Result<proto::TreeState> {
    Ok(c.get_tree_state(proto::BlockId { height: h, hash: vec![] }).await?.into_inner())
}

async fn get_block_range(
    c: &mut CompactTxStreamerClient<Channel>,
    start: u64,
    end: u64,
) -> Result<Vec<proto::CompactBlock>> {
    let range = proto::BlockRange {
        start: Some(proto::BlockId { height: start, hash: vec![] }),
        end: Some(proto::BlockId { height: end, hash: vec![] }),
    };
    let mut stream = c.get_block_range(range).await?.into_inner();
    let mut blocks = Vec::new();
    while let Some(block) = stream.message().await? {
        blocks.push(block);
    }
    Ok(blocks)
}

async fn broadcast_tx(c: &mut CompactTxStreamerClient<Channel>, data: Vec<u8>) -> Result<()> {
    let resp = c
        .send_transaction(proto::RawTransaction { data, height: 0 })
        .await?
        .into_inner();
    if resp.error_code != 0 {
        return Err(anyhow!("Rejected: {} (code {})", resp.error_message, resp.error_code));
    }
    Ok(())
}

// ─── Block scanning ───

struct FoundNote {
    note: Note,
    witness: IncrementalWitness<Node, 32>,
}

fn scan_blocks_for_notes(
    blocks: &[proto::CompactBlock],
    initial_tree: &CommitmentTree<Node, 32>,
    ivk: &PreparedIncomingViewingKey,
) -> Result<Vec<FoundNote>> {
    let mut tree = initial_tree.clone();
    let mut found: Vec<FoundNote> = Vec::new();
    let mut witnesses: Vec<IncrementalWitness<Node, 32>> = Vec::new();

    // Post-Canopy mainnet always enforces ZIP 212
    let domain = SaplingDomain::new(Zip212Enforcement::On);

    for block in blocks {
        for tx in &block.vtx {
            for output in &tx.outputs {
                if output.cmu.len() != 32 || output.epk.len() != 32 || output.ciphertext.len() < 52
                {
                    continue;
                }

                let cmu_bytes: [u8; 32] = output.cmu[..].try_into().unwrap();
                let cmu = match Option::from(ExtractedNoteCommitment::from_bytes(&cmu_bytes)) {
                    Some(c) => c,
                    None => continue,
                };

                let node = Node::from_cmu(&cmu);
                tree.append(node).map_err(|_| anyhow!("Tree full"))?;
                for w in &mut witnesses {
                    w.append(node).map_err(|_| anyhow!("Witness update failed"))?;
                }

                // Trial decryption
                let epk: [u8; 32] = output.epk[..].try_into().unwrap();
                let mut ct = [0u8; 52];
                ct.copy_from_slice(&output.ciphertext[..52]);

                let compact = CompactOutputDescription {
                    ephemeral_key: EphemeralKeyBytes(epk),
                    cmu,
                    enc_ciphertext: ct,
                };

                if let Some((note, _addr)) = try_compact_note_decryption(&domain, ivk, &compact) {
                    tracing::info!(
                        "Found note at height {}, value = {} zatoshi",
                        block.height,
                        note.value().inner()
                    );
                    let w = IncrementalWitness::from_tree(tree.clone())
                        .expect("tree should be non-empty after append");
                    witnesses.push(w.clone());
                    found.push(FoundNote { note, witness: w });
                }
            }
        }
    }

    // Pair found notes with their updated witnesses
    Ok(found
        .into_iter()
        .zip(witnesses)
        .map(|(mut f, w)| {
            f.witness = w;
            f
        })
        .collect())
}

// ─── Sweep transaction ───

fn build_sweep<P: Parameters + Copy>(
    params: P,
    extsk: &ExtendedSpendingKey,
    notes: Vec<FoundNote>,
    recipient: &PaymentAddress,
    anchor_height: BlockHeight,
    anchor: Anchor,
    prover: &LocalTxProver,
) -> Result<(String, Vec<u8>, u64)> {
    if notes.is_empty() {
        return Err(anyhow!("No notes found — address hasn't been funded yet"));
    }

    let total: u64 = notes.iter().map(|n| n.note.value().inner()).sum();
    if total <= FEE_ZATOSHI {
        return Err(anyhow!(
            "Balance ({} zatoshi) too small to cover fee ({})",
            total,
            FEE_ZATOSHI
        ));
    }
    let send_amount = total - FEE_ZATOSHI;
    tracing::info!("Sweeping {} zatoshi ({} fee)", total, FEE_ZATOSHI);

    let build_config = BuildConfig::Standard {
        sapling_anchor: Some(anchor),
        orchard_anchor: None,
    };

    let mut builder = Builder::new(params, anchor_height, build_config);

    let dfvk = extsk.to_diversifiable_full_viewing_key();
    let fvk = dfvk.fvk().clone();

    for found in &notes {
        let path: MerklePath = found
            .witness
            .path()
            .ok_or_else(|| anyhow!("Witness has no path"))?;
        builder
            .add_sapling_spend::<()>(fvk.clone(), found.note.clone(), path)
            .map_err(|e| anyhow!("add_sapling_spend: {:?}", e))?;
    }

    let ovk = extsk.expsk.ovk;
    builder
        .add_sapling_output::<()>(
            Some(ovk),
            *recipient,
            Zatoshis::from_u64(send_amount).map_err(|_| anyhow!("Invalid amount"))?,
            MemoBytes::empty(),
        )
        .map_err(|e| anyhow!("add_sapling_output: {:?}", e))?;

    let signing_set = TransparentSigningSet::default();
    let fee_rule = zcash_primitives::transaction::fees::zip317::FeeRule::standard();

    let result = builder
        .build(
            &signing_set,
            &[extsk.clone()],
            &[],
            rand::rngs::OsRng,
            prover,
            prover,
            &fee_rule,
        )
        .map_err(|e| anyhow!("build: {:?}", e))?;

    let tx = result.transaction();
    let txid = format!("{}", tx.txid());
    let mut tx_bytes = Vec::new();
    tx.write(&mut tx_bytes)?;

    Ok((txid, tx_bytes, send_amount))
}

// ─── HTTP handler ───

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let body = serde_json::json!({ "error": self.0.to_string() });
        (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        AppError(err.into())
    }
}

async fn handle_sweep(
    Json(req): Json<SweepRequest>,
) -> std::result::Result<Json<SweepResponse>, AppError> {
    let raw_key = decode_key(&req.key)?;
    let sk = derive_spending_key(&raw_key);
    let extsk = ExtendedSpendingKey::master(&sk);
    let dfvk = extsk.to_diversifiable_full_viewing_key();
    let ivk = PreparedIncomingViewingKey::new(&dfvk.to_ivk(Scope::External));
    let recipient = decode_sapling_address(&req.recipient)?;

    let mut client = connect_lwd(req.testnet).await?;
    let tip = get_latest_height(&mut client).await?;
    let scan_start = tip.saturating_sub(SCAN_BLOCKS);
    tracing::info!("Scanning blocks {} to {}", scan_start, tip);

    let tree_state = get_tree_state(&mut client, scan_start).await?;
    let tree_bytes = hex::decode(&tree_state.sapling_tree).context("Bad tree state hex")?;
    let initial_tree: CommitmentTree<Node, 32> =
        read_commitment_tree(&tree_bytes[..]).context("Failed to parse tree state")?;

    let blocks = get_block_range(&mut client, scan_start + 1, tip).await?;
    let notes = scan_blocks_for_notes(&blocks, &initial_tree, &ivk)?;

    if notes.is_empty() {
        return Err(AppError(anyhow!(
            "No funds found. Payment may not have confirmed yet or is older than ~{} hours.",
            SCAN_BLOCKS * 75 / 3600
        )));
    }

    tracing::info!("Loading Sapling proving parameters...");
    let prover = LocalTxProver::with_default_location()
        .ok_or_else(|| anyhow!("Sapling params not found. Run zcash-fetch-params first."))?;

    let anchor = Anchor::from(initial_tree.root());
    let anchor_height = BlockHeight::from_u32(tip as u32);

    let (txid, tx_bytes, send_amount) = if req.testnet {
        build_sweep(
            TestNetwork, &extsk, notes, &recipient, anchor_height, anchor, &prover,
        )?
    } else {
        build_sweep(
            MainNetwork, &extsk, notes, &recipient, anchor_height, anchor, &prover,
        )?
    };

    broadcast_tx(&mut client, tx_bytes).await?;
    tracing::info!("Broadcast! txid = {}", txid);

    Ok(Json(SweepResponse {
        txid,
        amount_zatoshi: send_amount,
        fee_zatoshi: FEE_ZATOSHI,
    }))
}

// ─── Main ───

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/api/sweep", post(handle_sweep))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    tracing::info!("Sweep backend listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
