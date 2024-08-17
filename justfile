set dotenv-load

platform := if arch() =~ "aarch64" {"linux/arm64"} else {"linux/amd64"}
image := if arch() =~ "aarch64" {"cosmwasm/workspace-optimizer-arm64:0.16.0"} else {"cosmwasm/workspace-optimizer:0.16.0"}

alias log := optimize-watch

_default:
  @just --list --unsorted

install-tools:
  @cargo install loc
  @cargo install bat
  @brew install jq
  @npm install -g @cosmwasm/ts-codegen

loc:
  @loc --exclude /.*_test\.rs$

# Generate optimized WASM artifacts
optimize:
  #!/usr/bin/env sh
  docker run --rm -v "$(pwd)":/code --platform {{platform}} \
    --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
    --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
    {{image}}

optimize-watch:
  @tail -f optimize.log | bat --paging=never -l log

unit-test:
  cargo test

# Upload optimized WASM artifacts to chain
upload:
  #!/usr/bin/env sh
  for d in ./artifacts/*.wasm; do
    echo $d;
    $CHAIND tx wasm store $d --from $TESTNET_KEY \
      --gas-prices $GAS_PRICES --gas-adjustment 1.7 --gas auto --chain-id $CHAIN_ID \
      --node $NODE -b block --yes -o json | jq '.logs' | grep -A 1 code_id
    echo "-----------------";
  done

simulation-test: optimize
	go test -v ./...

start-local-chains:
	./ts-relayer-tests/ci-scripts/wasmd/start.sh & 2>&1
	./ts-relayer-tests/ci-scripts/osmosis/start.sh & 2>&1

stop-local-chains:
	./ts-relayer-tests/ci-scripts/wasmd/stop.sh
	./ts-relayer-tests/ci-scripts/osmosis/stop.sh

integration-test:
    npm i --prefix ts-relayer-tests && npm run full-test --prefix ts-relayer-tests

test: unit-test simulation-test integration-test

lint:
	cargo +nightly clippy --all-targets -- -D warnings
