#!/bin/bash
# ----------------------------------------------------
# Instantiates the ICS721 Outgoing Whitelist Channel Proxy contract
# with the (channels, collections, checksums, collection fees) whitelist and reference to the ICS721 contract
# ----------------------------------------------------

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)

# select chain
if [[ -z "$CHAIN" ]]; then
    source "$SCRIPT_DIR"/select-chain.sh
    CHAIN=$(select_chain)
    export CHAIN
fi
echo "reading $SCRIPT_DIR/$CHAIN.env"
source "$SCRIPT_DIR"/"$CHAIN".env

MSG=$(
    cat <<EOF
{
    "config": {
        "origin": "$ADDR_ICS721",
        "owner": "$WALLET_OWNER"
    },
    "proxy": {
        "rate_limit": {"per_block": 100},
        "channels": $CHANNELS,
        "collections": $COLLECTIONS
    }
}
EOF
)
LABEL="ICS721 Outgoing Whitelist Channel Proxy"
CMD="$CLI tx wasm instantiate $CODE_ID_OUTGOING_PROXY '$MSG'"
CMD+=" --label '$LABEL'"
CMD+=" --from $WALLET --admin $WALLET_ADMIN"
CMD+=" --gas $CLI_GAS --gas-prices $CLI_GAS_PRICES --gas-adjustment $CLI_GAS_ADJUSTMENT"
CMD+=" --chain-id $CHAIN_ID --node $CHAIN_NODE -y"

echo "executing: $CMD" >&2
eval $CMD