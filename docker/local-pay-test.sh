#!/bin/sh
# Real-case Phantom scan-to-pay test on a private localnet:
#   - Postgres + POS backend in Docker (docker-compose.local-pay.yml)
#   - Solana test-validator natively on the host (arm64: no usable Docker image)
#   - Solana Pay URI + qr-pay.png for a real Phantom scan
#
# usage: sh docker/local-pay-test.sh [up|down]

set -eu

cd "$(dirname "$0")/.."
COMPOSE="docker compose -f docker/docker-compose.local-pay.yml"
API=http://localhost:8080
RPC_URL=http://localhost:8899
EMAIL=merchant@pos.local
PASS=merchant-pass-1
STATE_DIR=docker/.local-pay
VALIDATOR_LOG=$STATE_DIR/validator.log
LEDGER_DIR=$STATE_DIR/ledger
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"

if [ "${1:-up}" = "down" ]; then
  pkill -f solana-test-validator 2>/dev/null || true
  $COMPOSE down -v
  echo "localnet stopped."
  exit 0
fi

step() { printf '\n== %s ==\n' "$1"; }

[ -x "$HOME/.local/share/solana/install/active_release/bin/solana" ] || {
  echo "solana CLI missing: curl -sSf https://release.anza.xyz/v2.3.1/install | sh"; exit 1; }

mkdir -p "$STATE_DIR"

step "bring up docker stack (postgres + backend)"
$COMPOSE up -d --build --quiet-pull
i=0
until curl -sf "$API/health" >/dev/null; do
  i=$((i + 1)); [ "$i" -gt 60 ] && { echo "backend not healthy"; exit 1; }
  sleep 2
done
echo "backend ok"

step "start private test-validator"
if solana cluster-version --url "$RPC_URL" >/dev/null 2>&1; then
  echo "already running"
else
  nohup solana-test-validator --reset \
    --ledger "$LEDGER_DIR" --quiet >"$VALIDATOR_LOG" 2>&1 &
  i=0
  until solana cluster-version --url "$RPC_URL" >/dev/null 2>&1; do
    i=$((i + 1)); [ "$i" -gt 60 ] && { echo "validator not ready; see $VALIDATOR_LOG"; exit 1; }
    sleep 2
  done
fi
echo "validator ok at $RPC_URL"

step "merchant wallet + airdrop"
mkdir -p "$STATE_DIR"
[ -f "$STATE_DIR/merchant.json" ] || solana-keygen new --no-bip39-passphrase --force -o "$STATE_DIR/merchant.json" >/dev/null
MERCHANT=$(solana address -k "$STATE_DIR/merchant.json")
BAL=$(solana balance "$MERCHANT" --url "$RPC_URL" | grep -o '[0-9.]*' | head -1)
if [ "${BAL%%.*}" = "0" ]; then
  solana airdrop 100 "$MERCHANT" --url "$RPC_URL" >/dev/null
fi
echo "merchant address: $MERCHANT"

step "seed: register, login, shop, destination, goods"
curl -sf -X POST "$API/auth/register" \
  -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\"}" >/dev/null || true

TOKEN=$(curl -sf -X POST "$API/auth/login" \
  -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\"}" \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')

SHOP=$(curl -sf -X POST "$API/shops" -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"name":"Phantom Test Shop"}' \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["shop_id"])')

curl -sf -X POST "$API/shops/$SHOP/destinations" -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d "{\"shop_id\":\"$SHOP\",\"chain\":\"solana\",\"address\":\"$MERCHANT\",\"label\":\"localnet treasury\"}" \
  | python3 -m json.tool

GOODS=$(curl -sf -X POST "$API/shops/$SHOP/goods" -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d "{\"shop_id\":\"$SHOP\",\"name\":\"Coffee\",\"price\":\"1.5\",\"cost\":\"0.5\",\"units\":10,\"barcode\":\"4006381333931\"}" \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["goods_id"])')
echo "shop=$SHOP goods=$GOODS"

step "checkout -> invoice + solana pay payload"
DEST=$(curl -sf "$API/shops/$SHOP/destinations" -H "authorization: Bearer $TOKEN" \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)[0]["destination_id"])')

PAYLOAD=$(curl -sf -X POST "$API/shops/$SHOP/checkout" -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d "{\"shop_id\":\"$SHOP\",\"goods_ids\":[\"$GOODS\"],\"destination_id\":\"$DEST\"}" \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["qr_payload"])')
echo "qr payload: $PAYLOAD"

step "verify payload is a Phantom-compatible Solana Pay transfer URI"
python3 - "$PAYLOAD" "$MERCHANT" <<'EOF'
import sys
payload, merchant = sys.argv[1], sys.argv[2]
prefix, query = payload.split("?", 1)
assert prefix == f"solana:{merchant}", f"bad recipient: {prefix}"
fields = dict(kv.split("=", 1) for kv in query.split("&"))
assert fields.get("amount") == "1.5", f"bad amount: {fields}"
print("OK: solana:<merchant>?amount=1.5&message=<destination-id>")
EOF

step "render qr to scan"
cargo run -q -p pos-qr-code --example qr_png -- "$PAYLOAD" qr-pay.png

step "merchant balance on localnet"
solana balance "$MERCHANT" --url "$RPC_URL"

LAN_IP=$(ipconfig getifaddr en0 2>/dev/null || echo "<LAN-IP>")
cat <<EOF

done.
  payload : $PAYLOAD
  qr file : $(pwd)/qr-pay.png

Phantom setup (real scan):
  1. phone + this computer on the same LAN
  2. Phantom -> Settings -> (active network) -> RPC URL:
       http://$LAN_IP:8899
  3. create/import a throwaway wallet on that network
  4. scan qr-pay.png with the Phantom camera
       -> it prefills 1.5 SOL to $MERCHANT
  5. confirm, then check the merchant balance:
       solana balance $MERCHANT --url $RPC_URL

teardown: sh docker/local-pay-test.sh down
EOF
