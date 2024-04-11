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

function query_config() {
    echo "contract: $($CLI query wasm contract $ADDR_INCOMING_PROXY --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "origin: $($CLI query wasm contract-state smart $ADDR_INCOMING_PROXY '{"get_origin": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    CURRENT_CHANNELS=$($CLI query wasm contract-state smart $ADDR_INCOMING_PROXY '{"get_channels": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)
    echo "channels: $CURRENT_CHANNELS" >&2
}

echo "==========================================================================================" >&2
echo "configs before migration $ADDR_INCOMING_PROXY:" >&2
query_config

echo "==========================================================================================" >&2
echo "Do you want to migrate channels?" >&2
echo "current channels whitelist: $CURRENT_CHANNELS" >&2
echo "Channels from config: $CHANNELS" >&2
select yn in "Yes" "No"; do
    case $yn in
    Yes)
        MIGRATE_CHANNELS=true
        OPTION_ONE=", \"channels\": $CHANNELS"
        break
        ;;
    No)
        MIGRATE_CHANNELS=false
        OPTION_ONE=""
        break
        ;;
    *)
        echo "Please select Yes or No." >&2
        ;;
    esac
done

printf -v MSG '{
  "with_update": {
    "origin": "%s"
    %s
  }
}' \
    "$ADDR_ICS721" \
    "$OPTION_ONE"

CMD="$CLI tx wasm migrate $ADDR_INCOMING_PROXY $CODE_ID_INCOMING_PROXY '$MSG'"
CMD+=" --from $WALLET_ADMIN"
CMD+=" --gas-prices $CLI_GAS_PRICES --gas $CLI_GAS --gas-adjustment $CLI_GAS_ADJUSTMENT"
CMD+=" --chain-id $CHAIN_ID --node $CHAIN_NODE -y"

echo "executing: $CMD" >&2
eval $CMD
if [ $? -ne 0 ]; then
    echo "failed to migrate $ADDR_ICS721" >&2
    exit 1
fi

echo "==========================================================================================" >&2
echo "configs after migration $ADDR_INCOMING_PROXY:" >&2
sleep 10
query_config
