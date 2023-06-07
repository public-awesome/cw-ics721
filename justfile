optimize:
    docker run --rm -v "$(pwd)":/code --platform linux/amd64 \
      --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
      --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
      cosmwasm/workspace-optimizer:0.12.13

# Version of optimize that will run significantly faster on macbooks.
optimize-arm:
    docker run --rm -v "$(pwd)":/code --platform linux/arm64 \
      --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
      --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
      cosmwasm/workspace-optimizer-arm:0.12.13

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
	cargo +nightly fmt --all
	cargo +nightly clippy --all-targets -- -D warnings
