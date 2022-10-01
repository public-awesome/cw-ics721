optimize:
    docker run --rm -v "$(pwd)":/code --platform linux/amd64 \
      --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
      --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
      cosmwasm/workspace-optimizer:0.12.8

unit-test:
    cargo test

simulation-test: optimize
	go test ./...

ts-relayer-test:
	cd ts-relayer-tests && npm i && npm run full-test

test: unit-test simulation-test ts-relayer-test
