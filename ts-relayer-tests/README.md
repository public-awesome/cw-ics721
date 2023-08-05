# ICS-721 ts-relayer tests.

These tests relay NFTs between a local wasmd and osmosisd chain. This
is based on Ethan Frey's
[cw-ibc-demo](https://github.com/confio/cw-ibc-demo).

## Setup

Ensure you have node 14+ (16+ recommended):

```
node --version
```

Then install via npm as typical:

```
npm install
```

## Development

Build the source:

```
npm run build
```

Clean it up with prettier and eslint:

```
npm run fix
```

## Testing

### Run two chains in docker

This actually runs the test codes on contracts. To do so, we need to
start two blockchains in the background and then run the process. This
requires that you have docker installed and running on your local
machine. If you don't, please do that first before running the
scripts. (Also, they only work on Linux and MacOS... sorry Windows
folks, you are welcome to PR an equivalent).

Terminal 1:

```
./ci-scripts/wasmd/start.sh
```

Terminal 2:

```
./ci-scripts/osmosis/start.sh
```

If those start properly, you should see a series of `executed block`
messages. If they fail, check `debug.log` in that directory for full
log messages.

### Run first test

To run the tests, you will need to compile your contracts, and place
them in the `internal` folder. To make it easy, you can simply run
the `full-test` which will compile the contracts, place them in the
correct folder, "fix" your tests, and then run them.

Terminal 3:

```
npm run full-test
```

**NOTE** If you modify your contract, you will need to recompile the
contracts again, you can use `full-test` for that. ics721.spec.test
uses a cw721-base binary build, stored at
`tests/internal/cw721_base_v0.18.0.wasm` ([cw-nfs](https://github.com/CosmWasm/cw-nfts/releases/tag/v0.18.0)).

### Run tests

To run the tests again, you can simply use `test`.

Terminal 3:

```
npm run test
```

**NOTE**: If you modified your contract, you will need to run
`full-test` to run the tests on the new contracts.

### Stop chains

You may run and re-run tests many times. When you are done with it and
want to free up some system resources (stop running two blockchains in
the background), you need to run these commands to stop them properly:

```
./ci-scripts/wasmd/stop.sh
./ci-scripts/osmosis/stop.sh
```
