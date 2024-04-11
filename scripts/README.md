# Setup ICS721

For ICS721 it requires these contracts:

- ICS721: the bridge itself
- Incoming Proxy: optional contract for filtering incoming packets
- Outgoing Proxy: optional contract for filtering incoming packets

## Scripts

Scripts for setup must be executed in this order:

1. ICS721 without proxies: [instantiate-ics721.sh](./instantiate-ics721.sh)
2. Incoming Proxy: [instantiate-incoming-proxy.sh](./instantiate-incoming-proxy.sh)
3. Outgoing Proxy: [instantiate-outgoing-proxy.sh](instantiate-outgoing-proxy.sh)

In case proxies are used, ICS721 must be migrated for setting incoming and outgoing proxies:

4. Migrate ICS721: TBD

Once running, there are execute messages for proxies, allowing to add and remove channels and collections.

5. WL Collection: TBD
6. WL Channel: TBD

NOTE:
Above scripts use [select-chain.sh](./select-chain.sh). For each selected chain there is an `.env` file like `stargaze.env` and `osmosis.env`.
