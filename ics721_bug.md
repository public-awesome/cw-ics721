Bug in ICS721 when an NFT gets transferred back from chain B to A.

Preliminary: NFT (forward) transferred from chain A to B
- NFT outcoume: NFT escrowed by ICS721 on chain A, NFT minted on newly, instantiated collection on chain B
- state changes:
  - entry added on chain A: `OUTGOING_CLASS_TOKEN_TO_CHANNEL` (marker, check for identifying next time, NFT gets transferred back)
  - entry added on chain B: `INCOMING_CLASS_TOKEN_TO_CHANNEL`

Expected result on back transfer
- NFT burned on chain B
- "source/OG" NFT escrowed by ICS721 transferred to given recipent
- state changes:
  - removed entry in `INCOMING_CLASS_TOKEN_TO_CHANNEL` on chain A
  - removed entry in `OUTGOING_CLASS_TOKEN_TO_CHANNEL` on chain B
Actual result:
- NFT burned on chain B
- NFT minted on newly, instantiated collection
- NFT escrowed still escrowed by ICS721 on source collection



By the spec of ics721, it is possible doing bulk transfers (whilst cw721 doesnt support this yet).
I believe this was the main objection of `ActionAggregator`'s design:
1. for each NFT either an `Action::Redemption` or `Action::Creation` is created:
   - `Action::Redemption`: unescrow nft on forward transfer or
   - `Action::Creation`: mint NFT and instantiate collection
   - in addition for redemption/back transfer, entries in `OUTGOING_CLASS_TOKEN_TO_CHANNEL` are removed. That's the bug in contract's tx, we've identified
2. Each NFT action added to `ActionAggregator`
   - interestingly `Action` enums are converted to either `VoucherRedemption` or `VoucherCreation` structs
   - then converted structs are added to aggregator
   - imo conversion not needed here, better approach here:
3. Finally all gets wrapped into a single sub message:
   - create message list for each recreation or creation struct in aggregator:
     - convert to WasmMsg with:
       - `ExecuteMsg::Callback(CallbackMsg::CreateVouchers)` or
       - `ExecuteMsg::Callback(CallbackMsg::RedeemVouchers)`
    - message list represent a list of nft `operands`
    - optional incoming proxy is added on top of operands list
    - optional callback is added at the end of operands list
   - merge message list into into single, final sub message
     - final sub message is of type `reply all` for making sure TX always succeeds
     - if there's only one message in list, this will be used for final sub message
     - if message list contains more than one entries, they are merged into `ExecuteMsg::Callback(CallbackMsg::Conjunction {operands})`

As a result a single `CallbackMsg` sub msg is created, which is either a
- `CallbackMsg::CreateVouchers`
  - appends optional instantiate sub msg
  - appends `CallbackMsg::Mint` msg
- `CallbackMsg::RedeemVouchers`
  - transfer NFT to recipient
- `CallbackMsg::Conjunction`
  - appends all to messages (operands callbacks (`CreateVouchers` or `RedeemVouchers` + optional incoming proxy + optional callback))

This guarantees:
- ics721 contract always succeeds
- each sub message handled serateley
- in case of sub msg failure
  - it's partial state is reverted, but not its parent
  - read more here: https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#submessages)
    "... On an error, the subcall will revert any partial state changes due to this message, but not revert any state changes in the calling contract. The error may then be intercepted by the calling contract (for ReplyOn::Always and ReplyOn::Error). In this case, the messages error doesn't abort the whole transaction ..."
  - remaining sub messages are not executed
  - since operands in conjunction sub msg are added as messages (not sub msgs):
    - sub msg is parent of operand messages
    - ics721 is root parent of sub msg
    - if any msg fails (eg. msg1: tranfer NFT1, msg2: tranfer NFT2)
      - all messages and its parent/sub message is reverted
      - root parent/ics721 is not reverted

tl;dr:
- `receive_ibc_packet` response contains single callback sub msg, which:
- in case of failure, revert its own partial state, but wont revert contract's TX
- contains one or more messages

The more I think about it, we can create this result response:
- create directly call back msgs
- hence, no intermediate step of Action enums and Action is required

