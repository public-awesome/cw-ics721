use cosmwasm_std::{
    from_binary, to_binary, Addr, DepsMut, Empty, Env, IbcPacket, IbcReceiveResponse, StdResult,
    SubMsg, WasmMsg,
};

use crate::{
    ibc::{NonFungibleTokenPacketData, ACK_AND_DO_NOTHING},
    ibc_helpers::{get_endpoint_prefix, try_pop_source_prefix},
    msg::{CallbackMsg, ExecuteMsg, NewTokenInfo, TransferInfo},
    state::{INCOMING_CLASS_TOKEN_TO_CHANNEL, OUTGOING_CLASS_TOKEN_TO_CHANNEL, PO},
    ContractError,
};

/// Every incoming token has some associated action.
enum Action {
    /// We have seen this token before, it should be transfered.
    Transfer { class_id: String, token_id: String },
    /// We have not seen this token before, a new one needs to be
    /// created.
    NewToken {
        class_id: String,
        token_id: String,
        token_uri: String,
    },
}

/// Internal type for aggregating actions. Actions can be added via
/// `add_action`. Once aggregation has completed, a
/// `HandlePacketReceive` submessage can be created via the
/// `into_submessage` method.
#[derive(Default)]
struct ActionAggregator {
    pub transfers: Option<TransferInfo>,
    pub new_tokens: Option<NewTokenInfo>,
}

pub(crate) fn do_ibc_packet_receive(
    deps: DepsMut,
    env: Env,
    packet: IbcPacket,
) -> Result<IbcReceiveResponse, ContractError> {
    PO.error_if_paused(deps.storage)?;

    let data: NonFungibleTokenPacketData = from_binary(&packet.data)?;
    data.validate()?;

    let local_class_id = try_pop_source_prefix(&packet.src, &data.class_id);
    let receiver = deps.api.addr_validate(&data.receiver)?;
    let token_count = data.token_ids.len();

    let submessage = data
        .token_ids
        .into_iter()
        .zip(data.token_uris.into_iter())
        .try_fold(
            Vec::<Action>::with_capacity(token_count),
            |mut messages, (token_id, token_uri)| -> StdResult<_> {
                if let Some(local_class_id) = local_class_id {
                    let key = (local_class_id.to_string(), token_id.clone());
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
                            token_id,
                            class_id: local_class_id.to_string(),
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
                    (local_class_id.clone(), token_id.clone()),
                    &packet.dest.channel_id,
                )?;
                messages.push(Action::NewToken {
                    class_id: local_class_id,
                    token_id,
                    token_uri,
                });
                Ok(messages)
            },
        )?
        .into_iter()
        .fold(ActionAggregator::default(), ActionAggregator::add_action)
        .into_submessage(env.contract.address, receiver, data.class_uri)?;

    Ok(IbcReceiveResponse::default()
        .add_submessage(submessage)
        .add_attribute("method", "do_ibc_packet_receive")
        .add_attribute("class_id", data.class_id)
        .add_attribute("local_channel", packet.dest.channel_id)
        .add_attribute("counterparty_channel", packet.src.channel_id))
}

impl ActionAggregator {
    pub fn add_action(mut self, action: Action) -> Self {
        match action {
            Action::Transfer { class_id, token_id } => {
                self.transfers = Some(
                    self.transfers
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
            Action::NewToken {
                class_id,
                token_id,
                token_uri,
            } => {
                self.new_tokens = Some(
                    self.new_tokens
                        .map(|mut info| {
                            info.token_ids.push(token_id.clone());
                            info.token_uris.push(token_uri.clone());
                            info
                        })
                        .unwrap_or_else(|| NewTokenInfo {
                            class_id,
                            token_ids: vec![token_id],
                            token_uris: vec![token_uri],
                        }),
                )
            }
        }
        self
    }

    pub fn into_submessage(
        self,
        contract: Addr,
        receiver: Addr,
        class_uri: Option<String>,
    ) -> StdResult<SubMsg<Empty>> {
        Ok(SubMsg::reply_always(
            WasmMsg::Execute {
                contract_addr: contract.into_string(),
                msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::HandlePacketReceive {
                    class_uri,
                    receiver: receiver.into_string(),
                    transfers: self.transfers,
                    new_tokens: self.new_tokens,
                }))?,
                funds: vec![],
            },
            ACK_AND_DO_NOTHING,
        ))
    }
}

impl TransferInfo {
    pub(crate) fn into_wasm_msg(self, env: &Env, receiver: &Addr) -> StdResult<WasmMsg> {
        Ok(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::BatchTransfer {
                class_id: self.class_id,
                receiver: receiver.to_string(),
                token_ids: self.token_ids,
            }))?,
            funds: vec![],
        })
    }
}

impl NewTokenInfo {
    pub(crate) fn into_wasm_msg(
        self,
        env: &Env,
        receiver: &Addr,
        class_uri: Option<String>,
    ) -> StdResult<WasmMsg> {
        Ok(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::DoInstantiateAndMint {
                class_id: self.class_id,
                class_uri,
                receiver: receiver.to_string(),
                token_ids: self.token_ids,
                token_uris: self.token_uris,
            }))?,
            funds: vec![],
        })
    }
}
