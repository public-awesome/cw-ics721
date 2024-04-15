# CW-ICS721

This is an implementation of the [ICS 721 specification](https://github.com/cosmos/ibc/tree/master/spec/app/ics-721-nft-transfer) written in CosmWasm. It allows NFTs to be moved between IBC compatible blockchains.

This implementation

1. is entirely compatible with the cw721 NFT standard, the standard used by most NFT marketplaces in the IBC ecosystem;
2. has a minimal, but powerful governance system that can quickly pause the system in an emergency, without ceding any of the governance module's control over the ICS721 contract;
3. supports a proxy system that allows for arbitrary filtering and rate limiting of outgoing NFTs;
4. is well tested.

To enable ICS721 contracts to function correctly, the app chain needs to have at least `wasmd v0.31.0` installed, with the `cosmwasm_1_2` feature enabled. This requirement arises from the fact that the ICS721 contract uses `instantiate2` for creating predicted cw721 addresses. For more detailed information, please refer to the [CHANGELOG.md](https://github.com/CosmWasm/wasmd/blob/main/CHANGELOG.md#v0310-2023-03-13) in the `wasmd` repository.

## Getting Started

Follow these steps to set up contracts and channels:

1. Clone the [`cw-ics721`](https://github.com/public-awesome/cw-ics721) repository.
2. Build the contracts using the [`ts-relayer-tests/build.sh`](https://github.com/public-awesome/cw-ics721/blob/main/ts-relayer-tests/build.sh) script.
3. Upload and instantiate the `ics721-base` contract (refer to the [CosmWasm book](https://book.cosmwasm.com/) for details) on at least 2 CosmWasm-based app chains.
4. Set up relayers, such as [Cosmos/IBC Go](https://github.com/cosmos/relayer/) or [Hermes](https://hermes.informal.systems/).

To gain a better understanding of how ICS721 (interchain) workflows function, consider running the integration tests. You can find more information in the [ts-relayer-tests/README.md](./ts-relayer-tests/README.md) file. The integration tests perform the following actions:

- Set up 2 local chains.
- Upload interchain contracts.
- Create an IBC channel between both contracts.
- Create a collection contract (cw721).
- Mint an NFT.
- Transfer the NFT from one chain to another.

## From a thousand feet up

This contract deals in debt-vouchers.

![debt-vouchers](https://user-images.githubusercontent.com/30676292/210026430-ab673969-23b7-4ffd-964c-d22453e5adeb.png)

To send a NFT from chain A to chain B:

1. The NFT is locked on chain A.
2. A message is delivered over IBC to the destination chain describing the NFT that has been locked.
3. A debt-voucher, which is conveniently an exact replica of the NFT locked on chain A, is minted on chain B.

The duplicate NFT on the receiving chain is a debt-voucher. Possession of that debt-voucher on the receiving chain gives the holder the right to redeem it for the original NFT on chain A.

To return the transferred NFT:

1. The debt-voucher is returned to the ICS721 contract.
2. A message is sent to the source chain informing it that the debt voucher has been returned.
3. The original NFT is unlocked and sent to the receiver of the NFT.
4. The debt-voucher is burned on chain B.

The failure handling logic for this contract is also reasonably simple to explain: if the receiver does not process the packet correctly, the NFT sent to the ICS721 contract is returned to the sender as if the transfer had never happened.

## From closer to the ground

The complete process for an ICS-721 NFT transfer is described in this flowchart:

![ics721-flowchart](https://user-images.githubusercontent.com/30676292/195717720-8d0629c1-dcdb-4f99-8ffd-b828dc1a216d.png)

## Quick pauses and filtering

This implementation can be quickly paused by a subDAO and supports rich filtering and rate limiting for the NFTs allowed to traverse it.

Pause functionality is designed to allow for quick pauses by a trusted group, without conceding the ability to lock the contract to that group. To this end, the admin of this contract may appoint a subDAO which may pause the contract a _single time_. In pausing the contract, the subDAO loses the ability to pause again until it is reauthorized by governance.

After a pause, the ICS721 contract will remain paused until governance chooses to unpause it. During the unpause process governance may appoint a new subDAO or reappoint the existing one as pause manager. It is imagined that the admin of this contract will be a chain's community pool, and the pause manager will be a small, active subDAO. This process means that the subDAO may pause the contract in the event of a problem, but may not lock the contract, as in pausing the contract the subDAO burns its ability to do so again.

Filtering is enabled by an optional proxy that the ICS721 contract may be configured to use. If a proxy is configured, the ICS721 contract will only accept NFTs delivered by the proxy address. This proxy interface is very minimal and enables very flexible rate limiting and filtering. Currently, per-collection rate limiting is implemented. Users of this ICS721 contract are encouraged to implement their own filtering regimes and may add them to the [proxy repository](https://github.com/arkprotocol/cw-ics721-proxy) so that others may use them.

## Failure handling errata

This contract will never close an IBC channel between itself and another ICS721 contract or module. If the other side of a channel closes the connection, the ICS721 contract assumes this has happened due to a catastrophic bug in its counterparty or a malicious action. As such, if a channel closes NFTs will not be removable from it until governance intervention sets the policy for what to do.

Depending on what kind of filtering is applied to this contract, permissionless chains where anyone can instantiate a NFT contract may allow the transfer of a buggy cw721 implementation that causes transfers to fail.

These sorts of issues can cause trouble with relayer implementations. The inability to collect fees for relaying is a limitation of the IBC protocol and this ICS721 contract can not hope to address that. To this end, it is strongly recommended that users of this ICS721 contract and all other IBC bridges have users [relay their own packets](https://github.com/DA0-DA0/dao-dao-ui/issues/885). We will be working on an implementation of this that other front ends can easily integrate as part of this work.

## Callbacks

cw-ics721 supports [callbacks](./packages/ics721-types/src/types.rs#L67-L70) for Ics721ReceiveCallback and Ics721AckCallback.

1. Receive callback - Callback that is being called on the receiving chain when the NFT was succesfully transferred.
2. Ack callback - Callback that is being called on the sending chain notifying about the status of the transfer.

Workflow:

1. `send_nft` from cw721 -> cw-ics721.
2. `send_nft` holds `IbcOutgoingMsg` msg.
3. `IbcOutgoingMsg` holds `Ics721Memo` with optional receive (request) and ack (response) callbacks.
4. `cw-ics721` on target chain executes optional receive callback.
5. `cw-ics721` sends ack success or ack error to `cw-ics721` on source chain.
6. `cw-ics721` on source chain executes optional ack callback.

NOTES:

In case of 4. if any error occurs on target chain, NFT gets rolled back and return to sender on source chain.
In case of 6. ack callback also holds `Ics721Status::Success` or `Ics721Status::Failed(String)`

### Callback Execution

Callbacks are optional and can be added in the memo field of the transfer message:

```json
{
  "callbacks": {
    "ack_callback_data": "custom data to pass with the callback",
    "ack_callback_addr": "cosmos1...",
    "receive_callback_data": "custom data to pass with the callback",
    "receive_callback_addr": "cosmos1..."
  }
}
```

An [Ics721Memo](./packages/ics721-types/src/types.rs#L11-L30) may be provided as part of [IbcOutgoingMsg](./packages/ics721-types/src/ibc_types.rs#L99):

```rust
// -- ibc_types.rs
#[cw_serde]
pub struct IbcOutgoingMsg {
    /// The address that should receive the NFT being sent on the
    /// *receiving chain*.
    pub receiver: String,
    /// The *local* channel ID this ought to be sent away on. This
    /// contract must have a connection on this channel.
    pub channel_id: String,
    /// Timeout for the IBC message.
    pub timeout: IbcTimeout,
    /// Memo to add custom string to the msg
    pub memo: Option<String>,
}

// -- types.rs
pub struct Ics721Memo {
    pub callbacks: Option<Ics721Callbacks>,
}

/// The format we expect for the memo field on a send
#[cw_serde]
pub struct Ics721Callbacks {
    /// Data to pass with a callback on source side (status update)
    /// Note - If this field is empty, no callback will be sent
    pub ack_callback_data: Option<Binary>,
    /// The address that will receive the callback message
    /// Defaults to the sender address
    pub ack_callback_addr: Option<String>,
    /// Data to pass with a callback on the destination side (ReceiveNftIcs721)
    /// Note - If this field is empty, no callback will be sent
    pub receive_callback_data: Option<Binary>,
    /// The address that will receive the callback message
    /// Defaults to the receiver address
    pub receive_callback_addr: Option<String>,
}

```

In order to execute an ack callback, `ack_callback_data` must not be empty. In order to execute a receive callback, `receive_callback_data` must not be empty.

A contract sending an NFT with callback may look like this:

```rust
let callback_msg = MyAckCallbackMsgData {
  // ... any arbitrary data contract wants to
};
let mut callbacks = Ics721Callbacks {
    ack_callback_data: Some(to_json_binary(&callback_msg)?),
    ack_callback_addr: None, // in case of none ics721 uses recipient (default) as callback addr
    receive_callback_data: None,
    receive_callback_addr: None,
};
if let Some(counterparty_contract) = COUNTERPARTY_CONTRACT.may_load(deps.storage)? {
    callbacks.receive_callback_data = Some(to_json_binary(&callback_msg)?);
    callbacks.receive_callback_addr = Some(counterparty_contract); // here we need to set contract addr, since receiver is NFT receiver
}
let memo = Ics721Memo {
    callbacks: Some(callbacks),
};
let ibc_msg = IbcOutgoingMsg {
    receiver,
    channel_id,
    timeout: IbcTimeout::with_timestamp(env.block.time.plus_minutes(30)),
    memo: Some(Binary::to_base64(&to_json_binary(&memo)?)),
};
// send nft to ics721 (or outgoing proxy if set by ics721)
let send_nft_msg = Cw721ExecuteMsg::SendNft {
    contract: 'ADDR_ICS721_OUTGOING_PROXY'.to_string(),
    token_id: token_id.to_string(),
    msg: to_json_binary(&ibc_msg)?,
};
let send_nft_sub_msg = SubMsg::<Empty>::reply_on_success(
    WasmMsg::Execute {
        contract_addr: CW721_ADDR.load(storage)?.to_string(),
        msg: to_json_binary(&send_nft_msg)?,
        funds: vec![],
    },
    REPLY_NOOP,
);
```

### Contract to accept callbacks

In order for a contract to accept callbacks, it must implement the next messages:

```rust
pub enum ReceiverExecuteMsg {
    Ics721ReceiveCallback(Ics721ReceiveCallbackMsg),
    Ics721AckCallback(Ics721AckCallbackMsg),
}
```

`Ics721ReceiveCallback` is used for receive callbacks and gets the next data:

```rust
pub struct Ics721ReceiveCallbackMsg {
    /// The nft contract address that received the NFT
    pub nft_contract: String,
    /// The original packet that was sent
    pub original_packet: NonFungibleTokenPacketData,
    /// The provided custom msg by the sender
    pub msg: Binary,
}
```

`Ics721AckCallback` - is used for ack callback and gets the next data:

```rust
pub struct Ics721AckCallbackMsg {
    /// The status of the transfer (succeeded or failed)
    pub status: Ics721Status,
    /// The nft contract address that sent the NFT
    pub nft_contract: String,
    /// The original packet that was sent
    pub original_packet: NonFungibleTokenPacketData,
    /// The provided custom msg by the sender
    pub msg: Binary,
}
```

**IMPORTANT** - Those messages are permission-less and can be called by anyone with any data. It is the responsibility of the contract to validate the sender and make sure the sender is a trusted ICS721 contract.
Its also a good practice to confirm the owner of the transferred NFT by querying the nft contract.
