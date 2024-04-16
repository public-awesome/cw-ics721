#!/bin/bash
# ----------------------------------------------------
# Migrate the ICS721 Outgoing Whitelist Channel Proxy contract, and sets:
# - config with origin (ICS721) and owner
# - optional channels
# - optional collections
# - proxy with optional channels and collections whitelist
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
    echo "config: $($CLI query wasm contract-state smart $ADDR_OUTGOING_PROXY '{"get_config": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "contract: $($CLI query wasm contract $ADDR_OUTGOING_PROXY --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "rate limit: $($CLI query wasm contract-state smart $ADDR_OUTGOING_PROXY '{"get_rate_limit": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    CURRENT_CHANNELS=$($CLI query wasm contract-state smart $ADDR_OUTGOING_PROXY '{"get_channels_whitelist": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)
    echo "channels whitelist: $CURRENT_CHANNELS" >&2
    CURRENT_COLLECTIONS=$($CLI query wasm contract-state smart $ADDR_OUTGOING_PROXY '{"get_collections_whitelist": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)
    echo "collections whitelist: $CURRENT_COLLECTIONS" >&2
    echo "fees collection map: $($CLI query wasm contract-state smart $ADDR_OUTGOING_PROXY '{"get_fees_collection_map": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    CURRENT_COLLECTION_CHECKSUMS=$($CLI query wasm contract-state smart $ADDR_OUTGOING_PROXY '{"get_collection_checksums_whitelist": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)
    echo "collection checksums whitelist: $CURRENT_COLLECTION_CHECKSUMS" >&2
}

echo "==========================================================================================" >&2
echo "configs before migration $ADDR_OUTGOING_PROXY:" >&2
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

echo "Do you want to migrate collections?" >&2
echo "current collections whitelist: $CURRENT_COLLECTIONS" >&2
echo "Collection from config: $COLLECTIONS" >&2
select yn in "Yes" "No"; do
    case $yn in
    Yes)
        MIGRATE_COLLECTIONS=true
        OPTION_TWO=", \"collections\": $COLLECTIONS"
        break
        ;;
    No)
        MIGRATE_COLLECTIONS=false
        OPTION_TWO=""
        break
        ;;
    *)
        echo "Please select Yes or No." >&2
        ;;
    esac
done

printf -v MSG '{
  "with_update": {
    "config": {
      "origin": "%s",
      "owner": "%s"
    },
    "proxy": {
      %s
      %s
    }
  }
}' \
    "$ADDR_ICS721" \
    "$WALLET_OWNER" \
    "$OPTION_ONE" \
    "$OPTION_TWO"

CMD="$CLI tx wasm migrate $ADDR_OUTGOING_PROXY $CODE_ID_OUTGOING_PROXY '$MSG'"
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
echo "configs after migration $ADDR_OUTGOING_PROXY:" >&2
query_config
