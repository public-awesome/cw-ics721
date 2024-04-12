#!/bin/bash
# ----------------------------------------------------
# Instantiates the ICS721 contract with cw721_base_code_id and pauser
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

printf -v MSG '{"cw721_base_code_id": %s, "pauser": "%s"}' $CODE_ID_CW721 $WALLET_OWNER
CMD="$CLI tx wasm instantiate $CODE_ID_ICS721 '$MSG' --label 'ICS721 with rate limiter outgoing proxy'"
CMD+=" --from $WALLET --admin $WALLET_ADMIN"
CMD+=" --gas $CLI_GAS --gas-prices $CLI_GAS_PRICES --gas-adjustment $CLI_GAS_ADJUSTMENT"
CMD+=" --chain-id $CHAIN_ID --node $CHAIN_NODE -y"

echo "executing: $CMD" >&2
eval $CMD