# Setup ICS721

For ICS721 it requires these contracts:

- ICS721: the bridge itself
- Incoming Proxy: optional contract for filtering incoming packets
- Outgoing Proxy: optional contract for filtering incoming packets

NOTE:
Below scripts use [select-chain.sh](./select-chain.sh). For each selected chain there is an `.env` file like `stargaze.env` and `osmosis.env`.

## Scripts

### Initial Setup

Scripts for setup must be executed in this order:

1. ICS721 without proxies: [instantiate-ics721.sh](./instantiate-ics721.sh)
2. Incoming Proxy: [instantiate-incoming-proxy.sh](./instantiate-incoming-proxy.sh)
3. Outgoing Proxy: [instantiate-outgoing-proxy.sh](.instantiate-outgoing-proxy.sh)

After instantiation:

- update `ADDR_ICS721`, `ADDR_INCOMING_PROXY`, `ADDR_OUTGOING_PROXY` in env file
- Note: ICS721 is instantiated without(!) proxies, proxies are added via migration (velow)

### Migration

1. ICS721 : [migrate-ics721.sh](./migrate-ics721.sh)
2. Incoming Proxy: [migrate-incoming-proxy.sh](./migrate-incoming-proxy.sh)
3. Outgoing Proxy: [migrate-outgoing-proxy.sh](.migrate-outgoing-proxy.sh)

### Outgoing Proxy Messages

Usage:

```sh
$ ./scripts/whitelist-outgoing-proxy.sh
Usage: ./scripts/whitelist-outgoing-proxy.sh stargaze|osmosis [--add WHITELIST|--remove WHITELIST|--enable true_or_false] --type collection|channel|checksum|fees
Example:
./scripts/whitelist-outgoing-proxy.sh stargaze|osmosis --add channel-1 --type channel
./scripts/whitelist-outgoing-proxy.sh stargaze|osmosis --enable false --type collection
```

The owner of the outgoing proxy contract can add, remove and enable whitelists for collections, channels, collection checksums, and collection fees.
