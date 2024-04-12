#!/bin/bash
# ----------------------------------------------------
# execute messages on the outgoing proxy contract, for whitelist management:
# - add/remove collection
# - add/remove channel
# - add/remove collection checksum
# - enable/disable collection|channel|checksum|fees
# ----------------------------------------------------

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
set -o pipefail

# get CHAIN_CHOICES
source "$SCRIPT_DIR"/select-chain.sh

function query_config() {
    echo "config: $($CLI query wasm contract-state smart $ADDR_OUTGOING_PROXY '{"get_config": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE --output json | jq)" >&2
    echo "contract: $($CLI query wasm contract $ADDR_OUTGOING_PROXY --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "rate limit: $($CLI query wasm contract-state smart $ADDR_OUTGOING_PROXY '{"get_rate_limit": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "channels whitelist: $($CLI query wasm contract-state smart $ADDR_OUTGOING_PROXY '{"get_channels_whitelist": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "collections whitelist: $($CLI query wasm contract-state smart $ADDR_OUTGOING_PROXY '{"get_collections_whitelist": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "collection checkums whitelist: $($CLI query wasm contract-state smart $ADDR_OUTGOING_PROXY '{"get_collection_checksums_whitelist": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
    echo "fees collection map: $($CLI query wasm contract-state smart $ADDR_OUTGOING_PROXY '{"get_fees_collection_map": {}}' --chain-id $CHAIN_ID --node $CHAIN_NODE | jq)" >&2
}

ACTION=""
CHAIN=""
VALUE=""
TYPE=""

prev_arg=""
for arg in "$@"; do
    case $arg in
    --add | --remove | --enable)
        ACTION="$arg"
        ;;
    --fees) ;;
    --type) ;;
    collection | channel | fees | checksum)
        if [[ "$prev_arg" != "--type" ]]; then
            echo "Invalid argument order. Use --type before specifying 'collection' or 'channel'." >&2
            exit 1
        fi
        TYPE="$arg"
        ;;
    *)
        if [[ " ${CHAIN_CHOICES[*]} " =~ " $arg " ]]; then
            CHAIN="$arg"
        elif [[ "$prev_arg" == "--generate-only" || "$prev_arg" == "--broadcast" ]]; then
            FILE="$arg"
        elif [[ "$prev_arg" == "--add" || "$prev_arg" == "--remove" || "$prev_arg" == "--enable" ]]; then
            VALUE="$arg"
        elif [[ "$prev_arg" == "--fees" ]]; then
            FEES="$arg"
        else
            echo "Invalid argument: $arg" >&2
            echo "Usage: $0 ${CHAIN_CHOICES_STR// /|} [--add WHITELIST|--remove WHITELIST|--enable true_or_false] --type collection|channel|checksum|fees" >&2
            exit 1
        fi
        ;;
    esac
    prev_arg="$arg"
done

# Check if all required arguments are provided
if [[ -z "$ACTION" || -z "$CHAIN" || -z "$VALUE" ]]; then
    echo "Usage: $0 ${CHAIN_CHOICES_STR// /|} [--add WHITELIST|--remove WHITELIST|--enable true_or_false] --type collection|channel|checksum|fees" >&2
    echo "Example:" >&2
    echo "$0 ${CHAIN_CHOICES_STR// /|} --add channel-1 --type channel" >&2
    echo "$0 ${CHAIN_CHOICES_STR// /|} --enable false --type collection" >&2
    exit 1
fi

echo "reading $SCRIPT_DIR/$CHAIN.env"
source "$SCRIPT_DIR"/"$CHAIN".env

case $ACTION in
--add)
    if [ "$TYPE" == "collection" ]; then
        printf -v MSG '{"update_config": {"add_collection_to_whitelist": {"collection": "%s"}}}' $VALUE
    elif [ "$TYPE" == "channel" ]; then
        printf -v MSG '{"update_config": {"add_channel_to_whitelist": {"channel": "%s"}}}' $VALUE
    elif [ "$TYPE" == "fees" ]; then
        printf -v MSG '{"update_config": {"add_fees_collection": {"collection": "%s", "fees": {"amount": "%s", "denom": "%s"}}}}' $VALUE $FEES $CLI_DENOM
    elif [ "$TYPE" == "checksum" ]; then
        printf -v MSG '{"update_config": {"add_collection_checksum_to_whitelist": {"checksum": "%s"}}}' $VALUE
    fi
    ;;
--remove)
    if [ "$TYPE" == "collection" ]; then
        printf -v MSG '{"update_config": {"remove_collection_from_whitelist": {"collection": "%s"}}}' $VALUE
    elif [ "$TYPE" == "channel" ]; then
        printf -v MSG '{"update_config": {"remove_channel_from_whitelist": {"channel": "%s"}}}' $VALUE
    elif [ "$TYPE" == "fees" ]; then
        printf -v MSG '{"update_config": {"remove_fees_collection": {"collection": "%s"}}}' $VALUE
    elif [ "$TYPE" == "checksum" ]; then
        printf -v MSG '{"update_config": {"remove_collection_checksum_from_whitelist": {"checksum": "%s"}}}' $VALUE
    fi
    ;;
--enable)
    if [ "$TYPE" == "collection" ]; then
        printf -v MSG '{"update_config": {"enable_collections_whitelist": %s}}' $VALUE
    elif [ "$TYPE" == "channel" ]; then
        printf -v MSG '{"update_config": {"enable_channels_whitelist": %s}}' $VALUE
    elif [ "$TYPE" == "fees" ]; then
        printf -v MSG '{"update_config": {"enable_fees_collections_whitelist": %s}}' $VALUE
    elif [ "$TYPE" == "checksum" ]; then
        printf -v MSG '{"update_config": {"enable_collection_checksums_whitelist": %s}}' $VALUE
    fi
    ;;
*)
    echo "Invalid action $ACTION. Use --add, --remove, or --enable." >&2
    exit 1
    ;;
esac

echo "==========================================================================================" >&2
echo "=============== updating $ADDR_OUTGOING_PROXY using dev wallet $WALLET_DEV" >&2
CMD="$CLI tx wasm execute $ADDR_OUTGOING_PROXY '$MSG'"
CMD+=" --from $WALLET"
CMD+=" --gas $CLI_GAS --gas-prices $CLI_GAS_PRICES --gas-adjustment $CLI_GAS_ADJUSTMENT"
CMD+=" --chain-id $CHAIN_ID --node $CHAIN_NODE -y"
echo "$CMD" >&2
echo "Execute TX?" >&2
select yn in "Yes" "No"; do
    case $yn in
    Yes)
        break
        ;;
    No)
        exit 0
        ;;
    *)
        echo "Please select Yes or No." >&2
        ;;
    esac
done
eval $CMD
ERROR_CODE=${PIPESTATUS[0]}
if [ $ERROR_CODE -ne 0 ]; then
    echo "ERROR executing" >&2
    exit $ERROR_CODE
fi

echo "==========================================================================================" >&2
echo "config before change $ADDR_OUTGOING_PROXY=$ADDR_OUTGOING_PROXY:" >&2
sleep 10
query_config
