use std::fmt::Debug;

use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Deps, DepsMut, Empty, Env, IbcMsg, MessageInfo, Response,
    StdResult, SubMsg, WasmMsg,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    ibc::{NonFungibleTokenPacketData, INSTANTIATE_CW721_REPLY_ID, INSTANTIATE_PROXY_REPLY_ID},
    msg::{CallbackMsg, ExecuteMsg, IbcOutgoingMsg, InstantiateMsg, MigrateMsg},
    state::{
        UniversalAllNftInfoResponse, CLASS_ID_TO_CLASS, CLASS_ID_TO_NFT_CONTRACT, CW721_CODE_ID,
        NFT_CONTRACT_TO_CLASS_ID, OUTGOING_CLASS_TOKEN_TO_CHANNEL, PO, PROXY, TOKEN_METADATA,
    },
    token_types::{Class, ClassId, Token, TokenId, VoucherCreation, VoucherRedemption},
    ContractError,
};

pub trait Ics721Execute<T = Empty>
where
    T: Serialize + DeserializeOwned + Clone,
{
    type ClassData: Serialize + DeserializeOwned + Clone + Debug;

    fn instantiate(
        &self,
        deps: DepsMut,
        env: Env,
        _info: MessageInfo,
        msg: InstantiateMsg,
    ) -> StdResult<Response<T>> {
        CW721_CODE_ID.save(deps.storage, &msg.cw721_base_code_id)?;
        PROXY.save(deps.storage, &None)?;
        PO.set_pauser(deps.storage, deps.api, msg.pauser.as_deref())?;

        let proxy_instantiate = msg
            .proxy
            .map(|m| m.into_wasm_msg(env.contract.address))
            .map(|wasm| SubMsg::reply_on_success(wasm, INSTANTIATE_PROXY_REPLY_ID))
            .map_or(vec![], |s| vec![s]);

        Ok(Response::default()
            .add_submessages(proxy_instantiate)
            .add_attribute("method", "instantiate")
            .add_attribute("cw721_code_id", msg.cw721_base_code_id.to_string()))
    }

    fn execute(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: ExecuteMsg,
    ) -> Result<Response<T>, ContractError> {
        PO.error_if_paused(deps.storage)?;
        match msg {
            ExecuteMsg::ReceiveNft(cw721::Cw721ReceiveMsg {
                sender,
                token_id,
                msg,
            }) => self.execute_receive_nft(deps, env, info, token_id, sender, msg),
            ExecuteMsg::ReceiveProxyNft { eyeball, msg } => {
                self.execute_receive_proxy_nft(deps, env, info, eyeball, msg)
            }
            ExecuteMsg::Pause {} => self.execute_pause(deps, info),
            ExecuteMsg::Callback(msg) => self.execute_callback(deps, env, info, msg),
        }
    }

    fn execute_receive_nft(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        token_id: String,
        sender: String,
        msg: Binary,
    ) -> Result<Response<T>, ContractError> {
        if PROXY.load(deps.storage)?.is_some() {
            Err(ContractError::Unauthorized {})
        } else {
            self.receive_nft(deps, env, info, TokenId::new(token_id), sender, msg)
        }
    }

    fn execute_receive_proxy_nft(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        eyeball: String,
        msg: cw721::Cw721ReceiveMsg,
    ) -> Result<Response<T>, ContractError> {
        if PROXY
            .load(deps.storage)?
            .map_or(true, |proxy| info.sender != proxy)
        {
            return Err(ContractError::Unauthorized {});
        }
        let mut info = info;
        info.sender = deps.api.addr_validate(&eyeball)?;
        let cw721::Cw721ReceiveMsg {
            token_id,
            sender,
            msg,
        } = msg;
        let res = self.receive_nft(deps, env, info, TokenId::new(token_id), sender, msg)?;
        // override method key
        Ok(res.add_attribute("method", "execute_receive_proxy_nft"))
    }

    fn get_class_data(&self, deps: &DepsMut, sender: &Addr) -> StdResult<Option<Self::ClassData>>;

    fn receive_nft(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        token_id: TokenId,
        sender: String,
        msg: Binary,
    ) -> Result<Response<T>, ContractError> {
        let sender = deps.api.addr_validate(&sender)?;
        let msg: IbcOutgoingMsg = from_binary(&msg)?;

        let class = match NFT_CONTRACT_TO_CLASS_ID.may_load(deps.storage, info.sender.clone())? {
            Some(class_id) => CLASS_ID_TO_CLASS.load(deps.storage, class_id)?,
            // No class ID being present means that this is a local NFT
            // that has never been sent out of this contract.
            None => {
                let class_data = self.get_class_data(&deps, &info.sender)?;
                let data = class_data.as_ref().map(to_binary).transpose()?;
                let class = Class {
                    id: ClassId::new(info.sender.to_string()),
                    // There is no collection-level uri nor data in the
                    // cw721 specification so we set those values to
                    // `None` for local, cw721 NFTs.
                    uri: None,
                    data,
                };

                NFT_CONTRACT_TO_CLASS_ID.save(deps.storage, info.sender.clone(), &class.id)?;
                CLASS_ID_TO_NFT_CONTRACT.save(deps.storage, class.id.clone(), &info.sender)?;

                // Merging and usage of this PR may change that:
                // <https://github.com/CosmWasm/cw-nfts/pull/75>
                CLASS_ID_TO_CLASS.save(deps.storage, class.id.clone(), &class)?;
                class
            }
        };

        let UniversalAllNftInfoResponse { access, info } = deps.querier.query_wasm_smart(
            info.sender,
            &cw721::Cw721QueryMsg::AllNftInfo {
                token_id: token_id.clone().into(),
                include_expired: None,
            },
        )?;
        // make sure NFT is escrowed by ics721
        if access.owner != env.contract.address {
            return Err(ContractError::Unauthorized {});
        }

        let token_metadata = TOKEN_METADATA
            .may_load(deps.storage, (class.id.clone(), token_id.clone()))?
            .flatten();

        let ibc_message = NonFungibleTokenPacketData {
            class_id: class.id.clone(),
            class_uri: class.uri,
            class_data: class.data.clone(),

            token_ids: vec![token_id.clone()],
            token_uris: info.token_uri.map(|uri| vec![uri]),
            token_data: token_metadata.map(|metadata| vec![metadata]),

            sender: sender.into_string(),
            receiver: msg.receiver,
            memo: msg.memo,
        };
        let ibc_message = IbcMsg::SendPacket {
            channel_id: msg.channel_id.clone(),
            data: to_binary(&ibc_message)?,
            timeout: msg.timeout,
        };

        OUTGOING_CLASS_TOKEN_TO_CHANNEL.save(
            deps.storage,
            (class.id.clone(), token_id.clone()),
            &msg.channel_id,
        )?;
        // class_data might be collection data (if it comes from ICS721 contract) or some custom data (e.g. coming from nft-transfer module)
        // so only can output binary here
        let class_data_string = class
            .data
            .map_or("none".to_string(), |data| format!("{:?}", data));

        Ok(Response::default()
            .add_attribute("method", "execute_receive_nft")
            .add_attribute("token_id", token_id)
            .add_attribute("class_id", class.id)
            .add_attribute("class_data", class_data_string)
            .add_attribute("channel_id", msg.channel_id)
            .add_message(ibc_message))
    }

    fn execute_pause(
        &self,
        deps: DepsMut,
        info: MessageInfo,
    ) -> Result<Response<T>, ContractError> {
        PO.pause(deps.storage, &info.sender)?;
        Ok(Response::default().add_attribute("method", "pause"))
    }

    fn execute_callback(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: CallbackMsg,
    ) -> Result<Response<T>, ContractError> {
        if info.sender != env.contract.address {
            Err(ContractError::Unauthorized {})
        } else {
            match msg {
                CallbackMsg::CreateVouchers { receiver, create } => {
                    self.callback_create_vouchers(deps, env, receiver, create)
                }
                CallbackMsg::RedeemVouchers { receiver, redeem } => {
                    self.callback_redeem_vouchers(deps, receiver, redeem)
                }
                CallbackMsg::Mint {
                    class_id,
                    tokens,
                    receiver,
                } => self.callback_mint(deps, class_id, tokens, receiver),

                CallbackMsg::Conjunction { operands } => {
                    Ok(Response::default().add_messages(operands))
                }
            }
        }
    }

    /// Creates the specified debt vouchers by minting cw721 debt-voucher
    /// tokens for the receiver. If no debt-voucher collection yet exists
    /// a new collection is instantiated before minting the vouchers.
    fn callback_create_vouchers(
        &self,
        deps: DepsMut,
        env: Env,
        receiver: String,
        create: VoucherCreation,
    ) -> Result<Response<T>, ContractError> {
        let VoucherCreation { class, tokens } = create;
        let instantiate = if CLASS_ID_TO_NFT_CONTRACT.has(deps.storage, class.id.clone()) {
            vec![]
        } else {
            let message = SubMsg::<T>::reply_on_success(
                WasmMsg::Instantiate {
                    admin: None,
                    code_id: CW721_CODE_ID.load(deps.storage)?,
                    msg: self.init_msg(deps.as_ref(), &env, &class)?,
                    funds: vec![],
                    // Attempting to fit the class ID in the label field
                    // can make this field too long which causes data
                    // errors in the SDK.
                    label: "ics-721 debt-voucher cw-721".to_string(),
                },
                INSTANTIATE_CW721_REPLY_ID,
            );
            vec![message]
        };

        // Store mapping from classID to classURI. Notably, we don't check
        // if this has already been set. If a new NFT belonging to a class
        // ID we have already seen comes in with new metadata, we assume
        // that the metadata has been updated on the source chain and
        // update it for the class ID locally as well.
        CLASS_ID_TO_CLASS.save(deps.storage, class.id.clone(), &class)?;

        let mint = WasmMsg::Execute {
            contract_addr: env.contract.address.into_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::Mint {
                class_id: class.id,
                receiver,
                tokens,
            }))?,
            funds: vec![],
        };

        Ok(Response::<T>::default()
            .add_attribute("method", "callback_create_vouchers")
            .add_submessages(instantiate)
            .add_message(mint))
    }

    /// Default implementation using `cw721_base::msg::InstantiateMsg`
    fn init_msg(&self, _deps: Deps, env: &Env, class: &Class) -> StdResult<Binary> {
        to_binary(&cw721_base::msg::InstantiateMsg {
            // Name of the collection MUST be class_id as this is how
            // we create a map entry on reply.
            name: class.id.clone().into(),
            symbol: class.id.clone().into(),
            minter: env.contract.address.to_string(),
        })
    }

    /// Performs a recemption of debt vouchers returning the corresponding
    /// tokens to the receiver.
    fn callback_redeem_vouchers(
        &self,
        deps: DepsMut,
        receiver: String,
        redeem: VoucherRedemption,
    ) -> Result<Response<T>, ContractError> {
        let VoucherRedemption { class, token_ids } = redeem;
        let nft_contract = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class.id)?;
        let receiver = deps.api.addr_validate(&receiver)?;
        Ok(Response::default()
            .add_attribute("method", "callback_redeem_vouchers")
            .add_messages(
                token_ids
                    .into_iter()
                    .map(|token_id| {
                        Ok(WasmMsg::Execute {
                            contract_addr: nft_contract.to_string(),
                            msg: to_binary(&cw721::Cw721ExecuteMsg::TransferNft {
                                recipient: receiver.to_string(),
                                token_id: token_id.into(),
                            })?,
                            funds: vec![],
                        })
                    })
                    .collect::<StdResult<Vec<WasmMsg>>>()?,
            ))
    }

    fn callback_mint(
        &self,
        deps: DepsMut,
        class_id: ClassId,
        tokens: Vec<Token>,
        receiver: String,
    ) -> Result<Response<T>, ContractError> {
        let receiver = deps.api.addr_validate(&receiver)?;
        let cw721_addr = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id.clone())?;

        let mint = tokens
            .into_iter()
            .map(|Token { id, uri, data }| {
                // We save token metadata here as, ideally, once cw721
                // supports on-chain metadata, this is where we will set
                // that value on the debt-voucher token. Note that this is
                // set for every token, regardless of if data is None.
                TOKEN_METADATA.save(deps.storage, (class_id.clone(), id.clone()), &data)?;

                let msg = cw721_base::msg::ExecuteMsg::<Empty, Empty>::Mint {
                    token_id: id.into(),
                    token_uri: uri,
                    owner: receiver.to_string(),
                    extension: Empty::default(),
                };
                Ok(WasmMsg::Execute {
                    contract_addr: cw721_addr.to_string(),
                    msg: to_binary(&msg)?,
                    funds: vec![],
                })
            })
            .collect::<StdResult<Vec<_>>>()?;

        Ok(Response::default()
            .add_attribute("method", "callback_mint")
            .add_messages(mint))
    }

    fn migrate(
        &self,
        deps: DepsMut,
        _env: Env,
        msg: MigrateMsg,
    ) -> Result<Response<T>, ContractError> {
        match msg {
            MigrateMsg::WithUpdate {
                pauser,
                proxy,
                cw721_base_code_id,
            } => {
                PROXY.save(
                    deps.storage,
                    &proxy
                        .as_ref()
                        .map(|h| deps.api.addr_validate(h))
                        .transpose()?,
                )?;
                PO.set_pauser(deps.storage, deps.api, pauser.as_deref())?;
                if let Some(cw721_base_code_id) = cw721_base_code_id {
                    CW721_CODE_ID.save(deps.storage, &cw721_base_code_id)?;
                }
                Ok(Response::default()
                    .add_attribute("method", "migrate")
                    .add_attribute("pauser", pauser.map_or_else(|| "none".to_string(), |or| or))
                    .add_attribute("proxy", proxy.map_or_else(|| "none".to_string(), |or| or))
                    .add_attribute(
                        "cw721_base_code_id",
                        cw721_base_code_id.map_or_else(|| "none".to_string(), |or| or.to_string()),
                    ))
            }
        }
    }
}
