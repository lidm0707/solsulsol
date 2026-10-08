// TLS termination proxy: https on :18443 → surfpool http://localhost:8899
// lets phantom's devnet endpoint land on private surfpool in e2e tests
const https = require("node:https");
const http = require("node:http");
const fs = require("node:fs");
const path = require("node:path");

const TLS = path.join(__dirname, "..", "e2e", ".phantom", "tls");
const options = {
  key: fs.readFileSync(path.join(TLS, "key.pem")),
  cert: fs.readFileSync(path.join(TLS, "cert.pem")),
};

const server = https.createServer(options, (req, res) => {
  const chunks = [];
  req.on("data", (c) => chunks.push(c));
  req.on("end", () => {
    const upstream = http.request(
      { host: "127.0.0.1", port: 8899, method: req.method, path: req.url, headers: { "Content-Type": "application/json" } },
      (r) => {
        res.writeHead(r.statusCode ?? 502, r.headers);
        r.pipe(res);
      },
    );
    upstream.on("error", (e) => {
      res.writeHead(502);
      res.end(JSON.stringify({ proxyError: e.message }));
    });
    upstream.end(Buffer.concat(chunks));
  });
});

server.listen(18443, "127.0.0.1", () => console.log("tls proxy on :18443 → surfpool :8899"));
