use cosmwasm_std::{
    from_json, to_json_binary, Addr, Binary, Deps, DepsMut, Empty, Env, IbcPacket,
    IbcReceiveResponse, StdResult, SubMsg, WasmMsg,
};
use sha2::{Digest, Sha256};
use zip_optional::Zippable;

use crate::{
    helpers::{
        generate_receive_callback_msg, get_incoming_proxy_msg, get_instantiate2_address,
        get_receive_callback,
    },
    ibc::ACK_AND_DO_NOTHING_REPLY_ID,
    ibc_helpers::{get_endpoint_prefix, try_pop_source_prefix},
    msg::{CallbackMsg, ExecuteMsg},
    state::{CLASS_ID_TO_NFT_CONTRACT, CW721_CODE_ID, OUTGOING_CLASS_TOKEN_TO_CHANNEL, PO},
    token_types::{VoucherCreation, VoucherRedemption},
    ContractError,
};
use ics721_types::{
    ibc_types::NonFungibleTokenPacketData,
    token_types::{Class, ClassId, Token, TokenId},
};

pub(crate) fn receive_ibc_packet(
    deps: DepsMut,
    env: Env,
    packet: IbcPacket,
) -> Result<IbcReceiveResponse, ContractError> {
    PO.error_if_paused(deps.storage)?;
    let data: NonFungibleTokenPacketData = from_json(&packet.data)?;
    data.validate()?;

    // Check if NFT is local if not get the local class id
    let maybe_local_class_id = try_pop_source_prefix(&packet.src, &data.class_id);
    let callback = get_receive_callback(&data);
    let local_class_id = if let Some(local_class_id) = maybe_local_class_id {
        ClassId::new(local_class_id)
    } else {
        let local_prefix = get_endpoint_prefix(&packet.dest);
        ClassId::new(format!("{}{}", local_prefix, data.class_id))
    };

    // sub message holds 2 to 4 messages:
    // - one message for voucher creation or redemption, another message for updating incoming or outgoing channel
    let (is_redemption, voucher_and_channel_messages) = create_voucher_and_channel_messages(
        deps.as_ref(),
        env.clone(),
        data.clone(),
        maybe_local_class_id,
        local_class_id.clone(),
        packet.clone(),
    )?;
    // - one optional incoming proxy message
    let incoming_proxy_msg =
        get_incoming_proxy_msg(deps.as_ref().storage, packet.clone(), data.clone())?;
    // - one optional callback message
    let callback_msg = create_callback_msg(
        deps.as_ref(),
        &env,
        &data,
        is_redemption,
        callback,
        local_class_id,
    )?;

    let submessage = into_submessage(
        env.contract.address,
        voucher_and_channel_messages.0,
        voucher_and_channel_messages.1,
        callback_msg,
        incoming_proxy_msg,
    )?;

    let response = if let Some(memo) = data.memo {
        IbcReceiveResponse::default().add_attribute("ics721_memo", memo)
    } else {
        IbcReceiveResponse::default()
    };

    Ok(response
        .add_submessage(submessage)
        .add_attribute("method", "receive_ibc_packet")
        .add_attribute("class_id", data.class_id)
        .add_attribute("local_channel", packet.dest.channel_id)
        .add_attribute("counterparty_channel", packet.src.channel_id))
}

fn create_voucher_and_channel_messages(
    deps: Deps,
    env: Env,
    data: NonFungibleTokenPacketData,
    maybe_local_class_id: Option<&str>,
    local_class_id: ClassId,
    packet: IbcPacket,
) -> Result<(bool, (WasmMsg, WasmMsg)), ContractError> {
    let token_count = data.token_ids.len();
    let redemption_or_create = data
        .token_ids
        .into_iter()
        .zip_optional(data.token_uris)
        .zip_optional(data.token_data)
        .try_fold(
            (
                Vec::<TokenId>::with_capacity(token_count),
                Vec::<Token>::with_capacity(token_count),
            ),
            |mut redemption_or_create, ((token_id, token_uri), token_data)| -> StdResult<_> {
                // If class is not local, its something new
                if maybe_local_class_id.is_some() {
                    let key: (ClassId, TokenId) = (local_class_id.clone(), token_id.clone());
                    let outgoing_channel =
                        OUTGOING_CLASS_TOKEN_TO_CHANNEL.may_load(deps.storage, key.clone())?;

                    // Make sure the channel that used for outgoing transfer, is the same you use to transfer back
                    let returning_to_source = outgoing_channel.map_or(false, |outgoing_channel| {
                        outgoing_channel == packet.dest.channel_id
                    });

                    if returning_to_source {
                        // We previously sent this NFT out on this
                        // channel. Unlock the local version for the
                        // receiver.
                        redemption_or_create.0.push(token_id);
                        return Ok(redemption_or_create);
                    }
                }
                // It's not something we've sent out before => make a
                // new NFT.
                redemption_or_create.1.push(Token {
                    id: token_id,
                    uri: token_uri,
                    data: token_data,
                });
                Ok(redemption_or_create)
            },
        )?;
    let is_redemption = if !redemption_or_create.0.is_empty() && !redemption_or_create.1.is_empty()
    {
        // All token ids in the transfer must be either a redeption or creation
        // they can't be both, if they are both something is wrong.
        return Err(ContractError::InvalidTransferBothActions);
    } else if !redemption_or_create.0.is_empty() {
        true
    } else if !redemption_or_create.1.is_empty() {
        false
    } else {
        // This should never happen, as we must have at least 1 of the above actions
        return Err(ContractError::InvalidTransferNoAction);
    };

    let receiver = deps.api.addr_validate(&data.receiver)?;
    let voucher_and_channel_messages = match is_redemption {
        true => {
            let redemption = VoucherRedemption {
                class: Class {
                    id: local_class_id.clone(),
                    uri: data.class_uri.clone(),
                    data: data.class_data.clone(),
                },
                token_ids: redemption_or_create.0,
            };
            let redeem_outgoing_channels: Vec<(ClassId, TokenId)> = redemption
                .token_ids
                .clone()
                .into_iter()
                .map(|token_id| (local_class_id.clone(), token_id))
                .collect();
            let redeem_outgoing_channels_msg = WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_json_binary(&ExecuteMsg::Callback(
                    CallbackMsg::RedeemOutgoingChannelEntries(redeem_outgoing_channels),
                ))?,
                funds: vec![],
            };
            (
                redemption.into_wasm_msg(env.contract.address.clone(), receiver.to_string())?,
                redeem_outgoing_channels_msg,
            )
        }
        false => {
            let creation = VoucherCreation {
                class: Class {
                    id: local_class_id.clone(),
                    uri: data.class_uri.clone(),
                    data: data.class_data.clone(),
                },
                tokens: redemption_or_create.1,
            };
            let add_incoming_channels: Vec<((ClassId, TokenId), String)> = creation
                .tokens
                .clone()
                .into_iter()
                .map(|token| {
                    (
                        (local_class_id.clone(), token.id),
                        packet.dest.channel_id.clone(),
                    )
                })
                .collect();
            let add_incoming_channels_msg = WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_json_binary(&ExecuteMsg::Callback(
                    CallbackMsg::AddIncomingChannelEntries(add_incoming_channels),
                ))?,
                funds: vec![],
            };
            (
                creation.into_wasm_msg(env.contract.address.clone(), receiver.to_string())?,
                add_incoming_channels_msg,
            )
        }
    };

    Ok((is_redemption, voucher_and_channel_messages))
}

fn create_callback_msg(
    deps: Deps,
    env: &Env,
    data: &NonFungibleTokenPacketData,
    is_redemption: bool,
    callback: Option<(Binary, Option<String>)>,
    local_class_id: ClassId,
) -> Result<Option<WasmMsg>, ContractError> {
    if let Some((receive_callback_data, receive_callback_addr)) = callback {
        // callback require the nft contract, get it using the class id from the action
        let nft_contract = if is_redemption {
            // If its a redemption, it means we already have the contract address in storage

            CLASS_ID_TO_NFT_CONTRACT
                .load(deps.storage, local_class_id.clone())
                .map_err(|_| ContractError::NoNftContractForClassId(local_class_id.to_string()))
        } else {
            // If its a creation action, we can use the instantiate2 function to get the nft contract
            // we don't care of the contract is instantiated yet or not, as later submessage will instantiate it if its not.
            // The reason we use instantiate2 here is because we don't know if it was already instantiated or not.

            let cw721_code_id = CW721_CODE_ID.load(deps.storage)?;
            // for creating a predictable nft contract using, using instantiate2, we need: checksum, creator, and salt:
            // - using class id as salt for instantiating nft contract guarantees a) predictable address and b) uniqueness
            // for this salt must be of length 32 bytes, so we use sha256 to hash class id
            let mut hasher = Sha256::new();
            hasher.update(local_class_id.as_bytes());
            let salt = hasher.finalize().to_vec();

            get_instantiate2_address(deps, env.contract.address.as_str(), &salt, cw721_code_id)
        }?;

        Ok(generate_receive_callback_msg(
            deps,
            data,
            receive_callback_data,
            receive_callback_addr,
            nft_contract.to_string(),
        ))
    } else {
        Ok(None)
    }
}

pub fn into_submessage(
    contract: Addr,
    voucher_message: WasmMsg,
    channel_message: WasmMsg,
    callback_msg: Option<WasmMsg>,
    incoming_proxy_msg: Option<WasmMsg>,
) -> StdResult<SubMsg<Empty>> {
    let mut operands = Vec::with_capacity(4); // 4 is the max number of submessages we can have
    if let Some(incoming_proxy_msg) = incoming_proxy_msg {
        operands.push(incoming_proxy_msg)
    }

    operands.push(voucher_message);

    if let Some(callback_msg) = callback_msg {
        operands.push(callback_msg)
    }

    // once all other submessages are done, we can update incoming or outgoing channel
    operands.push(channel_message);

    let message = WasmMsg::Execute {
        contract_addr: contract.into_string(),
        msg: to_json_binary(&ExecuteMsg::Callback(CallbackMsg::Conjunction { operands }))?,
        funds: vec![],
    };
    Ok(SubMsg::reply_always(message, ACK_AND_DO_NOTHING_REPLY_ID))
}
