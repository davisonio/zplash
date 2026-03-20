import initWasm, {
  build_sweep_transaction,
  inspect_payment,
} from '../../wasm-sweep/pkg/zplash_wasm.js';

const PROXY = import.meta.env.VITE_PROXY_URL || 'http://localhost:3001';
const SCAN_BLOCKS = 1200;

let wasmReady = false;
let wasmLoading: Promise<void> | null = null;

async function ensureWasm() {
  if (wasmReady) return;
  if (!wasmLoading) {
    wasmLoading = initWasm().then(() => { wasmReady = true; });
  }
  await wasmLoading;
}

// Cache proving params after first download
let spendParams: Uint8Array | null = null;
let outputParams: Uint8Array | null = null;

async function loadParams(onProgress?: (msg: string) => void) {
  if (spendParams && outputParams) return { spend: spendParams, output: outputParams };

  onProgress?.('Downloading proving parameters (~50MB, one-time)...');

  const [spendResp, outputResp] = await Promise.all([
    fetch(`${PROXY}/api/params/spend`),
    fetch(`${PROXY}/api/params/output`),
  ]);

  if (!spendResp.ok || !outputResp.ok) {
    throw new Error('Failed to download Sapling proving parameters');
  }

  spendParams = new Uint8Array(await spendResp.arrayBuffer());
  outputParams = new Uint8Array(await outputResp.arrayBuffer());

  return { spend: spendParams, output: outputParams };
}

export interface SweepResult {
  txid: string;
  amountZatoshi: number;
  feeZatoshi: number;
}

export async function sweepFunds(
  key: string,
  recipient: string,
  onProgress?: (msg: string) => void,
): Promise<SweepResult> {
  await ensureWasm();

  onProgress?.('Connecting to Zcash network...');
  const latestResp = await fetch(`${PROXY}/api/latest`);
  if (!latestResp.ok) throw new Error('Failed to get latest block');
  const { height: tip } = await latestResp.json();
  const scanStart = Math.max(0, tip - SCAN_BLOCKS);

  onProgress?.('Fetching commitment tree...');
  const treeResp = await fetch(`${PROXY}/api/tree?height=${scanStart}`);
  if (!treeResp.ok) throw new Error('Failed to get tree state');
  const treeData = await treeResp.json();

  onProgress?.('Checking the last 24 hours of Zcash blocks...');
  const blocksResp = await fetch(`${PROXY}/api/blocks?start=${scanStart + 1}&end=${tip}`);
  if (!blocksResp.ok) throw new Error('Failed to fetch blocks');
  const blocks = await blocksResp.json();

  // Block data goes as JSON (small). Proving params go as binary Uint8Array (large).
  const inputJson = JSON.stringify({
    key,
    recipient,
    tree_state_hex: treeData.saplingTree,
    blocks,
    tip_height: tip,
  });

  onProgress?.('Checking whether this link is still claimable...');
  const inspection = JSON.parse(inspect_payment(inputJson)) as {
    status: 'claimable' | 'spent' | 'missing';
  };

  if (inspection.status === 'spent') {
    throw new Error('This link was already claimed or canceled.');
  }

  if (inspection.status === 'missing') {
    throw new Error('No claimable funds found. The link may still be confirming or may be older than 24 hours.');
  }

  const params = await loadParams(onProgress);

  onProgress?.('Building transaction and generating zk-proof...');
  const resultJson = build_sweep_transaction(inputJson, params.spend, params.output);
  const result = JSON.parse(resultJson);

  onProgress?.('Broadcasting transaction...');
  const broadcastResp = await fetch(`${PROXY}/api/broadcast`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ txHex: result.tx_hex }),
  });

  if (!broadcastResp.ok) {
    const err = await broadcastResp.json();
    throw new Error(err.error || 'Broadcast failed');
  }

  return {
    txid: result.txid,
    amountZatoshi: result.amount_zatoshi,
    feeZatoshi: result.fee_zatoshi,
  };
}
