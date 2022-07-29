This is an implementation of the [ICS 721
specification](https://github.com/cosmos/ibc/tree/master/spec/app/ics-721-nft-transfer)
written in CosmWasm.

**This code has not yet been audited. Please, do not use it for
anything mission critical.**

This is an extended implementation of [ICS
721](https://github.com/cosmos/ibc/tree/master/spec/app/ics-721-nft-transfer)
to make ICS 721 NFTs
[cw721](https://github.com/CosmWasm/cw-nfts/tree/main/packages/cw721)
compatible. This means that any dapp that interacts with cw721 NFTs
can also interact with NFTs from other blockchains when this
implementation of ICS 721 is on the receiving side of the transfer.

Three contracts orchestrate this:

1. `cw-ics721-bridge` implements the "NFT transfer bridge" and "NFT
   asset tracking module" parts of the ICS 721 spec.
2. `ics-escrow` escrows NFTs while are away on foreign chains.

## Sending NFTs

To send a NFT from one chain to another over IBC:

1. The bridge contract receives the NFT via cw721's `Send` method.
2. The bridge contract deserializes the `msg` field on `Send` into an
   `IbcAwayMsg` which specifies the channel the NFT ought to be sent
   away on as well as the receiver on the receiving chain.
3. The sent cw721 is locked in the escrow contract for the sending
   channel.
4. A message is sent to the bridge contract on the other side of the
   connection which causes it to mint an equivalent NFT on the
   receiving chain.
5. Upon receiving confirmation (ACK) from the receiving chain that the
   transfer has completed, burns the NFTs if they are returning to
   their original chain. For example for the path `A -> B -> A`, the
   NFTs minted on transfer from `A -> B` are burned on transfer from
   `B -> A`.

## Receiving NFTs

Upon receiving the message sent in (4) above, the bridge contract
checks if the NFT it is receiving had previously been sent from it to
the other chain. If it has been, the contract:

1. Updates the classID field on the NFT in accordance with the
   [specification](https://github.com/cosmos/ibc/tree/main/spec/app/ics-721-nft-transfer#data-structures).
2. Unescrows the NFTs that are returning and sends them to the
   receivers specified in the
   [`NonFungibleTokenPacketData`](https://github.com/cosmos/ibc/tree/main/spec/app/ics-721-nft-transfer#data-structures)
   message.

If the bridge contract determines that the NFTs being transfered have
not previously been sent from the chain it:

1. Updates the classID field to add its path information.
2. If it has never seen a NFT that is part of the collection being
   sent over, instantiates a new cw721 contract to represent that
   collection.
3. Mints new cw721 NFTs using the instantiated cw721 for the
   collection being sent over for the receivers.
