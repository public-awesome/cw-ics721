This is a work in progress implementation of the [ICS 721
specification](https://github.com/cosmos/ibc/tree/master/spec/app/ics-721-nft-transfer). The
implementation is extended to make IBC NFTs compatible with the
[cw721](https://github.com/CosmWasm/cw-nfts/tree/main/packages/cw721)
specification. This allows dapps and contracts on a CosmWasm chain to
transparently interact with ICS 721 NFTs as if they were native to the
chain.

The specification is implemented as a collection of contracts:

1. `cw-ics721-bridge` implements the "NFT transfer bridge" and "NFT
   asset tracking module" parts of the ICS 721 spec.
2. `ics-escrow` escrows NFTs while are away on foreign chains.
3. `cw721-ics` is a cw721 implementation that is modified to allow the
   asset tracking module to transfer and send NFTs on behalf of users
   interacting with that contract and the ICS 721 interface.
