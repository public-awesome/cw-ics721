# CW-ICS721

This is an implementation of the [ICS 721
specification](https://github.com/cosmos/ibc/tree/master/spec/app/ics-721-nft-transfer)
written in CosmWasm. It allows NFTs to be moved between IBC compatible
blockchains.

This implementation

1. is entirely compatible with the cw721 NFT standard, the standard
   used by most NFT marketplaces in the IBC ecosystem;
2. has a minimal, but powerful governance system that can quickly
   pause the system in an emergency, without ceding any of the
   governance module's control over the ICS721 contract;
3. supports a proxy system that allows for arbitrary filtering and
   rate limiting of outgoing NFTs;
4. is well tested.

## From a thousand feet up

This contract deals in debt-vouchers.

![debt-vouchers](https://user-images.githubusercontent.com/30676292/210026430-ab673969-23b7-4ffd-964c-d22453e5adeb.png)

To send a NFT from chain A to chain B:

1. The NFT is locked on chain A.
2. A message is delivered over IBC to the destination chain describing
   the NFT that has been locked.
3. A debt-voucher, which is conveniently an exact replica of the NFT
   locked on chain A, is minted on chain B.

The duplicate NFT on the receiving chain is a debt-voucher. Possession
of that debt-voucher on the receiving chain gives the holder the right
to redeem it for the original NFT on chain A.

To return the transferred NFT:

1. The debt-voucher is returned to the ICS721 contract.
2. A message is sent to the source chain informing it that the debt
   voucher has been returned.
3. The original NFT is unlocked and sent to the receiver of the NFT.
4. The debt-voucher is burned on chain B.

The failure handling logic for this contract is also reasonably simple
to explain: if the receiver does not process the packet correctly, the
NFT sent to the ICS721 contract is returned to the sender as if the transfer
had never happened.

## From closer to the ground

The complete process for an ICS-721 NFT transfer is described in this
flowchart:

![ics721-flowchart](https://user-images.githubusercontent.com/30676292/195717720-8d0629c1-dcdb-4f99-8ffd-b828dc1a216d.png)

## Quick pauses and filtering

This implementation can be quickly paused by a subDAO and supports
rich filtering and rate limiting for the NFTs allowed to traverse it.

Pause functionality is designed to allow for quick pauses by a trusted
group, without conceding the ability to lock the contract to that
group. To this end, the admin of this contract may appoint a subDAO
which may pause the contract a _single time_. In pausing the contract,
the subDAO loses the ability to pause again until it is reauthorized
by governance.

After a pause, the ICS721 contract will remain paused until governance chooses
to unpause it. During the unpause process governance may appoint a new
subDAO or reappoint the existing one as pause manager. It is imagined
that the admin of this contract will be a chain's community pool, and
the pause manager will be a small, active subDAO. This process means
that the subDAO may pause the contract in the event of a problem, but
may not lock the contract, as in pausing the contract the subDAO burns
its ability to do so again.

Filtering is enabled by an optional proxy that the ICS721 contract may be
configured to use. If a proxy is configured, the ICS721 contract will only
accept NFTs delivered by the proxy address. This proxy interface is
very minimal and enables very flexible rate limiting and
filtering. Currently, per-collection rate limiting is
implemented. Users of this ICS721 contract are encouraged to implement their
own filtering regimes and may add them to the [proxy
repository](https://github.com/arkprotocol/cw721-proxy) so that others may
use them.

## Failure handling errata

This contract will never close an IBC channel between itself and
another ICS721 contract or module. If the other side of a channel closes the connection,
the ICS721 contract assumes this has happened due to a catastrophic bug in its
counterparty or a malicious action. As such, if a channel closes NFTs
will not be removable from it until governance intervention sets the
policy for what to do.

Depending on what kind of filtering is applied to this contract,
permissionless chains where anyone can instantiate a NFT contract may
allow the transfer of a buggy cw721 implementation that causes
transfers to fail.

These sorts of issues can cause trouble with relayer
implementations. The inability to collect fees for relaying is a
limitation of the IBC protocol and this ICS721 contract can not hope
to address that. To this end, it is strongly recommended that users of
this ICS721 contract and all other IBC bridges have users [relay their own
packets](https://github.com/DA0-DA0/dao-dao-ui/issues/885). We will be
working on an implementation of this that other front ends can easily
integrate as part of this work.
