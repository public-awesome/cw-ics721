platform := if arch() =~ "aarch64" {"linux/arm64"} else {"linux/amd64"}
image := if arch() =~ "aarch64" {"cosmwasm/workspace-optimizer-arm64:0.12.12"} else {"cosmwasm/workspace-optimizer:0.12.12"}
optimize:
    docker run --rm -v "$(pwd)":/code --platform {{platform}} \
      --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
      --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
      {{image}}
      
unit-test:
    cargo test

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
