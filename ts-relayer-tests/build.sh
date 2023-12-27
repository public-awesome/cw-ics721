#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd ) # source: https://stackoverflow.com/a/246128/3437868
ARTIFACTS_DIR="$SCRIPT_DIR/../artifacts"
EXTERNAL_WASMS_DIR="$SCRIPT_DIR/../external-wasms"

## Compiles an optimizes the local contracts for testing with
## ts-relayer.

set -o errexit -o nounset -o pipefail
command -v shellcheck >/dev/null && shellcheck "$0"

cd "$(git rev-parse --show-toplevel)"

docker run --rm -v "$(pwd)":/code --platform linux/amd64 \
	--mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
	--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
	cosmwasm/workspace-optimizer:0.15.0

mkdir -p $SCRIPT_DIR/internal
cp $ARTIFACTS_DIR/*.wasm $SCRIPT_DIR/internal
cp $EXTERNAL_WASMS_DIR/*.wasm $SCRIPT_DIR/internal

echo "done. avaliable wasm blobs:"
ls ./ts-relayer-tests/internal
