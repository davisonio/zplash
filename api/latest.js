const grpc = require("@grpc/grpc-js");
const protoLoader = require("@grpc/proto-loader");
const path = require("path");

const PROTO_PATH = path.join(__dirname, "../proxy/proto/service.proto");
const LWD = "zec.rocks:443";

let client;
function getClient() {
  if (client) return client;
  const pkgDef = protoLoader.loadSync(PROTO_PATH, { keepCase: true, longs: Number });
  const proto = grpc.loadPackageDefinition(pkgDef).cash.z.wallet.sdk.rpc;
  client = new proto.CompactTxStreamer(LWD, grpc.credentials.createSsl());
  return client;
}

module.exports = (req, res) => {
  res.setHeader("Access-Control-Allow-Origin", "*");
  getClient().GetLatestBlock({}, (err, resp) => {
    if (err) return res.status(500).json({ error: err.message });
    res.json({ height: Number(resp.height) });
  });
};
