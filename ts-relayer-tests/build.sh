#!/bin/bash

## Compiles an optimizes the local contracts for testing with
## ts-relayer.

set -o errexit -o nounset -o pipefail
command -v shellcheck >/dev/null && shellcheck "$0"

cd "$(git rev-parse --show-toplevel)"

docker run --rm -v "$(pwd)":/code --platform linux/amd64 \
	--mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
	--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
	cosmwasm/workspace-optimizer:0.14.0

mkdir -p ./ts-relayer-tests/internal
cp ./artifacts/*.wasm ./ts-relayer-tests/internal
cp ./external-wasms/*.wasm ./ts-relayer-tests/internal

echo "done. avaliable wasm blobs:"
ls ./ts-relayer-tests/internal
