#!/bin/bash
# ----------------------------------------------------
# Migrates the ICS721 contract, and sets optional:
# - incoming proxy
# - outgoing proxy
# - cw721 code id
# - pauser
# - cw721 admin
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

function query_config() {
    echo "cw721 code id: $($CLI query wasm contract-state smart $ADDR_ICS721 '{"cw721_code_id": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "cw721 admin: $($CLI query wasm contract-state smart $ADDR_ICS721 '{"cw721_admin": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "outgoing proxy: $($CLI query wasm contract-state smart $ADDR_ICS721 '{"outgoing_proxy": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "incoming proxy: $($CLI query wasm contract-state smart $ADDR_ICS721 '{"incoming_proxy": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "pauser: $($CLI query wasm contract-state smart $ADDR_ICS721 '{"pauser": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "contract: $($CLI query wasm contract $ADDR_ICS721 --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
}

echo "==========================================================================================" >&2
echo "configs before migration $ADDR_ICS721:" >&2
query_config

echo "==========================================================================================" >&2
echo "!!! migrating $ADDR_ICS721 data: proxy: $ADDR_OUTGOING_PROXY, cw721 code id: $CODE_ID_CW721 !!!" >&2 # use CW721 if not set
MSG=$(
    cat <<EOF
{"with_update":{
    "incoming_proxy": "$ADDR_INCOMING_PROXY",
    "outgoing_proxy": "$ADDR_OUTGOING_PROXY",
    "cw721_base_code_id": $CODE_ID_CW721,
    "pauser": "$WALLET_OWNER",
    "cw721_admin": "$WALLET_ADMIN"
    }
}
EOF
)
CMD="$CLI tx wasm migrate $ADDR_ICS721 $CODE_ID_ICS721 '$MSG'"
CMD+=" --from $WALLET_ADMIN"
CMD+=" --gas $CLI_GAS --gas-prices $CLI_GAS_PRICES --gas-adjustment $CLI_GAS_ADJUSTMENT"
CMD+=" --chain-id $CHAIN_ID --node $CHAIN_NODE -y"

echo "executing: $CMD" >&2
eval $CMD
if [ $? -ne 0 ]; then
    echo "failed to migrate $ADDR_ICS721" >&2
    exit 1
fi

echo "==========================================================================================" >&2
echo "configs after migration $ADDR_ICS721:" >&2
sleep 10
query_config
