use std::fmt::Debug;

use cosmwasm_std::{
    from_json, to_json_binary, Addr, Binary, ContractInfoResponse, Deps, DepsMut, Empty, Env,
    Event, IbcMsg, MessageInfo, Order, Response, StdResult, SubMsg, WasmMsg,
};
use cw721::{
    DefaultOptionalCollectionExtension, DefaultOptionalCollectionExtensionMsg,
    DefaultOptionalNftExtension, DefaultOptionalNftExtensionMsg,
};
use cw_storage_plus::Map;
use ics721_types::{
    ibc_types::{IbcOutgoingMsg, IbcOutgoingProxyMsg, NonFungibleTokenPacketData},
    token_types::{Class, ClassId, Token, TokenId},
};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    helpers::get_instantiate2_address,
    ibc::{
        INSTANTIATE_CW721_REPLY_ID, INSTANTIATE_INCOMING_PROXY_REPLY_ID,
        INSTANTIATE_OUTGOING_PROXY_REPLY_ID,
    },
    msg::{CallbackMsg, ExecuteMsg, InstantiateMsg, MigrateMsg},
    query::{
        load_class_id_for_nft_contract, load_nft_contract_for_class_id,
        query_nft_contract_for_class_id, query_nft_contracts,
    },
    state::{
        ClassIdInfo, CollectionData, UniversalAllNftInfoResponse, ADMIN_USED_FOR_CW721,
        CLASS_ID_AND_NFT_CONTRACT_INFO, CLASS_ID_TO_CLASS, CONTRACT_ADDR_LENGTH, CW721_CODE_ID,
        INCOMING_CLASS_TOKEN_TO_CHANNEL, INCOMING_PROXY, OUTGOING_CLASS_TOKEN_TO_CHANNEL,
        OUTGOING_PROXY, PO, TOKEN_METADATA,
    },
    token_types::{VoucherCreation, VoucherRedemption},
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
        // proxy contracts are optional
        INCOMING_PROXY.save(deps.storage, &None)?;
        OUTGOING_PROXY.save(deps.storage, &None)?;
        PO.set_pauser(deps.storage, deps.api, msg.pauser.as_deref())?;

        let mut proxies_instantiate: Vec<SubMsg<T>> = Vec::new();
        if let Some(cii) = msg.incoming_proxy {
            proxies_instantiate.push(SubMsg::reply_on_success(
                cii.into_wasm_msg(env.clone().contract.address),
                // on reply proxy contract is set in INCOMING_PROXY
                INSTANTIATE_INCOMING_PROXY_REPLY_ID,
            ));
        }
        if let Some(cii) = msg.outgoing_proxy {
            proxies_instantiate.push(SubMsg::reply_on_success(
                cii.into_wasm_msg(env.contract.address),
                // on reply proxy contract is set in OUTGOING_PROXY
                INSTANTIATE_OUTGOING_PROXY_REPLY_ID,
            ));
        }

        ADMIN_USED_FOR_CW721.save(
            deps.storage,
            &msg.cw721_admin
                .as_ref()
                .map(|h| deps.api.addr_validate(h))
                .transpose()?,
        )?;

        let contract_addr_length = msg.contract_addr_length;
        if let Some(contract_addr_length) = contract_addr_length {
            CONTRACT_ADDR_LENGTH.save(deps.storage, &contract_addr_length)?;
        } else {
            CONTRACT_ADDR_LENGTH.remove(deps.storage);
        }

        Ok(Response::default()
            .add_submessages(proxies_instantiate)
            .add_attribute("method", "instantiate")
            .add_attribute("cw721_code_id", msg.cw721_base_code_id.to_string())
            .add_attribute(
                "cw721_admin",
                msg.cw721_admin
                    .map_or_else(|| "immutable".to_string(), |or| or),
            )
            .add_attribute(
                "contract_addr_length",
                contract_addr_length.map_or_else(|| "none".to_string(), |or| or.to_string()),
            ))
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
            ExecuteMsg::ReceiveNft(cw721::receiver::Cw721ReceiveMsg {
                sender,
                token_id,
                msg,
            }) => self.execute_receive_nft(deps, env, info, token_id, sender, msg),
            ExecuteMsg::Pause {} => self.execute_pause(deps, info),
            ExecuteMsg::Callback(msg) => self.execute_callback(deps, env, info, msg),
            ExecuteMsg::AdminCleanAndBurnNft {
                owner,
                token_id,
                class_id,
                collection,
            } => self.execute_admin_clean_and_burn_nft(
                deps, env, info, owner, token_id, class_id, collection,
            ),
            ExecuteMsg::AdminCleanAndUnescrowNft {
                recipient,
                token_id,
                class_id,
                collection,
            } => self.execute_admin_clean_and_unescrow_nft(
                deps, env, info, recipient, token_id, class_id, collection,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_admin_clean_and_burn_nft(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        owner: String,
        token_id: String,
        child_class_id: String,
        child_collection: String,
    ) -> Result<Response<T>, ContractError> {
        deps.api.addr_validate(&owner)?;
        // only admin can call this method
        let ContractInfoResponse { admin, .. } = deps
            .querier
            .query_wasm_contract_info(env.contract.address.to_string())?;
        if admin.is_some() && info.sender != admin.unwrap() {
            return Err(ContractError::Unauthorized {});
        }

        // check given child class id and child collection is the same as stored in the contract
        let token_id = TokenId::new(token_id);
        let child_class_id = ClassId::new(child_class_id);
        let child_collection = deps.api.addr_validate(&child_collection)?;
        match query_nft_contract_for_class_id(deps.storage, child_class_id.clone())? {
            Some(cw721_addr) => {
                if cw721_addr != child_collection {
                    return Err(ContractError::NoNftContractMatch {
                        child_collection: child_collection.to_string(),
                        class_id: child_class_id.to_string(),
                        token_id: token_id.into(),
                        cw721_addr: cw721_addr.to_string(),
                    });
                }
            }
            None => {
                return Err(ContractError::NoNftContractForClassId(
                    child_class_id.to_string(),
                ))
            }
        }

        // remove incoming channel entry and metadata
        INCOMING_CLASS_TOKEN_TO_CHANNEL
            .remove(deps.storage, (child_class_id.clone(), token_id.clone()));
        TOKEN_METADATA.remove(deps.storage, (child_class_id.clone(), token_id.clone()));

        // check NFT on child collection owned by recipient
        let maybe_nft_info: Option<UniversalAllNftInfoResponse> = deps
            .querier
            .query_wasm_smart(
                child_collection.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::AllNftInfo {
                    token_id: token_id.clone().into(),
                    include_expired: None,
                },
            )
            .ok();

        let mut response =
            Response::default().add_attribute("method", "execute_admin_clean_and_burn_nft");
        if let Some(UniversalAllNftInfoResponse { access, .. }) = maybe_nft_info {
            if access.owner != owner {
                return Err(ContractError::NotOwnerOfNft {
                    recipient: owner.to_string(),
                    token_id: token_id.clone().into(),
                    owner: access.owner.to_string(),
                });
            }
            // burn child NFT
            // note: this requires approval from recipient, or recipient burns it himself
            let burn_msg = WasmMsg::Execute {
                contract_addr: child_collection.to_string(),
                msg: to_json_binary(&cw721::msg::Cw721ExecuteMsg::<
                    DefaultOptionalNftExtensionMsg,
                    DefaultOptionalCollectionExtensionMsg,
                    Empty,
                >::Burn {
                    token_id: token_id.clone().into(),
                })?,
                funds: vec![],
            };
            response = response.add_message(burn_msg);
        }

        Ok(response)
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_admin_clean_and_unescrow_nft(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        recipient: String,
        token_id: String,
        home_class_id: String,
        home_collection: String,
    ) -> Result<Response<T>, ContractError> {
        deps.api.addr_validate(&recipient)?;
        // only admin can call this method
        let ContractInfoResponse { admin, .. } = deps
            .querier
            .query_wasm_contract_info(env.contract.address.to_string())?;
        if admin.is_some() && info.sender != admin.unwrap() {
            return Err(ContractError::Unauthorized {});
        }

        // check given home class id and home collection is the same as stored in the contract
        let home_class_id = ClassId::new(home_class_id);
        let home_collection = deps.api.addr_validate(&home_collection)?;
        match query_nft_contract_for_class_id(deps.storage, home_class_id.clone())? {
            Some(cw721_addr) => {
                if cw721_addr != home_collection {
                    return Err(ContractError::NoNftContractMatch {
                        child_collection: home_collection.to_string(),
                        class_id: home_class_id.to_string(),
                        token_id,
                        cw721_addr: cw721_addr.to_string(),
                    });
                }
            }
            None => {
                return Err(ContractError::NoNftContractForClassId(
                    home_class_id.to_string(),
                ))
            }
        }

        // remove outgoing channel entry
        let token_id = TokenId::new(token_id);
        OUTGOING_CLASS_TOKEN_TO_CHANNEL
            .remove(deps.storage, (home_class_id.clone(), token_id.clone()));

        // check NFT on home collection owned by ics721 contract
        let maybe_nft_info: Option<UniversalAllNftInfoResponse> = deps
            .querier
            .query_wasm_smart(
                home_collection.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::AllNftInfo {
                    token_id: token_id.clone().into(),
                    include_expired: None,
                },
            )
            .ok();

        let mut response =
            Response::default().add_attribute("method", "execute_admin_clean_and_unescrow_nft");
        if let Some(UniversalAllNftInfoResponse { access, .. }) = maybe_nft_info {
            if access.owner != env.contract.address {
                return Err(ContractError::NotEscrowedByIcs721(access.owner.to_string()));
            }
            // transfer NFT
            let transfer_msg = WasmMsg::Execute {
                contract_addr: home_collection.to_string(),
                msg: to_json_binary(&cw721::msg::Cw721ExecuteMsg::<
                    DefaultOptionalNftExtensionMsg,
                    DefaultOptionalCollectionExtensionMsg,
                    Empty,
                >::TransferNft {
                    recipient: recipient.to_string(),
                    token_id: token_id.clone().into(),
                })?,
                funds: vec![],
            };

            response = response.add_message(transfer_msg);
        }

        Ok(response)
    }

    /// ICS721 may receive an NFT from 2 sources:
    /// 1. From a local cw721 contract (e.g. cw721-base)
    /// 2. From a(n outgoing) proxy contract.
    ///
    /// In case of 2. outgoing proxy calls 2 messages:
    /// a) tranfer NFT to ICS721
    /// b) call/forwards "ReceiveNFt" message to ICS721.
    ///
    /// Unlike 1) proxy passes in b) an IbcOutgoingProxyMsg (and not an IbcOutgoingMsg)
    /// which also holds the collection address, since info.sender
    /// is the proxy contract - and not the collection.
    ///
    /// NB: outgoing proxy can use `SendNft` on collectio and pass it directly to ICS721,
    /// since one `OUTGOING_PROXY` is defined in ICS721, it accepts only NFT receives from this proxy.
    fn execute_receive_nft(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        token_id: String,
        nft_owner: String,
        msg: Binary,
    ) -> Result<Response<T>, ContractError> {
        // if there is an outgoing proxy, we need to check if the msg is IbcOutgoingProxyMsg
        let result = match OUTGOING_PROXY.load(deps.storage)? {
            Some(proxy) => {
                // accept only messages from the proxy
                if proxy != info.sender {
                    return Err(ContractError::Unauthorized {});
                }
                from_json::<IbcOutgoingProxyMsg>(msg.clone())
                    .ok()
                    .map(|msg| match deps.api.addr_validate(&msg.collection) {
                        Ok(nft_contract) => self.receive_nft(
                            deps,
                            env,
                            &nft_contract,
                            TokenId::new(token_id),
                            nft_owner,
                            msg.msg,
                        ),
                        Err(err) => Err(ContractError::Std(err)),
                    })
            }
            None => from_json::<IbcOutgoingMsg>(msg.clone()).ok().map(|_| {
                self.receive_nft(
                    deps,
                    env,
                    &info.sender,
                    TokenId::new(token_id),
                    nft_owner,
                    msg.clone(),
                )
            }),
        };
        result.ok_or(ContractError::UnknownMsg(msg))?
    }

    fn get_class_data(&self, deps: &DepsMut, sender: &Addr) -> StdResult<Option<Self::ClassData>>;

    fn receive_nft(
        &self,
        deps: DepsMut,
        env: Env,
        nft_contract: &Addr,
        token_id: TokenId,
        nft_owner: String,
        msg: Binary,
    ) -> Result<Response<T>, ContractError> {
        let nft_owner = deps.api.addr_validate(&nft_owner)?;
        let msg: IbcOutgoingMsg = from_json(msg)?;

        let class = match load_class_id_for_nft_contract(deps.as_ref().storage, nft_contract)? {
            Some(class_id) => CLASS_ID_TO_CLASS.load(deps.storage, class_id)?,
            // No class ID being present means that this is a local NFT
            // that has never been sent out of this contract.
            None => {
                let class_data = self.get_class_data(&deps, nft_contract)?;
                let data = class_data.as_ref().map(to_json_binary).transpose()?;
                let class = Class {
                    id: ClassId::new(nft_contract.to_string()),
                    // There is no collection-level uri nor data in the
                    // cw721 specification so we set those values to
                    // `None` for local, cw721 NFTs.
                    uri: None,
                    data,
                };

                let class_id_info = ClassIdInfo {
                    class_id: class.id.clone(),
                    address: nft_contract.clone(),
                };
                CLASS_ID_AND_NFT_CONTRACT_INFO.save(deps.storage, &class.id, &class_id_info)?;

                // Merging and usage of this PR may change that:
                // <https://github.com/CosmWasm/cw-nfts/pull/75>
                CLASS_ID_TO_CLASS.save(deps.storage, class.id.clone(), &class)?;
                class
            }
        };

        // make sure NFT is escrowed by ics721
        let UniversalAllNftInfoResponse { access, info } = deps.querier.query_wasm_smart(
            nft_contract,
            &cw721::msg::Cw721QueryMsg::<
                DefaultOptionalNftExtension,
                DefaultOptionalCollectionExtension,
                Empty,
            >::AllNftInfo {
                token_id: token_id.clone().into(),
                include_expired: None,
            },
        )?;
        if access.owner != env.contract.address {
            return Err(ContractError::NotEscrowedByIcs721(access.owner));
        }

        // cw721 doesn't support on-chain metadata yet
        // here NFT is transferred to another chain, NFT itself may have been transferred to his chain before
        // in this case ICS721 may have metadata stored
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

            sender: nft_owner.into_string(),
            receiver: msg.receiver,
            memo: msg.memo,
        };
        let ibc_message = IbcMsg::SendPacket {
            channel_id: msg.channel_id.clone(),
            data: to_json_binary(&ibc_message)?,
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
            .map_or("none".to_string(), |data| format!("{data:?}"));

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
                CallbackMsg::RedeemOutgoingChannelEntries(entries) => {
                    self.callback_redeem_outgoing_channel_entries(deps, entries)
                }
                CallbackMsg::AddIncomingChannelEntries(entries) => {
                    self.callback_save_incoming_channel_entries(deps, entries)
                }
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

        // Store mapping from classID to classURI. Notably, we don't check
        // if this has already been set. If a new NFT belonging to a class
        // ID we have already seen comes in with new metadata, we assume
        // that the metadata has been updated on the source chain and
        // update it for the class ID locally as well.
        CLASS_ID_TO_CLASS.save(deps.storage, class.id.clone(), &class)?;

        let mint = WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_json_binary(&ExecuteMsg::Callback(CallbackMsg::Mint {
                class_id: class.id.clone(),
                receiver,
                tokens: tokens.clone(),
            }))?,
            funds: vec![],
        };

        let (class_id_info, instantiate) =
            self.create_instantiate_msg(deps, &env, class.clone())?;

        let token_ids = format!("{:?}", tokens);
        let event = Event::new("ics721_receive_create_vouchers")
            .add_attribute("class_id", class_id_info.class_id)
            .add_attribute("nft_contract", class_id_info.address)
            .add_attribute("token_ids", token_ids);

        Ok(Response::<T>::default()
            .add_attribute("method", "callback_create_vouchers")
            .add_event(event)
            .add_submessages(instantiate)
            .add_message(mint))
    }

    fn create_instantiate_msg(
        &self,
        deps: DepsMut,
        env: &Env,
        class: Class,
    ) -> Result<(ClassIdInfo, Vec<SubMsg<T>>), ContractError> {
        let maybe_class_id_info =
            CLASS_ID_AND_NFT_CONTRACT_INFO.may_load(deps.as_ref().storage, &class.id)?;
        if let Some(nft_contract) = maybe_class_id_info {
            Ok((nft_contract, vec![]))
        } else {
            let class_id = ClassId::new(class.id.clone());
            let cw721_code_id = CW721_CODE_ID.load(deps.storage)?;
            // for creating a predictable nft contract using, using instantiate2, we need: checksum, creator, and salt:
            // - using class id as salt for instantiating nft contract guarantees a) predictable address and b) uniqueness
            // for this salt must be of length 32 bytes, so we use sha256 to hash class id
            let mut hasher = Sha256::new();
            hasher.update(class_id.as_bytes());
            let salt = hasher.finalize().to_vec();

            let nft_contract = get_instantiate2_address(
                deps.as_ref(),
                env.contract.address.as_str(),
                &salt,
                cw721_code_id,
            )?;

            // Save classId <-> contract mappings.
            let class_id_info = ClassIdInfo {
                class_id: class_id.clone(),
                address: nft_contract.clone(),
            };
            CLASS_ID_AND_NFT_CONTRACT_INFO.save(deps.storage, &class.id, &class_id_info)?;

            let admin = ADMIN_USED_FOR_CW721
                .load(deps.storage)?
                .map(|a| a.to_string());
            let message = SubMsg::<T>::reply_on_success(
                WasmMsg::Instantiate2 {
                    admin,
                    code_id: cw721_code_id,
                    msg: self.init_msg(deps.as_ref(), env, &class)?,
                    funds: vec![],
                    // Attempting to fit the class ID in the label field
                    // can make this field too long which causes data
                    // errors in the SDK.
                    label: "ics-721 debt-voucher cw-721".to_string(),
                    salt: salt.into(),
                },
                INSTANTIATE_CW721_REPLY_ID,
            );
            Ok((class_id_info, vec![message]))
        }
    }

    /// Default implementation using `cw721_base::msg::InstantiateMsg`
    fn init_msg(&self, deps: Deps, env: &Env, class: &Class) -> StdResult<Binary> {
        // use ics721 creator for withdraw address
        let ContractInfoResponse { creator, .. } = deps
            .querier
            .query_wasm_contract_info(env.contract.address.to_string())?;

        // use by default ClassId, in case there's no class data with name and symbol
        let mut instantiate_msg =
            cw721_base::msg::InstantiateMsg::<DefaultOptionalCollectionExtensionMsg> {
                name: class.id.clone().into(),
                symbol: class.id.clone().into(),
                minter: Some(env.contract.address.to_string()),
                creator: None,
                collection_info_extension: None,
                withdraw_address: Some(creator),
            };

        // use collection data for setting name and symbol
        let collection_data = class
            .data
            .clone()
            .and_then(|binary| from_json::<CollectionData>(binary).ok());
        if let Some(collection_data) = collection_data {
            instantiate_msg.name = collection_data.name;
            instantiate_msg.symbol = collection_data.symbol;
        }

        to_json_binary(&instantiate_msg)
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
        let nft_contract = load_nft_contract_for_class_id(deps.storage, class.id.to_string())?;
        let receiver = deps.api.addr_validate(&receiver)?;

        let token_ids_string = format!("{:?}", token_ids);
        let event = Event::new("ics721_receive_redeem_vouchers")
            .add_attribute("class_id", class.id)
            .add_attribute("nft_contract", nft_contract.clone())
            .add_attribute("token_ids", token_ids_string);

        Ok(Response::default()
            .add_attribute("method", "callback_redeem_vouchers")
            .add_event(event)
            .add_messages(
                token_ids
                    .into_iter()
                    .map(|token_id| {
                        Ok(WasmMsg::Execute {
                            contract_addr: nft_contract.to_string(),
                            msg: to_json_binary(&cw721::msg::Cw721ExecuteMsg::<
                                DefaultOptionalNftExtensionMsg,
                                DefaultOptionalCollectionExtensionMsg,
                                Empty,
                            >::TransferNft {
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
        let nft_contract =
            load_nft_contract_for_class_id(deps.as_ref().storage, class_id.to_string())?;

        let mint = tokens
            .into_iter()
            .map(|Token { id, uri, data }| {
                // Source chain may have provided token metadata, so we save token metadata here
                // Note, once cw721 doesn't support on-chain metadata yet - but this is where we will set
                // that value on the debt-voucher token once it is supported.
                // Also note that this is set for every token, regardless of if data is None.
                TOKEN_METADATA.save(deps.storage, (class_id.clone(), id.clone()), &data)?;

                let msg = cw721_base::msg::ExecuteMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::Mint {
                    token_id: id.into(),
                    token_uri: uri,
                    owner: receiver.to_string(),
                    extension: None,
                };
                Ok(WasmMsg::Execute {
                    contract_addr: nft_contract.to_string(),
                    msg: to_json_binary(&msg)?,
                    funds: vec![],
                })
            })
            .collect::<StdResult<Vec<_>>>()?;

        Ok(Response::default()
            .add_attribute("method", "callback_mint")
            .add_messages(mint))
    }

    fn callback_redeem_outgoing_channel_entries(
        &self,
        deps: DepsMut,
        entries: Vec<(ClassId, TokenId)>,
    ) -> Result<Response<T>, ContractError> {
        for (class_id, token_id) in entries {
            OUTGOING_CLASS_TOKEN_TO_CHANNEL.remove(deps.storage, (class_id, token_id));
        }
        Ok(Response::default().add_attribute("method", "callback_redeem_outgoing_channel_entries"))
    }

    fn callback_save_incoming_channel_entries(
        &self,
        deps: DepsMut,
        entries: Vec<((ClassId, TokenId), String)>,
    ) -> Result<Response<T>, ContractError> {
        for (key, channel) in entries {
            INCOMING_CLASS_TOKEN_TO_CHANNEL.save(deps.storage, key, &channel)?;
        }
        Ok(Response::default().add_attribute("method", "callback_save_incoming_channel_entries"))
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
                incoming_proxy,
                outgoing_proxy,
                cw721_base_code_id,
                cw721_admin,
                contract_addr_length,
            } => {
                // disables incoming proxy if none is provided!
                INCOMING_PROXY.save(
                    deps.storage,
                    &incoming_proxy
                        .as_ref()
                        .map(|h| deps.api.addr_validate(h))
                        .transpose()?,
                )?;
                // disables outgoing proxy if none is provided!
                OUTGOING_PROXY.save(
                    deps.storage,
                    &outgoing_proxy
                        .as_ref()
                        .map(|h| deps.api.addr_validate(h))
                        .transpose()?,
                )?;
                PO.set_pauser(deps.storage, deps.api, pauser.as_deref())?;
                if let Some(cw721_base_code_id) = cw721_base_code_id {
                    CW721_CODE_ID.save(deps.storage, &cw721_base_code_id)?;
                }
                if let Some(cw721_admin) = cw721_admin.clone() {
                    if cw721_admin.is_empty() {
                        ADMIN_USED_FOR_CW721.save(deps.storage, &None)?;
                    } else {
                        ADMIN_USED_FOR_CW721
                            .save(deps.storage, &Some(deps.api.addr_validate(&cw721_admin)?))?;
                    }
                }

                if let Some(contract_addr_length) = contract_addr_length {
                    CONTRACT_ADDR_LENGTH.save(deps.storage, &contract_addr_length)?;
                } else {
                    CONTRACT_ADDR_LENGTH.remove(deps.storage);
                }

                let response = Response::default()
                    .add_attribute("method", "migrate")
                    .add_attribute("pauser", pauser.map_or_else(|| "none".to_string(), |or| or))
                    .add_attribute(
                        "outgoing_proxy",
                        outgoing_proxy.map_or_else(|| "none".to_string(), |or| or),
                    )
                    .add_attribute(
                        "incoming_proxy",
                        incoming_proxy.map_or_else(|| "none".to_string(), |or| or),
                    )
                    .add_attribute(
                        "cw721_base_code_id",
                        cw721_base_code_id.map_or_else(|| "none".to_string(), |or| or.to_string()),
                    )
                    .add_attribute(
                        "cw721_admin",
                        cw721_admin.map_or_else(
                            || "none".to_string(),
                            |or| {
                                if or.is_empty() {
                                    "immutable".to_string()
                                } else {
                                    or
                                }
                            },
                        ),
                    )
                    .add_attribute(
                        "contract_addr_length",
                        contract_addr_length
                            .map_or_else(|| "none".to_string(), |or| or.to_string()),
                    );

                self.migrate_legacy(deps, response)
            }
        }
    }

    // TODO once migrated:
    // - this complete block can be deleted
    // - legacy map 'e' and 'f' can be deleted
    fn migrate_legacy(
        &self,
        deps: DepsMut,
        response: Response<T>,
    ) -> Result<Response<T>, ContractError> {
        // we migrate only in case CLASS_ID_AND_NFT_CONTRACT_INFO is not populated yet
        let is_empty = query_nft_contracts(deps.as_ref(), None, None)
            .map(|nft_contracts| nft_contracts.is_empty())?;
        if is_empty {
            // - get legacy map and migrate it to new indexed map
            let legacy_nft_contract_to_class_id: Map<Addr, ClassId> = Map::new("f");
            match cw_paginate_storage::paginate_map(
                deps.as_ref(),
                &legacy_nft_contract_to_class_id,
                None,
                None,
                Order::Ascending,
            ) {
                Ok(nft_contract_and_class_id) => {
                    let response = response.add_attribute(
                        "migrated nft contracts",
                        nft_contract_and_class_id.len().to_string(),
                    );
                    for (nft_contract, class_id) in nft_contract_and_class_id {
                        let class_id_info = ClassIdInfo {
                            class_id: class_id.clone(),
                            address: nft_contract.clone(),
                        };
                        CLASS_ID_AND_NFT_CONTRACT_INFO.save(
                            deps.storage,
                            &class_id,
                            &class_id_info,
                        )?;
                    }
                    Ok(response)
                }
                Err(err) => Err(ContractError::Std(err)),
            }
        } else {
            Ok(response)
        }
    }
}
