use cosmwasm_std::{
    from_json, to_json_binary, Addr, Binary, DepsMut, Empty, Env, IbcPacket, IbcReceiveResponse,
    StdResult, SubMsg, WasmMsg,
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
    state::{
        CLASS_ID_TO_NFT_CONTRACT, CW721_CODE_ID, INCOMING_CLASS_TOKEN_TO_CHANNEL,
        OUTGOING_CLASS_TOKEN_TO_CHANNEL, PO,
    },
    token_types::{VoucherCreation, VoucherRedemption},
    ContractError,
};
use ics721_types::{
    ibc_types::NonFungibleTokenPacketData,
    token_types::{Class, ClassId, Token, TokenId},
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
    let data: NonFungibleTokenPacketData = from_json(&packet.data)?;
    data.validate()?;

    let cloned_data = data.clone();
    let receiver = deps.api.addr_validate(&data.receiver)?;
    let token_count = data.token_ids.len();

    // Check if NFT is local if not get the local class id
    let maybe_local_class_id = try_pop_source_prefix(&packet.src, &data.class_id);
    let callback = get_receive_callback(&data);

    let action_aggregator = data
        .token_ids
        .into_iter()
        .zip_optional(data.token_uris)
        .zip_optional(data.token_data)
        .try_fold(
            Vec::<Action>::with_capacity(token_count),
            |mut messages, ((token_id, token_uri), token_data)| -> StdResult<_> {
                // If class is not local, its something new
                if let Some(local_class_id) = maybe_local_class_id {
                    let local_class_id = ClassId::new(local_class_id);
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
        );

    // All token ids in the transfer must be either a redeption or creation
    // they can't be both, if they are both something is wrong.
    if action_aggregator.redemption.is_some() && action_aggregator.creation.is_some() {
        return Err(ContractError::InvalidTransferBothActions);
    }

    // if there is a callback, generate the callback message
    let callback_msg = if let Some((receive_callback_data, receive_callback_addr)) = callback {
        // callback require the nft contract, get it using the class id from the action
        let nft_contract = if let Some(voucher) = action_aggregator.redemption.clone() {
            // If its a redemption, it means we already have the contract address in storage

            CLASS_ID_TO_NFT_CONTRACT
                .load(deps.storage, voucher.class.id.clone())
                .map_err(|_| ContractError::NoNftContractForClassId(voucher.class.id.to_string()))
        } else if let Some(voucher) = action_aggregator.creation.clone() {
            // If its a creation action, we can use the instantiate2 function to get the nft contract
            // we don't care of the contract is instantiated yet or not, as later submessage will instantiate it if its not.
            // The reason we use instantiate2 here is because we don't know if it was already instantiated or not.

            let cw721_code_id = CW721_CODE_ID.load(deps.storage)?;
            // for creating a predictable nft contract using, using instantiate2, we need: checksum, creator, and salt:
            // - using class id as salt for instantiating nft contract guarantees a) predictable address and b) uniqueness
            // for this salt must be of length 32 bytes, so we use sha256 to hash class id
            let mut hasher = Sha256::new();
            hasher.update(voucher.class.id.as_bytes());
            let salt = hasher.finalize().to_vec();

            get_instantiate2_address(
                deps.as_ref(),
                env.contract.address.as_str(),
                &salt,
                cw721_code_id,
            )
        } else {
            // This should never happen, as we must have at least 1 of the above actions
            Err(ContractError::InvalidTransferNoAction)
        }?;

        generate_receive_callback_msg(
            deps.as_ref(),
            &cloned_data,
            receive_callback_data,
            receive_callback_addr,
            nft_contract.to_string(),
        )
    } else {
        None
    };

    let incoming_proxy_msg =
        get_incoming_proxy_msg(deps.storage, packet.clone(), cloned_data.clone())?;
    let submessage = action_aggregator.into_submessage(
        env.contract.address,
        receiver,
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
    //
    // Having both redemption and creation action in the same transfer
    // tells us its a malicious act that we should reject.
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

    pub fn into_submessage(
        self,
        contract: Addr,
        receiver: Addr,
        callback_msg: Option<WasmMsg>,
        incoming_proxy_msg: Option<WasmMsg>,
    ) -> StdResult<SubMsg<Empty>> {
        let mut m = Vec::with_capacity(3); // 3 is the max number of submessages we can have
        if let Some(incoming_proxy_msg) = incoming_proxy_msg {
            m.push(incoming_proxy_msg)
        }

        // we can only have redeem or create, not both
        if let Some(redeem) = self.redemption {
            m.push(redeem.into_wasm_msg(contract.clone(), receiver.to_string())?)
        }
        if let Some(create) = self.creation {
            m.push(create.into_wasm_msg(contract.clone(), receiver.into_string())?)
        }

        if let Some(callback_msg) = callback_msg {
            m.push(callback_msg)
        }
        let message = if m.len() == 1 {
            m[0].clone()
        } else {
            WasmMsg::Execute {
                contract_addr: contract.into_string(),
                msg: to_json_binary(&ExecuteMsg::Callback(CallbackMsg::Conjunction {
                    operands: m,
                }))?,
                funds: vec![],
            }
        };
        Ok(SubMsg::reply_always(message, ACK_AND_DO_NOTHING_REPLY_ID))
    }
}
