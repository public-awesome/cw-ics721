use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    from_binary, to_binary, Binary, DepsMut, Empty, Env, IbcBasicResponse, IbcChannelCloseMsg,
    IbcChannelConnectMsg, IbcChannelOpenMsg, IbcChannelOpenResponse, IbcPacket, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, Reply, Response, StdResult,
    SubMsgResult, WasmMsg,
};
use cw_utils::parse_reply_instantiate_data;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    error::Never,
    ibc_helpers::{ack_fail, ack_success, try_get_ack_error, validate_order_and_version},
    ibc_packet_receive::receive_ibc_packet,
    state::{
        CLASS_ID_TO_NFT_CONTRACT, INCOMING_CLASS_TOKEN_TO_CHANNEL, NFT_CONTRACT_TO_CLASS_ID,
        OUTGOING_CLASS_TOKEN_TO_CHANNEL, PROXY, TOKEN_METADATA,
    },
    token_types::{ClassId, TokenId},
    ContractError,
};

/// Submessage reply ID used for instantiating cw721 contracts.
pub(crate) const INSTANTIATE_CW721_REPLY_ID: u64 = 0;
/// Submessage reply ID used for instantiating the proxy contract.
pub(crate) const INSTANTIATE_PROXY_REPLY_ID: u64 = 1;
/// Submessages dispatched with this reply ID will set the ack on the
/// response depending on if the submessage execution succeded or
/// failed.
pub(crate) const ACK_AND_DO_NOTHING: u64 = 2;
/// The IBC version this contract expects to communicate with.
pub const IBC_VERSION: &str = "ics721-1";

#[cw_serde]
#[serde(rename_all = "camelCase")]
pub struct NonFungibleTokenPacketData {
    /// Uniquely identifies the collection which the tokens being
    /// transfered belong to on the sending chain. Must be non-empty.
    pub class_id: ClassId,
    /// Optional URL that points to metadata about the
    /// collection. Must be non-empty if provided.
    pub class_uri: Option<String>,
    /// Optional base64 encoded field which contains on-chain metadata
    /// about the NFT class. Must be non-empty if provided.
    pub class_data: Option<Binary>,
    /// Uniquely identifies the tokens in the NFT collection being
    /// transfered. This MUST be non-empty.
    pub token_ids: Vec<TokenId>,
    /// Optional URL that points to metadata for each token being
    /// transfered. `tokenUris[N]` should hold the metadata for
    /// `tokenIds[N]` and both lists should have the same if
    /// provided. Must be non-empty if provided.
    pub token_uris: Option<Vec<String>>,
    /// Optional base64 encoded metadata for the tokens being
    /// transfered. `tokenData[N]` should hold metadata for
    /// `tokenIds[N]` and both lists should have the same length if
    /// provided. Must be non-empty if provided.
    pub token_data: Option<Vec<Binary>>,

    /// The address sending the tokens on the sending chain.
    pub sender: String,
    /// The address that should receive the tokens on the receiving
    /// chain.
    pub receiver: String,
    /// Memo to add custom string to the msg
    pub memo: Option<String>,
}

pub trait Ics721Ibc<T = Empty>
where
    T: Serialize + DeserializeOwned + Clone,
{
    fn ibc_channel_open(
        &self,
        _deps: DepsMut,
        _env: Env,
        msg: IbcChannelOpenMsg,
    ) -> Result<IbcChannelOpenResponse, ContractError> {
        validate_order_and_version(msg.channel(), msg.counterparty_version())?;
        Ok(None)
    }

    fn ibc_channel_connect(
        &self,
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

    fn ibc_channel_close(
        &self,
        _deps: DepsMut,
        _env: Env,
        msg: IbcChannelCloseMsg,
    ) -> Result<IbcBasicResponse, ContractError> {
        match msg {
            // Error any TX that would cause the channel to close that is
            // coming from the local chain.
            IbcChannelCloseMsg::CloseInit { channel: _ } => Err(ContractError::CantCloseChannel {}),
            // If we're here, something has gone catastrophically wrong on
            // our counterparty chain. Per the `CloseInit` handler above,
            // this contract will _never_ allow its channel to be
            // closed.
            //
            // Clearly, if this happens for a channel with real NFTs that
            // have been sent out on it, we need some admin
            // intervention. What intervention? No idea. It is unclear why
            // this would ever happen (without the counterparty being
            // malicious in which case it's also situational), yet alone
            // what to do in response. The admin of this contract is
            // expected to migrate it if this happens.
            //
            // Note: erroring here would prevent our side of the channel
            // closing (bad because the channel is, for all intents and
            // purposes, closed) so we must allow the transaction through.
            IbcChannelCloseMsg::CloseConfirm { channel: _ } => Ok(IbcBasicResponse::default()),
        }
    }

    fn ibc_packet_receive(
        &self,
        deps: DepsMut,
        env: Env,
        msg: IbcPacketReceiveMsg,
    ) -> Result<IbcReceiveResponse, Never> {
        // Regardless of if our processing of this packet works we need to
        // commit an ACK to the chain. As such, we wrap all handling logic
        // in a seprate function and on error write out an error ack.
        match receive_ibc_packet(deps, env, msg.packet) {
            Ok(response) => Ok(response),
            Err(error) => Ok(IbcReceiveResponse::new()
                .add_attribute("method", "ibc_packet_receive")
                .add_attribute("error", error.to_string())
                .set_ack(ack_fail(error.to_string()))),
        }
    }

    fn ibc_packet_ack(
        &self,
        deps: DepsMut,
        _env: Env,
        ack: IbcPacketAckMsg,
    ) -> Result<IbcBasicResponse, ContractError> {
        if let Some(error) = try_get_ack_error(&ack.acknowledgement) {
            self.handle_packet_fail(deps, ack.original_packet, &error)
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
                        TOKEN_METADATA.remove(deps.storage, (msg.class_id.clone(), token.clone()));

                        messages.push(WasmMsg::Execute {
                            contract_addr: nft_contract.to_string(),
                            msg: to_binary(&cw721::Cw721ExecuteMsg::Burn {
                                token_id: token.into(),
                            })?,
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

    fn ibc_packet_timeout(
        &self,
        deps: DepsMut,
        _env: Env,
        msg: IbcPacketTimeoutMsg,
    ) -> Result<IbcBasicResponse, ContractError> {
        self.handle_packet_fail(deps, msg.packet, "timeout")
    }

    /// Return the NFT locked in the ICS721 contract to sender; roll back.
    fn handle_packet_fail(
        &self,
        deps: DepsMut,
        packet: IbcPacket,
        error: &str,
    ) -> Result<IbcBasicResponse, ContractError> {
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
                        token_id: token_id.into(),
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

    fn reply(&self, deps: DepsMut, _env: Env, reply: Reply) -> Result<Response<T>, ContractError> {
        match reply.id {
            INSTANTIATE_CW721_REPLY_ID => {
                // Don't need to add an ack or check for an error here as this
                // is only replies on success. This is OK because it is only
                // ever used in `DoInstantiateAndMint` which itself is always
                // a submessage of `ibc_packet_receive` which is caught and
                // handled correctly by the reply handler for
                // `ACK_AND_DO_NOTHING`.

                let res = parse_reply_instantiate_data(reply)?;
                let cw721_addr = deps.api.addr_validate(&res.contract_address)?;

                // We need to map this address back to a class
                // ID. Fourtunately, we set the name of the new NFT
                // contract to the class ID.
                let cw721::ContractInfoResponse { name, .. } = deps
                    .querier
                    .query_wasm_smart(cw721_addr.clone(), &cw721::Cw721QueryMsg::ContractInfo {})?;
                let class_id = ClassId::new(name);

                // Save classId <-> contract mappings.
                CLASS_ID_TO_NFT_CONTRACT.save(deps.storage, class_id.clone(), &cw721_addr)?;
                NFT_CONTRACT_TO_CLASS_ID.save(deps.storage, cw721_addr.clone(), &class_id)?;

                Ok(Response::default()
                    .add_attribute("method", "instantiate_cw721_reply")
                    .add_attribute("class_id", class_id)
                    .add_attribute("cw721_addr", cw721_addr))
            }
            INSTANTIATE_PROXY_REPLY_ID => {
                let res = parse_reply_instantiate_data(reply)?;
                let proxy_addr = deps.api.addr_validate(&res.contract_address)?;
                PROXY.save(deps.storage, &Some(proxy_addr))?;

                Ok(Response::default()
                    .add_attribute("method", "instantiate_proxy_reply_id")
                    .add_attribute("proxy", res.contract_address))
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
                    SubMsgResult::Err(err) => Ok(Response::new().set_data(ack_fail(err))),
                }
            }
            _ => Err(ContractError::UnrecognisedReplyId {}),
        }
    }
}
