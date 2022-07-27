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
3. `cw721-ics` is a cw721 implementation that is modified to allow the
   asset tracking module to transfer and send NFTs on behalf of users
   interacting with that contract and the ICS 721 interface.

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

## Asset transfer module compatability

In order to be ICS 721 compliant, we need to implement asset tracking
module
[interface](https://github.com/cosmos/ibc/tree/main/spec/app/ics-721-nft-transfer#sub-protocols)
defined in the specification. To this end, the bridge contract
implements all of the methods described therein. In order for messages
like `Transfer` and `Burn` to work, the bridge contract needs to be
able to perform those actions on behalf of an address. For example:

1. Address A tells the bridge contract they would like to send their
   NFT to address B.
2. Bridge contract fires off a submessage to `cw721-ics` to do that
   transfer.

The `cw721-ics` contract is needed here as a vanilla cw721 contract
would see the sender field on the submessage set as the bridge
contract's address and fail the transaction. For all NFT contracts
instantiated by the bridge the cw721-ics contract is used. This allows
for ICS721 interface compatability.
