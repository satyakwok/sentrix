#!/bin/bash
# reset_testnet.sh — Full testnet reset with 4 validators properly registered
# Run on VPS3 as sentriscloud

set -e

ADMIN_KEY="REDACTED_FOUNDER_KEY_DRAINED_2026_04_17"  # founder

# 4 validators (addr, name, pubkey)
VAL1_ADDR="0x682126f5f973bddda2c92fb0dfce8a4ba275c99b"
VAL1_PUB="042053a0f8f9ac637764b1836629f1f8182ee74dc7d8c8441cbac73fa273ad1a95869bd26c2375e707cdbb5b28f31eee7dc2b55e8be96b3ada5c64cef48a1e9a3b"

VAL2_ADDR="0x4f9988506a5e9d7a4b0eae15e5b71a02834d6c0f"
VAL2_PUB="046ccdb2a0e827d6250e1f8e6b3a17da2c415ed346c7ef03d03997f8b21df9af480a7810a49c43a492aa0f69c36c1f576a3282a4091ece5f7aa4188bea21be88c6"

VAL3_ADDR="0x245785f409d853bb04d6910a3a0f78df7cbd184c"
VAL3_PUB="0450468c223824fe856dd62d7201c537b9979726d0bd32f3bd6eb25acaedfbed77975a3945f37d3e660ac91d50f689ca880211364748c82c8e04ff85efd7134fdf"

VAL4_ADDR="0x4e9b3c4901ce6f5f60e3fb97ab97b3352675f36b"
VAL4_PUB="04856f1ce8c2965af80da84770b436f3a56015f7cd32946ccd19830e95ec8321dd183ad176ccf4a1fc3a03b126f0380a7787ebdb3c8a9b31f1c5c681c21d862c0f"

FOUNDER="0x4f3319a747fd564136209cd5d9e7d1a1e4d142be"

echo "=== Stopping testnet validators ==="
sudo systemctl stop sentrix-testnet-val1 sentrix-testnet-val2 sentrix-testnet-val3 sentrix-testnet-val4 2>/dev/null || true

echo "=== Clearing testnet state ==="
sudo rm -rf /opt/sentrix-testnet/data/chain.db /opt/sentrix-testnet/data/identity
sudo rm -rf /opt/sentrix-testnet/data2/chain.db /opt/sentrix-testnet/data2/identity
sudo rm -rf /opt/sentrix-testnet/data3/chain.db /opt/sentrix-testnet/data3/identity
sudo rm -rf /opt/sentrix-testnet/data4/chain.db /opt/sentrix-testnet/data4/identity

for i in 1 2 3 4; do
  D=/opt/sentrix-testnet/data
  if [ $i -gt 1 ]; then D=/opt/sentrix-testnet/data$i; fi

  echo ""
  echo "=== Setting up $D (val$i) ==="

  # Init chain
  sudo -u sentriscloud env \
    SENTRIX_DATA_DIR=$D \
    SENTRIX_CHAIN_ID=7120 \
    /opt/sentrix/sentrix init --admin $FOUNDER 2>&1 | tail -1

  # Add all 4 validators atomically (order doesn't matter, but must be done before any block produced)
  for v in 1 2 3 4; do
    case $v in
      1) VA=$VAL1_ADDR; VP=$VAL1_PUB; VN="testnet-val1" ;;
      2) VA=$VAL2_ADDR; VP=$VAL2_PUB; VN="testnet-val2" ;;
      3) VA=$VAL3_ADDR; VP=$VAL3_PUB; VN="testnet-val3" ;;
      4) VA=$VAL4_ADDR; VP=$VAL4_PUB; VN="testnet-val4" ;;
    esac
    sudo -u sentriscloud env \
      SENTRIX_DATA_DIR=$D \
      SENTRIX_CHAIN_ID=7120 \
      SENTRIX_ADMIN_KEY=$ADMIN_KEY \
      /opt/sentrix/sentrix validator add $VA $VN $VP 2>&1 | tail -1
  done
done

echo ""
echo "=== Starting all validators simultaneously ==="
sudo systemctl daemon-reload
sudo systemctl start sentrix-testnet-val1 sentrix-testnet-val2 sentrix-testnet-val3 sentrix-testnet-val4
sleep 15

echo ""
echo "=== Health check ==="
for i in 1 2 3 4; do
  PORT=$((9544 + i))
  H=$(curl -sf http://localhost:$PORT/chain/info 2>/dev/null | python3 -c "import sys,json; d=json.load(sys.stdin); print(f'height={d[\"height\"]} validators={d[\"active_validators\"]}')" 2>/dev/null || echo "DOWN")
  echo "val$i (port $PORT): $H"
done
