use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, DepsMut, Empty, Env, IbcPacket, IbcReceiveResponse,
    StdResult, SubMsg, WasmMsg,
};
use zip_optional::Zippable;

use crate::{
    ibc::{NonFungibleTokenPacketData, ACK_AND_DO_NOTHING},
    ibc_helpers::{get_endpoint_prefix, try_pop_source_prefix},
    msg::{CallbackMsg, ExecuteMsg},
    state::{INCOMING_CLASS_TOKEN_TO_CHANNEL, OUTGOING_CLASS_TOKEN_TO_CHANNEL, PO},
    token_types::{Class, ClassId, Token, TokenId, VoucherCreation, VoucherRedemption},
    ContractError,
};

/// Every incoming token has some associated action.
enum Action {
    /// Debt-voucher redemption.
    Redemption {
        class_id: ClassId,
        token_id: TokenId,
    },
    /// Debt-voucher creation.
    Creation { class_id: ClassId, token: Token },
}

/// Internal type for aggregating actions. Actions can be added via
/// `add_action`. Once aggregation has completed, a
/// `HandlePacketReceive` submessage can be created via the
/// `into_submessage` method.
///
/// Unlike `class_id`, class data and uri will always be the same
/// across one transfer so we store only one copy at the top level and
/// initialize it at creation time.
#[derive(Default)]
struct ActionAggregator {
    class_uri: Option<String>,
    class_data: Option<Binary>,

    redemption: Option<VoucherRedemption>,
    creation: Option<VoucherCreation>,
}

pub(crate) fn receive_ibc_packet(
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
        .zip_optional(data.token_uris)
        .zip_optional(data.token_data)
        .try_fold(
            Vec::<Action>::with_capacity(token_count),
            |mut messages, ((token_id, token_uri), token_data)| -> StdResult<_> {
                if let Some(local_class_id) = local_class_id {
                    let local_class_id = ClassId::new(local_class_id);
                    let key = (local_class_id.clone(), token_id.clone());
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
                        messages.push(Action::Redemption {
                            token_id,
                            class_id: local_class_id,
                        });
                        return Ok(messages);
                    }
                }
                // It's not something we've sent out before => make a
                // new NFT.
                let local_prefix = get_endpoint_prefix(&packet.dest);
                let local_class_id = ClassId::new(format!("{}{}", local_prefix, data.class_id));
                INCOMING_CLASS_TOKEN_TO_CHANNEL.save(
                    deps.storage,
                    (local_class_id.clone(), token_id.clone()),
                    &packet.dest.channel_id,
                )?;
                messages.push(Action::Creation {
                    class_id: local_class_id,
                    token: Token {
                        id: token_id,
                        uri: token_uri,
                        data: token_data,
                    },
                });
                Ok(messages)
            },
        )?
        .into_iter()
        .fold(
            ActionAggregator::new(data.class_uri, data.class_data),
            ActionAggregator::add_action,
        )
        .into_submessage(env.contract.address, receiver)?;

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

impl ActionAggregator {
    pub fn new(class_uri: Option<String>, class_data: Option<Binary>) -> Self {
        Self {
            class_uri,
            class_data,
            redemption: None,
            creation: None,
        }
    }

    // the ics-721 rx logic is a functional implementation of this
    // imperative pseudocode:
    //
    // ```
    // def select_actions(class_id, token, ibc_channel):
    //     (local_class_id, could_be_local) = pop_src_prefix(class_id)
    //     actions = []
    //
    //     for token in tokens:
    //         if could_be_local:
    //             returning_to_source = outgoing_tokens.has(token)
    //             if returning_to_source:
    //                 outgoing_tokens.remove(token)
    //                 actions.push(redeem_voucher, token, local_class_id)
    //                 continue
    //         incoming_tokens.save(token)
    //         prefixed_class_id = prefix(class_id, ibc_channel)
    //         actions.push(create_voucher, token, prefixed_class_id)
    //
    //     return actions
    // ```
    //
    // as `class_id` is fixed:
    //
    // 1. all `create_voucher` actions will have class id
    //    `prefixed_class_id`
    // 2. all `redeem_voucher` actions will have class id
    //    `local_class_id`
    //
    // in other words:
    //
    // 1. `create_voucher` actions will all have the same `class_id`
    // 2. `redeem_voucher` actions will all have the same `class_id`
    //
    // we make use of these properties here in that we only store one
    // copy of class information per voucher action.
    //
    // ---
    //
    // tangental but nonetheless important aside:
    //
    // 3. not all create and redeem actions will have the same
    //    `class_id`.
    //
    // by counterexample: two identical tokens are sent by a malicious
    // counterparty, the first removes the token from the
    // outgoing_tokens map, the second then creates a create_voucher
    // action.
    //
    // see `TestDoubleSendInSingleMessage` in `/e2e/adversarial_test.go`
    // for a test demonstrating this.
    pub fn add_action(mut self, action: Action) -> Self {
        match action {
            Action::Redemption { class_id, token_id } => {
                self.redemption = match self.redemption {
                    Some(mut r) => {
                        r.token_ids.push(token_id);
                        Some(r)
                    }
                    None => Some(VoucherRedemption {
                        class: Class {
                            id: class_id,
                            uri: self.class_uri.clone(),
                            data: self.class_data.clone(),
                        },
                        token_ids: vec![token_id],
                    }),
                }
            }
            Action::Creation { class_id, token } => {
                self.creation = match self.creation {
                    Some(mut c) => {
                        c.tokens.push(token);
                        Some(c)
                    }
                    None => Some(VoucherCreation {
                        class: Class {
                            id: class_id,
                            uri: self.class_uri.clone(),
                            data: self.class_data.clone(),
                        },
                        tokens: vec![token],
                    }),
                }
            }
        };
        self
    }

    pub fn into_submessage(self, contract: Addr, receiver: Addr) -> StdResult<SubMsg<Empty>> {
        let mut m = Vec::with_capacity(2);
        if let Some(redeem) = self.redemption {
            m.push(redeem.into_wasm_msg(contract.clone(), receiver.to_string())?)
        }
        if let Some(create) = self.creation {
            m.push(create.into_wasm_msg(contract.clone(), receiver.into_string())?)
        }
        let message = if m.len() == 1 {
            m[0].clone()
        } else {
            WasmMsg::Execute {
                contract_addr: contract.into_string(),
                msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::Conjunction {
                    operands: m,
                }))?,
                funds: vec![],
            }
        };
        Ok(SubMsg::reply_always(message, ACK_AND_DO_NOTHING))
    }
}
