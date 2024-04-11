#!/bin/bash
# ----------------------------------------------------
# - exports CHAIN_NET and CHAIN based on user input  -
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

printf -v MSG '{"origin": "%s", "channels": %s}' $ADDR_ICS721 $CHANNELS
LABEL="ICS721 Incoming Whitelist Channel Proxy, Managed by Ark Protocol"
CMD="$CLI tx wasm instantiate $CODE_ID_INCOMING_PROXY '$MSG'"
CMD+=" --label '$LABEL'"
CMD+=" --from $WALLET --admin $WALLET_ADMIN"
CMD+=" --gas $CLI_GAS --gas-prices $CLI_GAS_PRICES --gas-adjustment $CLI_GAS_ADJUSTMENT"
CMD+=" --chain-id $CHAIN_ID --node $CHAIN_NODE -y"

echo "executing: $CMD" >&2
eval $CMD