use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use blake2b_simd::Params as Blake2bParams;
use incrementalmerkletree::{frontier::CommitmentTree, witness::IncrementalWitness};
use sapling_crypto::{
    keys::PreparedIncomingViewingKey,
    note::ExtractedNoteCommitment,
    note_encryption::{CompactOutputDescription, SaplingDomain, Zip212Enforcement},
    zip32::ExtendedSpendingKey,
    Anchor, Node, Note, Nullifier, PaymentAddress,
};
use zcash_address::unified::{Container, Encoding as UnifiedEncoding, Receiver};
use zcash_note_encryption::{try_compact_note_decryption, EphemeralKeyBytes};
use zcash_primitives::{
    consensus::{BlockHeight, MainNetwork},
    merkle_tree::read_commitment_tree,
    transaction::{
        builder::{BuildConfig, Builder},
        fees::FeeRule as _,
    },
};
use zcash_protocol::{memo::MemoBytes, value::Zatoshis};
use zcash_transparent::builder::TransparentSigningSet;
use zip32::Scope;

// ─── Types for block data (passed as JSON, small) ───

#[derive(Deserialize)]
struct CompactSpend {
    nf: String,
}

#[derive(Deserialize)]
struct CompactOutput {
    cmu: String,
    epk: String,
    ciphertext: String,
}

#[derive(Deserialize)]
struct CompactTx {
    #[serde(default)]
    spends: Vec<CompactSpend>,
    #[serde(default)]
    outputs: Vec<CompactOutput>,
}

#[derive(Deserialize)]
struct CompactBlock {
    #[allow(dead_code)]
    height: u64,
    vtx: Vec<CompactTx>,
}

#[derive(Deserialize)]
struct SweepInput {
    key: String,
    recipient: String,
    tree_state_hex: String,
    blocks: Vec<CompactBlock>,
    tip_height: u32,
}

#[derive(Serialize)]
struct SweepResult {
    tx_hex: String,
    txid: String,
    amount_zatoshi: u64,
    fee_zatoshi: u64,
}

#[derive(Serialize)]
struct SweepCheckResult {
    status: &'static str,
    amount_zatoshi: Option<u64>,
    fee_zatoshi: Option<u64>,
}

struct FoundNote {
    note: Note,
    witness: IncrementalWitness<Node, 32>,
    nullifier: Nullifier,
}

struct ClaimablePayment {
    extsk: ExtendedSpendingKey,
    recipient: PaymentAddress,
    notes: Vec<FoundNote>,
    anchor: Anchor,
    tip_height: u32,
    send_amount: u64,
    fee: u64,
}

enum ScanOutcome {
    Claimable(ClaimablePayment),
    Spent,
    Missing,
}

// ─── Key helpers ───

fn decode_key(bech32_key: &str) -> Result<[u8; 32], String> {
    let (hrp, data) = bech32::decode(bech32_key).map_err(|e| format!("Bad key: {}", e))?;
    let hrp_str: &str = hrp.as_str();
    if hrp_str != "zkey" {
        return Err(format!("Expected HRP 'zkey', got '{}'", hrp_str));
    }
    let bytes: Vec<u8> = Vec::<u8>::from(data);
    bytes
        .try_into()
        .map_err(|_| "Key must be 32 bytes".to_string())
}

fn derive_sk(key: &[u8; 32]) -> [u8; 32] {
    let h = Blake2bParams::new()
        .hash_length(32)
        .personal(b"Zcash_PaymentURI")
        .hash(key);
    let mut sk = [0u8; 32];
    sk.copy_from_slice(h.as_bytes());
    sk
}

fn decode_address(addr: &str) -> Result<PaymentAddress, String> {
    if let Ok((hrp, data)) = bech32::decode(addr) {
        let hrp_str: &str = hrp.as_str();
        if hrp_str == "zs" || hrp_str == "ztestsapling" {
            let bytes: Vec<u8> = Vec::<u8>::from(data);
            let arr: [u8; 43] = bytes
                .try_into()
                .map_err(|_| "Address must be 43 bytes".to_string())?;
            return PaymentAddress::from_bytes(&arr)
                .ok_or_else(|| "Invalid address".to_string());
        }
    }

    if let Ok((_network, ua)) = zcash_address::unified::Address::decode(addr) {
        let sapling_receiver = ua
            .items()
            .into_iter()
            .find_map(|receiver| match receiver {
                Receiver::Sapling(data) => Some(data),
                _ => None,
            })
            .ok_or_else(|| "Unified address is missing a Sapling receiver".to_string())?;

        return PaymentAddress::from_bytes(&sapling_receiver)
            .ok_or_else(|| "Invalid Sapling receiver".to_string());
    }

    Err("Expected a shielded Zcash address (u1... or zs1...)".to_string())
}

fn required_fee(note_count: usize, tip_height: u32) -> Result<u64, JsError> {
    let fee_rule = zcash_primitives::transaction::fees::zip317::FeeRule::standard();
    let fee = fee_rule
        .fee_required(
            &MainNetwork,
            BlockHeight::from_u32(tip_height),
            std::iter::empty(),
            std::iter::empty(),
            note_count,
            1,
            0,
        )
        .map_err(|e| JsError::new(&format!("fee: {}", e)))?;

    Ok(u64::from(fee))
}

fn parse_input(input_json: &str) -> Result<SweepInput, JsError> {
    serde_json::from_str(input_json).map_err(|e| JsError::new(&format!("Bad input: {}", e)))
}

fn scan_payment(input: SweepInput) -> Result<ScanOutcome, JsError> {
    let raw_key = decode_key(&input.key).map_err(|e| JsError::new(&e))?;
    let sk = derive_sk(&raw_key);
    let extsk = ExtendedSpendingKey::master(&sk);
    let dfvk = extsk.to_diversifiable_full_viewing_key();
    let ivk = PreparedIncomingViewingKey::new(&dfvk.to_ivk(Scope::External));
    let nk = dfvk.to_nk(Scope::External);
    let recipient = decode_address(&input.recipient).map_err(|e| JsError::new(&e))?;

    let tree_bytes = hex::decode(&input.tree_state_hex)
        .map_err(|e| JsError::new(&format!("Bad tree hex: {}", e)))?;
    let mut tree: CommitmentTree<Node, 32> = read_commitment_tree(&tree_bytes[..])
        .map_err(|e| JsError::new(&format!("Bad tree: {}", e)))?;

    let domain = SaplingDomain::new(Zip212Enforcement::On);
    let mut found_notes: Vec<FoundNote> = Vec::new();
    let mut spent_nullifiers = HashSet::<[u8; 32]>::new();

    for block in &input.blocks {
        for tx in &block.vtx {
            for spend in &tx.spends {
                let nf_bytes: [u8; 32] = hex::decode(&spend.nf)
                    .map_err(|_| JsError::new("Bad nullifier hex"))?
                    .try_into()
                    .map_err(|_| JsError::new("Nullifier not 32 bytes"))?;
                spent_nullifiers.insert(nf_bytes);
            }

            for output in &tx.outputs {
                let cmu_bytes: [u8; 32] = hex::decode(&output.cmu)
                    .map_err(|_| JsError::new("Bad cmu hex"))?
                    .try_into()
                    .map_err(|_| JsError::new("cmu not 32 bytes"))?;

                let cmu = match Option::from(ExtractedNoteCommitment::from_bytes(&cmu_bytes)) {
                    Some(c) => c,
                    None => continue,
                };

                let node = Node::from_cmu(&cmu);
                tree.append(node).map_err(|_| JsError::new("Tree full"))?;
                for found in &mut found_notes {
                    found
                        .witness
                        .append(node)
                        .map_err(|_| JsError::new("Witness update failed"))?;
                }

                let epk_bytes: [u8; 32] = hex::decode(&output.epk)
                    .map_err(|_| JsError::new("Bad epk hex"))?
                    .try_into()
                    .map_err(|_| JsError::new("epk not 32 bytes"))?;

                let ct_bytes =
                    hex::decode(&output.ciphertext).map_err(|_| JsError::new("Bad ciphertext hex"))?;
                if ct_bytes.len() < 52 {
                    continue;
                }

                let mut ct = [0u8; 52];
                ct.copy_from_slice(&ct_bytes[..52]);

                let compact = CompactOutputDescription {
                    ephemeral_key: EphemeralKeyBytes(epk_bytes),
                    cmu,
                    enc_ciphertext: ct,
                };

                if let Some((note, _)) = try_compact_note_decryption(&domain, &ivk, &compact) {
                    let witness = IncrementalWitness::from_tree(tree.clone())
                        .expect("tree non-empty after append");
                    let position = u64::from(witness.witnessed_position());
                    let nullifier = note.nf(&nk, position);
                    found_notes.push(FoundNote {
                        note,
                        witness,
                        nullifier,
                    });
                }
            }
        }
    }

    if found_notes.is_empty() {
        return Ok(ScanOutcome::Missing);
    }

    let notes: Vec<FoundNote> = found_notes
        .into_iter()
        .filter(|found| !spent_nullifiers.contains(&found.nullifier.0))
        .collect();

    if notes.is_empty() {
        return Ok(ScanOutcome::Spent);
    }

    let total: u64 = notes.iter().map(|n| n.note.value().inner()).sum();
    let fee = required_fee(notes.len(), input.tip_height)?;
    if total <= fee {
        return Err(JsError::new(&format!(
            "Balance ({} zatoshi) too small for fee ({})",
            total, fee
        )));
    }

    Ok(ScanOutcome::Claimable(ClaimablePayment {
        extsk,
        recipient,
        notes,
        anchor: Anchor::from(tree.root()),
        tip_height: input.tip_height,
        send_amount: total - fee,
        fee,
    }))
}

// ─── Public WASM API ───

#[wasm_bindgen]
pub fn inspect_payment(input_json: &str) -> Result<String, JsError> {
    let input = parse_input(input_json)?;
    let result = match scan_payment(input)? {
        ScanOutcome::Claimable(payment) => SweepCheckResult {
            status: "claimable",
            amount_zatoshi: Some(payment.send_amount),
            fee_zatoshi: Some(payment.fee),
        },
        ScanOutcome::Spent => SweepCheckResult {
            status: "spent",
            amount_zatoshi: None,
            fee_zatoshi: None,
        },
        ScanOutcome::Missing => SweepCheckResult {
            status: "missing",
            amount_zatoshi: None,
            fee_zatoshi: None,
        },
    };

    serde_json::to_string(&result).map_err(|e| JsError::new(&format!("json: {}", e)))
}

#[wasm_bindgen]
pub fn build_sweep_transaction(
    input_json: &str,
    spend_params: &[u8],
    output_params: &[u8],
) -> Result<String, JsError> {
    let input = parse_input(input_json)?;
    let payment = match scan_payment(input)? {
        ScanOutcome::Claimable(payment) => payment,
        ScanOutcome::Spent => return Err(JsError::new("This link was already claimed or canceled")),
        ScanOutcome::Missing => {
            return Err(JsError::new(
                "No claimable funds found. The link may still be confirming or may be older than 24 hours.",
            ))
        }
    };

    let build_config = BuildConfig::Standard {
        sapling_anchor: Some(payment.anchor),
        orchard_anchor: None,
    };

    let mut builder = Builder::new(
        MainNetwork,
        BlockHeight::from_u32(payment.tip_height),
        build_config,
    );

    let dfvk = payment.extsk.to_diversifiable_full_viewing_key();
    let fvk = dfvk.fvk().clone();
    for found in &payment.notes {
        let path = found
            .witness
            .path()
            .ok_or_else(|| JsError::new("No merkle path"))?;
        builder
            .add_sapling_spend::<()>(fvk.clone(), found.note.clone(), path)
            .map_err(|e| JsError::new(&format!("add_spend: {:?}", e)))?;
    }

    builder
        .add_sapling_output::<()>(
            Some(payment.extsk.expsk.ovk),
            payment.recipient,
            Zatoshis::from_u64(payment.send_amount).map_err(|_| JsError::new("Bad amount"))?,
            MemoBytes::empty(),
        )
        .map_err(|e| JsError::new(&format!("add_output: {:?}", e)))?;

    let prover = zcash_proofs::prover::LocalTxProver::from_bytes(spend_params, output_params);
    let signing_set = TransparentSigningSet::default();
    let fee_rule = zcash_primitives::transaction::fees::zip317::FeeRule::standard();

    let result = builder
        .build(
            &signing_set,
            &[payment.extsk.clone()],
            &[],
            rand::rngs::OsRng,
            &prover,
            &prover,
            &fee_rule,
        )
        .map_err(|e| JsError::new(&format!("build: {:?}", e)))?;

    let tx = result.transaction();
    let txid = format!("{}", tx.txid());
    let mut tx_bytes = Vec::new();
    tx.write(&mut tx_bytes)
        .map_err(|e| JsError::new(&format!("serialize: {}", e)))?;

    let output = SweepResult {
        tx_hex: hex::encode(&tx_bytes),
        txid,
        amount_zatoshi: payment.send_amount,
        fee_zatoshi: payment.fee,
    };

    serde_json::to_string(&output).map_err(|e| JsError::new(&format!("json: {}", e)))
}

// Address derivation for the create flow
#[wasm_bindgen]
pub fn derive_address_bytes(seed: &[u8]) -> Result<Vec<u8>, JsError> {
    if seed.len() != 32 {
        return Err(JsError::new("Seed must be 32 bytes"));
    }
    let extsk = ExtendedSpendingKey::master(seed);
    let dfvk = extsk.to_diversifiable_full_viewing_key();
    let (_, address) = dfvk.default_address();
    Ok(address.to_bytes().to_vec())
}
