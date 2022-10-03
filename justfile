optimize:
    docker run --rm -v "$(pwd)":/code --platform linux/amd64 \
      --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
      --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
      cosmwasm/workspace-optimizer:0.12.8

unit-test:
    cargo test

simulation-test: optimize
	go test -v ./...

integration-test: optimize
    npm i --prefix ts-relayer-tests && npm run full-test --prefix ts-relayer-tests

test: unit-test simulation-test integration-test
