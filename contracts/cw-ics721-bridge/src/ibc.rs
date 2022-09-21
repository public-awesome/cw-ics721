use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, DepsMut, Empty, Env, IbcBasicResponse,
    IbcChannelCloseMsg, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcPacket, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, Reply, Response, StdResult,
    SubMsg, SubMsgResult, WasmMsg,
};
use cw_utils::parse_reply_instantiate_data;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    error::Never,
    helpers::{ACK_AND_DO_NOTHING, INSTANTIATE_CW721_REPLY_ID},
    ibc_helpers::{
        ack_fail, ack_success, get_endpoint_prefix, try_get_ack_error, try_pop_source_prefix,
        validate_order_and_version,
    },
    msg::{CallbackMsg, ExecuteMsg},
    state::{
        CLASS_ID_TO_NFT_CONTRACT, INCOMING_CLASS_TOKEN_TO_CHANNEL, NFT_CONTRACT_TO_CLASS_ID,
        OUTGOING_CLASS_TOKEN_TO_CHANNEL,
    },
    ContractError,
};

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NonFungibleTokenPacketData {
    /// Uniquely identifies the collection which the tokens being
    /// transfered belong to on the sending chain.
    pub class_id: String,
    /// URL that points to metadata about the collection. This is not
    /// validated.
    pub class_uri: Option<String>,
    /// Uniquely identifies the tokens in the NFT collection being
    /// transfered.
    pub token_ids: Vec<String>,
    /// URL that points to metadata for each token being
    /// transfered. `tokenUris[N]` should hold the metadata for
    /// `tokenIds[N]` and both lists should have the same length.
    pub token_uris: Vec<String>,
    /// The address sending the tokens on the sending chain.
    pub sender: String,
    /// The address that should receive the tokens on the receiving
    /// chain.
    pub receiver: String,
}

pub const IBC_VERSION: &str = "ics721-1";
const ACK_ERROR_FALLBACK: &str =
    "an unexpected error occurred - error text is hidden because it would serialize as ACK success";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn ibc_channel_open(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelOpenMsg,
) -> Result<(), ContractError> {
    validate_order_and_version(msg.channel(), msg.counterparty_version())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn ibc_channel_connect(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelConnectMsg,
) -> Result<IbcBasicResponse, ContractError> {
    validate_order_and_version(msg.channel(), msg.counterparty_version())?;

    Ok(IbcBasicResponse::new()
        .add_attribute("method", "ibc_channel_connect")
        .add_attribute("channel", &msg.channel().endpoint.channel_id)
        .add_attribute("port", &msg.channel().endpoint.port_id))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn ibc_channel_close(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelCloseMsg,
) -> Result<IbcBasicResponse, ContractError> {
    match msg {
        IbcChannelCloseMsg::CloseInit { channel: _ } => Err(ContractError::CantCloseChannel {}),
        IbcChannelCloseMsg::CloseConfirm { channel: _ } => {
            // TODO: Is this actually the correct logic? If we do hit
            // this, IBC is telling us "the channel has been closed
            // despite your objection". Will IBC ever tell us this?
            // Should we release NFTs / remove the channel from
            // CHANNELS if this happens?
            unreachable!("channel can not be closed")
        }
        _ => unreachable!("channel can not be closed"),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn ibc_packet_receive(
    deps: DepsMut,
    env: Env,
    msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, Never> {
    // Regardless of if our processing of this packet works we need to
    // commit an ACK to the chain. As such, we wrap all handling logic
    // in a seprate function and on error write out an error ack.
    match do_ibc_packet_receive(deps, env, msg.packet) {
        Ok(response) => Ok(response),
        Err(error) => Ok(IbcReceiveResponse::new()
            .add_attribute("method", "ibc_packet_receive")
            .add_attribute("error", error.to_string())
            .set_ack(ack_fail(&error.to_string()).unwrap())),
    }
}

fn do_ibc_packet_receive(
    deps: DepsMut,
    env: Env,
    packet: IbcPacket,
) -> Result<IbcReceiveResponse, ContractError> {
    /// Every incoming token has some associated action.
    enum Action {
        /// We have seen this token before, it should be transfered.
        Transfer { class_id: String, token_id: String },
        /// We have not seen this token before, a new one needs to be
        /// created.
        InstantiateAndMint {
            class_id: String,
            token_id: String,
            token_uri: String,
        },
    }

    /// Used to aggregate Action::Transfer actions.
    struct TransferInfo {
        pub class_id: String,
        pub token_ids: Vec<String>,
    }
    /// Used to aggregate Action::InstantiateAndMint actions.
    struct InstantiateAndMintInfo {
        pub class_id: String,
        pub token_ids: Vec<String>,
        pub token_uris: Vec<String>,
    }
    /// Tracks what needs to be done in response to an incoming IBC
    /// message.
    struct WhatToDo {
        pub transfer: Option<TransferInfo>,
        pub iandm: Option<InstantiateAndMintInfo>,
    }

    impl WhatToDo {
        pub fn add_action(mut self, action: Action) -> Self {
            match action {
                Action::Transfer { class_id, token_id } => {
                    self.transfer = Some(
                        self.transfer
                            .map(|mut info| {
                                info.token_ids.push(token_id.clone());
                                info
                            })
                            .unwrap_or_else(|| TransferInfo {
                                class_id,
                                token_ids: vec![token_id],
                            }),
                    )
                }
                Action::InstantiateAndMint {
                    class_id,
                    token_id,
                    token_uri,
                } => {
                    self.iandm = Some(
                        self.iandm
                            .map(|mut info| {
                                info.token_ids.push(token_id.clone());
                                info.token_uris.push(token_uri.clone());
                                info
                            })
                            .unwrap_or_else(|| InstantiateAndMintInfo {
                                class_id,
                                token_ids: vec![token_id],
                                token_uris: vec![token_uri],
                            }),
                    )
                }
            }
            self
        }

        pub fn into_submessages(
            self,
            contract: Addr,
            receiver: Addr,
            class_uri: Option<String>,
        ) -> StdResult<Vec<SubMsg<Empty>>> {
            let mut messages = Vec::with_capacity(2);
            if let Some(TransferInfo {
                class_id,
                token_ids,
            }) = self.transfer
            {
                messages.push(SubMsg::reply_always(
                    WasmMsg::Execute {
                        contract_addr: contract.to_string(),
                        msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::BatchTransfer {
                            class_id,
                            receiver: receiver.to_string(),
                            token_ids,
                        }))?,
                        funds: vec![],
                    },
                    ACK_AND_DO_NOTHING,
                ));
            }
            if let Some(InstantiateAndMintInfo {
                class_id,
                token_ids,
                token_uris,
            }) = self.iandm
            {
                messages.push(SubMsg::reply_always(
                    WasmMsg::Execute {
                        contract_addr: contract.into_string(),
                        msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::DoInstantiateAndMint {
                            class_id,
                            class_uri,
                            token_ids,
                            token_uris,
                            receiver: receiver.into_string(),
                        }))?,
                        funds: vec![],
                    },
                    ACK_AND_DO_NOTHING,
                ))
            }
            Ok(messages)
        }
    }

    let data: NonFungibleTokenPacketData = from_binary(&packet.data)?;
    data.validate()?;

    let local_class_id = try_pop_source_prefix(&packet.src, &data.class_id);
    let receiver = deps.api.addr_validate(&data.receiver)?;
    let token_count = data.token_ids.len();

    // Say we're connected to three identical chains A, B, and C. For
    // each of these chains call the local channel ID `C` and the
    // local port `P`. After taking the path (A -> B -> C) the class
    // ID on C is `P/C/P/C/P/C/contract_address`.
    //
    // Now, lets say the next hop we take is from (C -> A). A receives
    // a packet with prefix `P/C`. According to the logic on the spec,
    // it would recognize this as its prefix and attempt to release
    // its local version of the NFT (from the prefix alone, it seems
    // like this has previously been transfered away!). This attempt
    // to release fails though as there has never been a transfer from
    // (A -> C)!
    //
    // What do we need to do instead? Before attempting the transfer,
    // we need to verify that the incoming NFT has previously been
    // transfered out. If it has not, we should not attempt the
    // transfer and instead (correctly) treat it as a new NFT that we
    // have not seen before and create a new local cw721 contract.
    let messages = data
        .token_ids
        .into_iter()
        .zip(data.token_uris.into_iter())
        .try_fold(
            Vec::<Action>::with_capacity(token_count),
            |mut messages, (token, token_uri)| -> StdResult<_> {
                if let Some(local_class_id) = local_class_id {
                    let key = (local_class_id.to_string(), token.clone());
                    let outgoing_channel =
                        OUTGOING_CLASS_TOKEN_TO_CHANNEL.may_load(deps.storage, key.clone())?;
                    let returning_to_source = outgoing_channel.map_or(false, |outgoing_channel| {
                        outgoing_channel == packet.dest.channel_id
                    });
                    if returning_to_source {
                        // We previously sent this NFT out on this
                        // channel. Unlock the local version for the
                        // receiver.
                        OUTGOING_CLASS_TOKEN_TO_CHANNEL.remove(deps.storage, key);
                        messages.push(Action::Transfer {
                            class_id: local_class_id.to_string(),
                            token_id: token,
                        });
                        return Ok(messages);
                    }
                }
                // It's not something we've sent out before => make a
                // new NFT.
                let local_prefix = get_endpoint_prefix(&packet.dest);
                let local_class_id = format!("{}{}", local_prefix, data.class_id);
                INCOMING_CLASS_TOKEN_TO_CHANNEL.save(
                    deps.storage,
                    (local_class_id.clone(), token.clone()),
                    &packet.dest.channel_id,
                )?;
                messages.push(Action::InstantiateAndMint {
                    class_id: local_class_id,
                    token_id: token,
                    token_uri,
                });
                Ok(messages)
            },
        )?
        .into_iter()
        .fold(
            WhatToDo {
                transfer: None,
                iandm: None,
            },
            WhatToDo::add_action,
        )
        .into_submessages(env.contract.address, receiver, data.class_uri)?;

    // FIXME(ekez): these submessages need to be merged into a single
    // submessage so we don't override ACKs.
    Ok(IbcReceiveResponse::default().add_submessages(messages))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn ibc_packet_ack(
    deps: DepsMut,
    _env: Env,
    ack: IbcPacketAckMsg,
) -> Result<IbcBasicResponse, ContractError> {
    if let Some(error) = try_get_ack_error(&ack.acknowledgement)? {
        handle_packet_fail(deps, ack.original_packet, &error)
    } else {
        let msg: NonFungibleTokenPacketData = from_binary(&ack.original_packet.data)?;

        let nft_contract = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, msg.class_id.clone())?;
        // Burn all of the tokens being transfered out that were
        // previously transfered in on this channel.
        let burn_notices = msg.token_ids.iter().cloned().try_fold(
            Vec::<WasmMsg>::new(),
            |mut messages, token| -> StdResult<_> {
                let key = (msg.class_id.clone(), token.clone());
                let source_channel =
                    INCOMING_CLASS_TOKEN_TO_CHANNEL.may_load(deps.storage, key.clone())?;
                let returning_to_source = source_channel.map_or(false, |source_channel| {
                    source_channel == ack.original_packet.src.channel_id
                });
                if returning_to_source {
                    // This token's journey is complete, for now.
                    INCOMING_CLASS_TOKEN_TO_CHANNEL.remove(deps.storage, key);
                    messages.push(WasmMsg::Execute {
                        contract_addr: nft_contract.to_string(),
                        msg: to_binary(&cw721::Cw721ExecuteMsg::Burn { token_id: token })?,
                        funds: vec![],
                    })
                }
                Ok(messages)
            },
        )?;

        Ok(IbcBasicResponse::new()
            .add_messages(burn_notices)
            .add_attribute("method", "acknowledge")
            .add_attribute("sender", msg.sender)
            .add_attribute("receiver", msg.receiver)
            .add_attribute("classId", msg.class_id)
            .add_attribute("token_ids", format!("{:?}", msg.token_ids)))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn ibc_packet_timeout(
    deps: DepsMut,
    _env: Env,
    msg: IbcPacketTimeoutMsg,
) -> Result<IbcBasicResponse, ContractError> {
    handle_packet_fail(deps, msg.packet, "timeout")
}

fn handle_packet_fail(
    deps: DepsMut,
    packet: IbcPacket,
    error: &str,
) -> Result<IbcBasicResponse, ContractError> {
    // Return to sender!
    let message: NonFungibleTokenPacketData = from_binary(&packet.data)?;
    let nft_address = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, message.class_id.clone())?;
    let sender = deps.api.addr_validate(&message.sender)?;

    let messages = message
        .token_ids
        .iter()
        .cloned()
        .map(|token_id| -> StdResult<_> {
            OUTGOING_CLASS_TOKEN_TO_CHANNEL
                .remove(deps.storage, (message.class_id.clone(), token_id.clone()));
            Ok(WasmMsg::Execute {
                contract_addr: nft_address.to_string(),
                msg: to_binary(&cw721::Cw721ExecuteMsg::TransferNft {
                    recipient: sender.to_string(),
                    token_id,
                })?,
                funds: vec![],
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(IbcBasicResponse::new()
        .add_messages(messages)
        .add_attribute("method", "handle_packet_fail")
        .add_attribute("token_ids", format!("{:?}", message.token_ids))
        .add_attribute("class_id", message.class_id)
        .add_attribute("channel_id", packet.src.channel_id)
        .add_attribute("address_refunded", message.sender)
        .add_attribute("error", error))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        INSTANTIATE_CW721_REPLY_ID => {
            // Don't need to add an ack or check for an error here as this
            // is only replies on success. This is OK because it is only
            // ever used in `DoInstantiateAndMint` which itself is always
            // a submessage of `ibc_packet_receive` which is caught and
            // handled correctly by the reply handler for
            // `INSTANTIATE_AND_MINT_CW721_REPLY_ID`.

            let res = parse_reply_instantiate_data(reply)?;
            let cw721_addr = deps.api.addr_validate(&res.contract_address)?;

            // We need to map this address back to a class
            // ID. Fourtunately, we set the name of the new NFT
            // contract to the class ID.
            let cw721::ContractInfoResponse { name: class_id, .. } = deps
                .querier
                .query_wasm_smart(cw721_addr.clone(), &cw721::Cw721QueryMsg::ContractInfo {})?;

            // Save classId <-> contract mappings.
            CLASS_ID_TO_NFT_CONTRACT.save(deps.storage, class_id.clone(), &cw721_addr)?;
            NFT_CONTRACT_TO_CLASS_ID.save(deps.storage, cw721_addr.clone(), &class_id)?;

            Ok(Response::default()
                .add_attribute("method", "instantiate_cw721_reply")
                .add_attribute("class_id", class_id)
                .add_attribute("cw721_addr", cw721_addr))
        }
        // These messages don't need to do any state changes in the
        // reply - just need to commit an ack.
        ACK_AND_DO_NOTHING => {
            match reply.result {
                // On success, set a successful ack. Nothing else to do.
                SubMsgResult::Ok(_) => Ok(Response::new().set_data(ack_success())),
                // On error we need to use set_data to override the data field
                // from our caller, the IBC packet recv, and acknowledge our
                // failure.  As per:
                // https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#handling-the-reply
                SubMsgResult::Err(err) => Ok(Response::new().set_data(
                    ack_fail(&err).unwrap_or_else(|_e| ack_fail(ACK_ERROR_FALLBACK).unwrap()),
                )),
            }
        }
        _ => Err(ContractError::UnrecognisedReplyId {}),
    }
}
