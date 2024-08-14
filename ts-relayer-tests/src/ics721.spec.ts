import { CosmWasmSigner } from "@confio/relayer";
import { fromUtf8 } from "@cosmjs/encoding";
import anyTest, { ExecutionContext, TestFn } from "ava";
import { Order } from "cosmjs-types/ibc/core/channel/v1/channel";

import { instantiateContract } from "./controller";
import {
  allTokens,
  approve,
  getCw721CollectionInfoAndExtension,
  getCw721ContractInfo_v16,
  getCw721CreatorOwnership,
  getCw721Minter_v16,
  getCw721MinterOwnership,
  mint,
  nftInfo,
  ownerOf,
  sendNft,
} from "./cw721-utils";
import {
  adminCleanAndBurnNft,
  adminCleanAndUnescrowNft,
  incomingChannels,
  migrate,
  migrateIncomingProxy,
  nftContracts,
  outgoingChannels,
} from "./ics721-utils";
import {
  assertAckErrors,
  assertAckSuccess,
  bigIntReplacer,
  ChannelAndLinkInfo,
  ContractMsg,
  createIbcConnectionAndChannel,
  MNEMONIC,
  setupOsmosisClient,
  setupWasmClient,
  uploadAndInstantiate,
  uploadAndInstantiateAll,
} from "./utils";

interface TestContext {
  wasmClient: CosmWasmSigner;
  wasmAddr: string;

  osmoClient: CosmWasmSigner;
  osmoAddr: string;

  wasmCw721: string;
  wasmCw721_v16: string;
  wasmIcs721: string;
  wasmCw721IncomingProxyId: number;
  wasmCw721IncomingProxy: string;
  wasmCw721OutgoingProxy: string;

  osmoCw721: string;
  osmoCw721_v16: string;
  osmoIcs721: string;
  osmoCw721IncomingProxy: string;
  osmoCw721OutgoingProxy: string;

  channel: ChannelAndLinkInfo;
  onlyOsmoIncomingChannel: ChannelAndLinkInfo; // this channel is WLed only in incoming proxy on osmo side
  otherChannel: ChannelAndLinkInfo;
}

const test = anyTest as TestFn<TestContext>;

const WASM_FILE_CW721 = "./internal/cw721_metadata_onchain_v0.19.0.wasm";
const WASM_FILE_CW721_v16 = "./internal/cw721_metadata_onchain_v0.16.0.wasm";
const WASM_FILE_CW721_INCOMING_PROXY = "./internal/cw721_incoming_proxy.wasm";
const WASM_FILE_CW721_OUTGOING_PROXY =
  "./internal/cw721_outgoing_proxy_rate_limit.wasm";
const WASM_FILE_CW_ICS721_ICS721 = "./internal/ics721_base.wasm";
const MALICIOUS_CW721 = "./internal/cw721_tester.wasm";

const standardSetup = async (t: ExecutionContext<TestContext>) => {
  t.context.wasmClient = await setupWasmClient(MNEMONIC);
  t.context.osmoClient = await setupOsmosisClient(MNEMONIC);

  t.context.wasmAddr = t.context.wasmClient.senderAddress;
  t.context.osmoAddr = t.context.osmoClient.senderAddress;

  const { wasmClient, osmoClient } = t.context;

  const wasmContracts: Record<string, ContractMsg> = {
    cw721: {
      path: WASM_FILE_CW721,
      instantiateMsg: {
        name: "ark wasm",
        symbol: "ark wasm",
        collection_info_extension: {
          description: "description",
          image: "https://ark.pass/image.png",
          external_link: "https://interchain.arkprotocol.io",
          explicit_content: false,
          royalty_info: {
            payment_address: wasmClient.senderAddress,
            share: "0.1",
          },
        },
        creator: wasmClient.senderAddress,
        minter: wasmClient.senderAddress,
        withdraw_address: wasmClient.senderAddress,
      },
    },
    cw721_v16: {
      path: WASM_FILE_CW721_v16,
      instantiateMsg: {
        name: "ark wasm",
        symbol: "ark wasm",
        minter: wasmClient.senderAddress,
      },
    },
    cw721IncomingProxy: {
      path: WASM_FILE_CW721_INCOMING_PROXY,
      instantiateMsg: undefined,
    },
    cw721OutgoingProxy: {
      path: WASM_FILE_CW721_OUTGOING_PROXY,
      instantiateMsg: undefined,
    },
    ics721: {
      path: WASM_FILE_CW_ICS721_ICS721,
      instantiateMsg: undefined,
    },
  };

  const osmoContracts: Record<string, ContractMsg> = {
    cw721: {
      path: WASM_FILE_CW721,
      instantiateMsg: {
        name: "ark osmo",
        symbol: "ark osmo",
        collection_info_extension: {
          description: "description",
          image: "https://ark.pass/image.png",
          external_link: "https://interchain.arkprotocol.io",
          explicit_content: false,
          royalty_info: {
            payment_address: osmoClient.senderAddress,
            share: "0.1",
          },
        },
        creator: osmoClient.senderAddress,
        minter: osmoClient.senderAddress,
        withdraw_address: osmoClient.senderAddress,
      },
    },
    cw721_v16: {
      path: WASM_FILE_CW721_v16,
      instantiateMsg: {
        name: "ark osmo",
        symbol: "ark osmo",
        minter: osmoClient.senderAddress,
      },
    },
    cw721IncomingProxy: {
      path: WASM_FILE_CW721_INCOMING_PROXY,
      instantiateMsg: undefined,
    },
    cw721OutgoingProxy: {
      path: WASM_FILE_CW721_OUTGOING_PROXY,
      instantiateMsg: undefined,
    },
    ics721: {
      path: WASM_FILE_CW_ICS721_ICS721,
      instantiateMsg: undefined,
    },
  };

  const info = await uploadAndInstantiateAll(
    wasmClient,
    osmoClient,
    wasmContracts,
    osmoContracts
  );

  const wasmCw721Id = info.wasmContractInfos.cw721.codeId;
  const osmoCw721Id = info.osmoContractInfos.cw721.codeId;

  const wasmCw721IncomingProxyId =
    info.wasmContractInfos.cw721IncomingProxy.codeId;
  t.context.wasmCw721IncomingProxyId = wasmCw721IncomingProxyId;
  const osmoCw721IncomingProxyId =
    info.osmoContractInfos.cw721IncomingProxy.codeId;

  const wasmIcs721Id = info.wasmContractInfos.ics721.codeId;
  const osmoIcs721Id = info.osmoContractInfos.ics721.codeId;

  const wasmCw721OutgoingProxyId =
    info.wasmContractInfos.cw721OutgoingProxy.codeId;
  const osmoCw721OutgoingProxyId =
    info.osmoContractInfos.cw721OutgoingProxy.codeId;

  t.context.wasmCw721 = info.wasmContractInfos.cw721.address as string;
  t.context.wasmCw721_v16 = info.wasmContractInfos.cw721_v16.address as string;
  t.context.osmoCw721 = info.osmoContractInfos.cw721.address as string;
  t.context.osmoCw721_v16 = info.osmoContractInfos.cw721_v16.address as string;

  t.log(`instantiating wasm ICS721 contract (${wasmIcs721Id})`);
  const { contractAddress: wasmIcs721 } = await instantiateContract(
    wasmClient,
    wasmIcs721Id,
    {
      cw721_base_code_id: wasmCw721Id,
      cw721_admin: wasmClient.senderAddress, // set cw721 admin, this will be also used as creator and payment address for escrowed cw721 by ics721
    },
    "label ics721"
  );
  t.log(`- wasm ICS721 contract address: ${wasmIcs721}`);
  t.context.wasmIcs721 = wasmIcs721;

  t.log(`instantiating osmo ICS721 contract (${osmoIcs721Id})`);
  const { contractAddress: osmoIcs721 } = await instantiateContract(
    osmoClient,
    osmoIcs721Id,
    {
      cw721_base_code_id: osmoCw721Id,
      cw721_admin: osmoClient.senderAddress, // set cw721 admin, this will be also used as creator and payment address for escrowed cw721 by ics721
    },
    "label ics721"
  );
  t.log(`- osmo ICS721 contract address: ${osmoIcs721}`);
  t.context.osmoIcs721 = osmoIcs721;

  t.log(
    `creating IBC connection and channel between ${wasmIcs721} <-> ${osmoIcs721}`
  );
  const channelInfo = await createIbcConnectionAndChannel(
    wasmClient,
    osmoClient,
    wasmIcs721,
    osmoIcs721,
    Order.ORDER_UNORDERED,
    "ics721-1"
  );
  t.log(
    `- channel for incoming proxy on both chains: ${JSON.stringify(
      channelInfo.channel,
      bigIntReplacer,
      2
    )}`
  );
  t.context.channel = channelInfo;

  t.log(
    `instantiating wasm cw721-incoming-proxy (${wasmCw721IncomingProxyId}) with channel ${channelInfo.channel.src.channelId}`
  );
  const { contractAddress: wasmCw721IncomingProxy } = await instantiateContract(
    wasmClient,
    wasmCw721IncomingProxyId,
    {
      origin: wasmIcs721,
      channels: [channelInfo.channel.src.channelId],
    },
    "label incoming proxy"
  );
  t.log(`- wasm cw721-incoming-proxy address: ${wasmCw721IncomingProxy}`);
  t.context.wasmCw721IncomingProxy = wasmCw721IncomingProxy;

  t.log(
    `migrate ${wasmIcs721} contract with incoming proxy ${wasmCw721IncomingProxy}`
  );
  await migrate(wasmClient, wasmIcs721, wasmIcs721Id, wasmCw721IncomingProxy);

  const onlyOsmoIncomingChannelInfo = await createIbcConnectionAndChannel(
    wasmClient,
    osmoClient,
    wasmIcs721,
    osmoIcs721,
    Order.ORDER_UNORDERED,
    "ics721-1"
  );
  t.log(
    `- channel for incoming proxy only on wasm chain: ${JSON.stringify(
      onlyOsmoIncomingChannelInfo.channel,
      bigIntReplacer,
      2
    )}`
  );
  t.context.onlyOsmoIncomingChannel = onlyOsmoIncomingChannelInfo;

  t.log(
    `instantiating osmo cw721-incoming-proxy (${osmoCw721IncomingProxyId}) with channel ${channelInfo.channel.dest.channelId}and ${onlyOsmoIncomingChannelInfo.channel.dest.channelId}`
  );
  const { contractAddress: osmoCw721IncomingProxy } = await instantiateContract(
    osmoClient,
    osmoCw721IncomingProxyId,
    {
      origin: osmoIcs721,
      channels: [
        channelInfo.channel.dest.channelId,
        onlyOsmoIncomingChannelInfo.channel.dest.channelId,
      ],
    },
    "label incoming proxy"
  );
  t.log(`- osmo cw721-incoming-proxy address: ${osmoCw721IncomingProxy}`);
  t.context.osmoCw721IncomingProxy = osmoCw721IncomingProxy;

  const per_block = 10; // use high rate limit to avoid test failures
  t.log(
    `instantiating wasm cw721-outgoing-proxy (${wasmCw721OutgoingProxyId}) with ${per_block} per blocks rate limit`
  );
  const { contractAddress: wasmCw721OutgoingProxy } = await instantiateContract(
    wasmClient,
    wasmCw721OutgoingProxyId,
    {
      origin: wasmIcs721,
      rate_limit: { per_block },
    },
    "label outgoing proxy"
  );
  t.log(`- wasm cw721-outgoing-proxy address: ${wasmCw721OutgoingProxy}`);
  t.context.wasmCw721OutgoingProxy = wasmCw721OutgoingProxy;

  t.log(
    `instantiating osmo cw721-outgoing-proxy (${osmoCw721OutgoingProxyId}) with ${per_block} per blocks rate limit`
  );
  const { contractAddress: osmoCw721OutgoingProxy } = await instantiateContract(
    osmoClient,
    osmoCw721OutgoingProxyId,
    {
      origin: osmoIcs721,
      rate_limit: { per_block },
    },
    "label outgoing proxy"
  );
  t.log(`- osmo cw721-outgoing-proxy address: ${osmoCw721OutgoingProxy}`);
  t.context.osmoCw721OutgoingProxy = osmoCw721OutgoingProxy;

  t.log(
    `migrate ${wasmIcs721} contract with incoming (${wasmCw721IncomingProxy}) and outgoing proxy (${wasmCw721OutgoingProxy})`
  );
  await migrate(
    wasmClient,
    wasmIcs721,
    wasmIcs721Id,
    wasmCw721IncomingProxy,
    wasmCw721OutgoingProxy
  );

  t.log(
    `migrate ${osmoIcs721} contract with incoming (${osmoCw721IncomingProxy}) and outgoing proxy (${osmoCw721OutgoingProxy})`
  );
  await migrate(
    osmoClient,
    osmoIcs721,
    osmoIcs721Id,
    osmoCw721IncomingProxy,
    osmoCw721OutgoingProxy
  );

  t.log(
    `creating another IBC connection and channel between wasm and osmo (${wasmIcs721} <-> ${osmoIcs721})`
  );
  const otherChannelInfo = await createIbcConnectionAndChannel(
    wasmClient,
    osmoClient,
    wasmIcs721,
    osmoIcs721,
    Order.ORDER_UNORDERED,
    "ics721-1"
  );
  t.log(
    `- other channel not WLed for incoming proxy: ${JSON.stringify(
      otherChannelInfo.channel,
      bigIntReplacer,
      2
    )}`
  );
  t.context.otherChannel = otherChannelInfo;

  t.pass();
};

test.serial("transfer NFT: wasmd -> osmo", async (t) => {
  await standardSetup(t);

  const {
    wasmClient,
    wasmAddr,
    wasmCw721,
    wasmIcs721,
    wasmCw721IncomingProxyId,
    wasmCw721IncomingProxy,
    wasmCw721OutgoingProxy,
    osmoClient,
    osmoAddr,
    osmoIcs721,
    channel,
    otherChannel,
    onlyOsmoIncomingChannel,
  } = t.context;

  let tokenId = "1";
  await mint(wasmClient, wasmCw721, tokenId, wasmAddr, undefined);
  // assert token is minted
  let tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

  // ==== happy path: transfer NFT to osmo chain and back to wasm chain ====
  // test transfer NFT to osmo chain
  t.log(`transfering to osmo chain via ${channel.channel.src.channelId}`);
  let ibcMsg = {
    receiver: osmoAddr,
    channel_id: channel.channel.src.channelId,
    timeout: {
      block: {
        revision: 1,
        height: 90000,
      },
    },
  };
  let transferResponse = await sendNft(
    wasmClient,
    wasmCw721,
    wasmCw721OutgoingProxy,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);

  // Relay and verify we got a success
  t.log("relaying packets");
  let info = await channel.link.relayAll();
  assertAckSuccess(info.acksFromA);

  // assert NFT on chain A is locked/owned by ICS contract
  tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmIcs721, tokenOwner.owner);
  // assert NFT minted on chain B
  let osmoClassId = `${channel.channel.dest.portId}/${channel.channel.dest.channelId}/${wasmCw721}`;
  let osmoCw721 = await osmoClient.sign.queryContractSmart(osmoIcs721, {
    nft_contract: { class_id: osmoClassId },
  });
  let allNFTs = await allTokens(osmoClient, osmoCw721);
  t.true(allNFTs.tokens.length === 1);
  // assert NFT on chain B is owned by osmoAddr
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);
  // assert escrowed collection data on chain B is properly set
  const wasmCollectionData = await getCw721CollectionInfoAndExtension(
    wasmClient,
    wasmCw721
  );
  const osmoCollectionData = await getCw721CollectionInfoAndExtension(
    osmoClient,
    osmoCw721
  );
  // osmoCollectionData.updated_at = wasmCollectionData.updated_at; // ignore updated_at
  if (wasmCollectionData.extension?.royalty_info?.payment_address) {
    wasmCollectionData.extension.royalty_info.payment_address = osmoAddr; // osmo cw721 admin address is used as payment address
  }
  t.deepEqual(
    wasmCollectionData,
    osmoCollectionData,
    `collection data must match: ${JSON.stringify(
      wasmCollectionData
    )} vs ${JSON.stringify(osmoCollectionData)}`
  );
  // assert creator is set to osmoAddr
  const creatorOwnerShip = await getCw721CreatorOwnership(
    osmoClient,
    osmoCw721
  );
  t.is(osmoAddr, creatorOwnerShip.owner);
  // assert minter is set to ics721
  const minterOwnerShip = await getCw721MinterOwnership(osmoClient, osmoCw721);
  t.is(osmoIcs721, minterOwnerShip.owner);

  const wasmNftInfo = await nftInfo(wasmClient, wasmCw721, tokenId);
  // assert extension is not null
  t.truthy(wasmNftInfo.extension);
  const osmoNftInfo = await nftInfo(osmoClient, osmoCw721, tokenId);
  t.truthy(osmoNftInfo.extension);
  // assert nft with extension is same
  t.deepEqual(wasmNftInfo, osmoNftInfo);

  // test back transfer NFT to wasm chain
  t.log(`transfering back to wasm chain via ${channel.channel.dest.channelId}`);
  transferResponse = await sendNft(
    osmoClient,
    osmoCw721,
    t.context.osmoCw721OutgoingProxy,
    {
      receiver: wasmAddr,
      channel_id: channel.channel.dest.channelId,
      timeout: {
        block: {
          revision: 1,
          height: 90000,
        },
      },
    },
    tokenId
  );
  t.truthy(transferResponse);
  t.log("relaying packets");

  // Verify we got a success
  info = await channel.link.relayAll();
  assertAckSuccess(info.acksFromA);

  // assert NFT burned on chain B
  allNFTs = await allTokens(osmoClient, osmoCw721);
  t.true(allNFTs.tokens.length === 0);
  // assert NFT on chain A is returned to owner
  tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

  // ==== test transfer NFT to osmo chain via unknown, not WLed channel by incoming proxy ====
  // test rejected NFT transfer due to unknown channel by incoming proxy
  tokenId = "2";
  await mint(wasmClient, wasmCw721, tokenId, wasmAddr, undefined);
  // assert token is minted
  tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

  t.log(
    `transfering to osmo chain via unknown ${otherChannel.channel.src.channelId}`
  );
  const beforeWasmOutgoingClassTokenToChannelList = await outgoingChannels(
    wasmClient,
    wasmIcs721
  );
  const beforeWasmIncomingClassTokenToChannelList = await incomingChannels(
    wasmClient,
    wasmIcs721
  );
  const beforeWasmNftContractsToClassIdList = await nftContracts(
    wasmClient,
    wasmIcs721
  );
  const beforeOsmoOutgoingClassTokenToChannelList = await outgoingChannels(
    osmoClient,
    osmoIcs721
  );
  const beforeOsmoIncomingClassTokenToChannelList = await incomingChannels(
    osmoClient,
    osmoIcs721
  );
  const beforeOsmoNftContractsToClassIdList = await nftContracts(
    osmoClient,
    osmoIcs721
  );

  ibcMsg = {
    receiver: osmoAddr,
    channel_id: otherChannel.channel.src.channelId,
    timeout: {
      block: {
        revision: 1,
        height: 90000,
      },
    },
  };
  transferResponse = await sendNft(
    wasmClient,
    wasmCw721,
    wasmCw721OutgoingProxy,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);

  // Relay and verify we got an error
  t.log("relaying packets");
  info = await otherChannel.link.relayAll();
  assertAckErrors(info.acksFromA);
  // assert no change before and after relay
  const afterWasmOutgoingClassTokenToChannelList = await outgoingChannels(
    wasmClient,
    wasmIcs721
  );
  const afterWasmIncomingClassTokenToChannelList = await incomingChannels(
    wasmClient,
    wasmIcs721
  );
  const afterWasmNftContractsToClassIdList = await nftContracts(
    wasmClient,
    wasmIcs721
  );
  t.deepEqual(
    beforeWasmOutgoingClassTokenToChannelList,
    afterWasmOutgoingClassTokenToChannelList,
    `outgoing channels must be unchanged:
- wasm before: ${JSON.stringify(beforeWasmOutgoingClassTokenToChannelList)}
- wasm after: ${JSON.stringify(afterWasmOutgoingClassTokenToChannelList)}`
  );
  t.deepEqual(
    beforeWasmIncomingClassTokenToChannelList,
    afterWasmIncomingClassTokenToChannelList,
    `incoming channels must be unchanged:
- wasm before: ${JSON.stringify(beforeWasmIncomingClassTokenToChannelList)}
- wasm after: ${JSON.stringify(afterWasmIncomingClassTokenToChannelList)}`
  );
  t.deepEqual(
    beforeWasmNftContractsToClassIdList,
    afterWasmNftContractsToClassIdList,
    `nft contracts must be unchanged:
- wasm before: ${JSON.stringify(beforeWasmNftContractsToClassIdList)}
- wasm after: ${JSON.stringify(afterWasmNftContractsToClassIdList)}`
  );
  const afterOsmoOutgoingClassTokenToChannelList = await outgoingChannels(
    osmoClient,
    osmoIcs721
  );
  const afterOsmoIncomingClassTokenToChannelList = await incomingChannels(
    osmoClient,
    osmoIcs721
  );
  const afterOsmoNftContractsToClassIdList = await nftContracts(
    osmoClient,
    osmoIcs721
  );
  t.deepEqual(
    beforeOsmoOutgoingClassTokenToChannelList,
    afterOsmoOutgoingClassTokenToChannelList,
    `outgoing channels must be unchanged:
- osmo before: ${JSON.stringify(beforeOsmoOutgoingClassTokenToChannelList)}
- osmo after: ${JSON.stringify(afterOsmoOutgoingClassTokenToChannelList)}`
  );
  t.deepEqual(
    beforeOsmoIncomingClassTokenToChannelList,
    afterOsmoIncomingClassTokenToChannelList,
    `incoming channels must be unchanged:
- osmo before: ${JSON.stringify(beforeOsmoIncomingClassTokenToChannelList)}
- osmo after: ${JSON.stringify(afterOsmoIncomingClassTokenToChannelList)}`
  );
  t.deepEqual(
    beforeOsmoNftContractsToClassIdList,
    afterOsmoNftContractsToClassIdList,
    `nft contracts must be unchanged:
- osmo before: ${JSON.stringify(beforeOsmoNftContractsToClassIdList)}
- osmo after: ${JSON.stringify(afterOsmoNftContractsToClassIdList)}`
  );

  // assert NFT on chain A is returned to owner
  tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

  // ==== test transfer NFT to osmo chain via channel WLed ONLY on osmo incoming proxy and back to wasm chain ====
  tokenId = "3";
  await mint(wasmClient, wasmCw721, tokenId, wasmAddr, undefined);
  // assert token is minted
  tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

  // test transfer NFT to osmo chain
  t.log(
    `transfering to osmo chain via ${onlyOsmoIncomingChannel.channel.src.channelId}`
  );
  ibcMsg = {
    receiver: osmoAddr,
    channel_id: onlyOsmoIncomingChannel.channel.src.channelId,
    timeout: {
      block: {
        revision: 1,
        height: 90000,
      },
    },
  };
  transferResponse = await sendNft(
    wasmClient,
    wasmCw721,
    wasmCw721OutgoingProxy,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);

  // Relay and verify we got a success
  t.log("relaying packets");
  info = await onlyOsmoIncomingChannel.link.relayAll();
  assertAckSuccess(info.acksFromA);

  // assert 1 entry for outgoing channels
  let wasmOutgoingClassTokenToChannelList = await outgoingChannels(
    wasmClient,
    wasmIcs721
  );
  t.log(
    `- outgoing channels: ${JSON.stringify(
      wasmOutgoingClassTokenToChannelList
    )}`
  );
  t.true(
    wasmOutgoingClassTokenToChannelList.length === 1,
    `outgoing channels must have one entry: ${JSON.stringify(
      wasmOutgoingClassTokenToChannelList
    )}`
  );

  // assert NFT minted on chain B
  osmoClassId = `${onlyOsmoIncomingChannel.channel.dest.portId}/${onlyOsmoIncomingChannel.channel.dest.channelId}/${wasmCw721}`;
  osmoCw721 = await osmoClient.sign.queryContractSmart(osmoIcs721, {
    nft_contract: { class_id: osmoClassId },
  });
  allNFTs = await allTokens(osmoClient, osmoCw721);
  t.true(allNFTs.tokens.length === 1);
  // assert NFT on chain B is owned by osmoAddr
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);
  // assert NFT on chain A is locked/owned by ICS contract
  tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmIcs721, tokenOwner.owner);
  // assert NFT on chain B is owned by osmoAddr
  osmoClassId = `${onlyOsmoIncomingChannel.channel.dest.portId}/${onlyOsmoIncomingChannel.channel.dest.channelId}/${wasmCw721}`;
  osmoCw721 = await osmoClient.sign.queryContractSmart(osmoIcs721, {
    nft_contract: { class_id: osmoClassId },
  });
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);

  // test back transfer NFT to wasm chain, where onlyOsmoIncomingChannel is not WLed on wasm chain
  t.log(
    `transfering back to wasm chain via unknown ${onlyOsmoIncomingChannel.channel.dest.channelId}`
  );
  transferResponse = await sendNft(
    osmoClient,
    osmoCw721,
    t.context.osmoCw721OutgoingProxy,
    {
      receiver: wasmAddr,
      channel_id: onlyOsmoIncomingChannel.channel.dest.channelId,
      timeout: {
        block: {
          revision: 1,
          height: 90000,
        },
      },
    },
    tokenId
  );
  t.truthy(transferResponse);
  // before relay NFT escrowed by ICS721
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoIcs721, tokenOwner.owner);

  // Relay and verify we got an error
  t.log("relaying packets");
  info = await onlyOsmoIncomingChannel.link.relayAll();
  for (const ack of info.acksFromB) {
    const parsed = JSON.parse(fromUtf8(ack.acknowledgement));
    t.log(`- ack: ${JSON.stringify(parsed)}`);
  }
  assertAckErrors(info.acksFromB);

  // assert after failed relay, NFT on chain B is returned to owner
  allNFTs = await allTokens(osmoClient, osmoCw721);
  t.true(allNFTs.tokens.length === 1);
  // assert NFT is returned to sender on osmo chain
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);

  // ==== WL channel on wasm chain and test back transfer again ====
  t.log(
    `migrate ${wasmCw721IncomingProxy} contract and add channel ${onlyOsmoIncomingChannel.channel.src.channelId}`
  );
  await migrateIncomingProxy(
    wasmClient,
    wasmCw721IncomingProxy,
    wasmCw721IncomingProxyId,
    [
      channel.channel.src.channelId,
      onlyOsmoIncomingChannel.channel.src.channelId,
    ]
  );

  // test back transfer NFT to wasm chain, where onlyOsmoIncomingChannel is not WLed on wasm chain
  t.log(
    `transfering back to wasm chain via WLed ${onlyOsmoIncomingChannel.channel.dest.channelId}`
  );
  transferResponse = await sendNft(
    osmoClient,
    osmoCw721,
    t.context.osmoCw721OutgoingProxy,
    {
      receiver: wasmAddr,
      channel_id: onlyOsmoIncomingChannel.channel.dest.channelId,
      timeout: {
        block: {
          revision: 1,
          height: 90000,
        },
      },
    },
    tokenId
  );
  t.truthy(transferResponse);
  // before relay NFT escrowed by ICS721
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoIcs721, tokenOwner.owner);

  allNFTs = await allTokens(osmoClient, osmoCw721);
  t.log(`- all tokens: ${JSON.stringify(allNFTs)}`);

  // query nft contracts
  let nftContractsToClassIdList = await nftContracts(wasmClient, wasmIcs721);
  t.log(`- nft contracts: ${JSON.stringify(nftContractsToClassIdList)}`);
  t.true(
    nftContractsToClassIdList.length === 1,
    `nft contracts must have exactly one entry: ${JSON.stringify(
      nftContractsToClassIdList
    )}`
  );

  // Relay and verify success
  t.log("relaying packets");
  info = await onlyOsmoIncomingChannel.link.relayAll();
  for (const ack of info.acksFromB) {
    const parsed = JSON.parse(fromUtf8(ack.acknowledgement));
    t.log(`- ack: ${JSON.stringify(parsed)}`);
  }
  assertAckSuccess(info.acksFromB);

  // assert outgoing channels is empty
  wasmOutgoingClassTokenToChannelList = await outgoingChannels(
    wasmClient,
    wasmIcs721
  );
  t.true(
    wasmOutgoingClassTokenToChannelList.length === 0,
    `outgoing channels not empty: ${JSON.stringify(
      wasmOutgoingClassTokenToChannelList
    )}`
  );

  // assert after success relay, NFT on chain B is burned
  allNFTs = await allTokens(osmoClient, osmoCw721);
  t.log(`- all tokens: ${JSON.stringify(allNFTs)}`);
  t.true(allNFTs.tokens.length === 0);
  // assert list is unchanged
  nftContractsToClassIdList = await nftContracts(wasmClient, wasmIcs721);
  t.log(`- nft contracts: ${JSON.stringify(nftContractsToClassIdList)}`);
  t.true(
    nftContractsToClassIdList.length === 1,
    `nft contracts must have exactly one entry: ${JSON.stringify(
      nftContractsToClassIdList
    )}`
  );
  // assert NFT is returned to sender on wasm chain
  tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmAddr, tokenOwner.owner);
});

test.serial("transfer NFT v16: wasm -> osmo", async (t) => {
  await standardSetup(t);

  const {
    wasmClient,
    wasmAddr,
    wasmCw721_v16,
    wasmIcs721,
    wasmCw721IncomingProxyId,
    wasmCw721IncomingProxy,
    wasmCw721OutgoingProxy,
    osmoClient,
    osmoAddr,
    osmoIcs721,
    channel,
    otherChannel,
    onlyOsmoIncomingChannel,
  } = t.context;

  let tokenId = "1";
  await mint(wasmClient, wasmCw721_v16, tokenId, wasmAddr, undefined);
  // assert token is minted
  let tokenOwner = await ownerOf(wasmClient, wasmCw721_v16, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

  // ==== happy path: transfer NFT to osmo chain and back to wasm chain ====
  // test transfer NFT to osmo chain
  t.log(`transfering to osmo chain via ${channel.channel.src.channelId}`);
  let ibcMsg = {
    receiver: osmoAddr,
    channel_id: channel.channel.src.channelId,
    timeout: {
      block: {
        revision: 1,
        height: 90000,
      },
    },
  };
  let transferResponse = await sendNft(
    wasmClient,
    wasmCw721_v16,
    wasmCw721OutgoingProxy,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);

  // Relay and verify we got a success
  t.log("relaying packets");
  let info = await channel.link.relayAll();
  assertAckSuccess(info.acksFromA);

  // assert NFT on chain A is locked/owned by ICS contract
  tokenOwner = await ownerOf(wasmClient, wasmCw721_v16, tokenId);
  t.is(wasmIcs721, tokenOwner.owner);
  // assert NFT minted on chain B
  let osmoClassId = `${channel.channel.dest.portId}/${channel.channel.dest.channelId}/${wasmCw721_v16}`;
  let osmoCw721 = await osmoClient.sign.queryContractSmart(osmoIcs721, {
    nft_contract: { class_id: osmoClassId },
  });
  let allNFTs = await allTokens(osmoClient, osmoCw721);
  t.true(allNFTs.tokens.length === 1);
  // assert NFT on chain B is owned by osmoAddr
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);
  // assert escrowed collection data on chain B is properly set
  // wasm collection data holds only name and symbol
  const wasmCollectionData = await getCw721ContractInfo_v16(
    wasmClient,
    wasmCw721_v16
  );
  const osmoCollectionData = await getCw721CollectionInfoAndExtension(
    osmoClient,
    osmoCw721
  );
  t.deepEqual(
    wasmCollectionData.name,
    osmoCollectionData.name,
    `name must match: ${wasmCollectionData.name} vs ${osmoCollectionData.name}`
  );
  t.deepEqual(
    wasmCollectionData.symbol,
    osmoCollectionData.symbol,
    `symbol must match: ${wasmCollectionData.symbol} vs ${osmoCollectionData.symbol}`
  );
  // assert minter is set to ics721
  const minterOwnerShip = await getCw721Minter_v16(osmoClient, osmoCw721);
  t.is(osmoIcs721, minterOwnerShip.minter);

  const wasmNftInfo = await nftInfo(wasmClient, wasmCw721_v16, tokenId);
  // assert extension is not null
  t.truthy(wasmNftInfo.extension);
  const osmoNftInfo = await nftInfo(osmoClient, osmoCw721, tokenId);
  t.truthy(osmoNftInfo.extension);
  // assert nft with extension is same
  t.deepEqual(wasmNftInfo, osmoNftInfo);

  // test back transfer NFT to wasm chain
  t.log(`transfering back to wasm chain via ${channel.channel.dest.channelId}`);
  transferResponse = await sendNft(
    osmoClient,
    osmoCw721,
    t.context.osmoCw721OutgoingProxy,
    {
      receiver: wasmAddr,
      channel_id: channel.channel.dest.channelId,
      timeout: {
        block: {
          revision: 1,
          height: 90000,
        },
      },
    },
    tokenId
  );
  t.truthy(transferResponse);
  t.log("relaying packets");

  // Verify we got a success
  info = await channel.link.relayAll();
  assertAckSuccess(info.acksFromA);

  // assert NFT burned on chain B
  allNFTs = await allTokens(osmoClient, osmoCw721);
  t.true(allNFTs.tokens.length === 0);
  // assert NFT on chain A is returned to owner
  tokenOwner = await ownerOf(wasmClient, wasmCw721_v16, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

  // ==== test transfer NFT to osmo chain via unknown, not WLed channel by incoming proxy ====
  // test rejected NFT transfer due to unknown channel by incoming proxy
  tokenId = "2";
  await mint(wasmClient, wasmCw721_v16, tokenId, wasmAddr, undefined);
  // assert token is minted
  tokenOwner = await ownerOf(wasmClient, wasmCw721_v16, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

  t.log(
    `transfering to osmo chain via unknown ${otherChannel.channel.src.channelId}`
  );
  const beforeWasmOutgoingClassTokenToChannelList = await outgoingChannels(
    wasmClient,
    wasmIcs721
  );
  const beforeWasmIncomingClassTokenToChannelList = await incomingChannels(
    wasmClient,
    wasmIcs721
  );
  const beforeWasmNftContractsToClassIdList = await nftContracts(
    wasmClient,
    wasmIcs721
  );
  const beforeOsmoOutgoingClassTokenToChannelList = await outgoingChannels(
    osmoClient,
    osmoIcs721
  );
  const beforeOsmoIncomingClassTokenToChannelList = await incomingChannels(
    osmoClient,
    osmoIcs721
  );
  const beforeOsmoNftContractsToClassIdList = await nftContracts(
    osmoClient,
    osmoIcs721
  );

  ibcMsg = {
    receiver: osmoAddr,
    channel_id: otherChannel.channel.src.channelId,
    timeout: {
      block: {
        revision: 1,
        height: 90000,
      },
    },
  };
  transferResponse = await sendNft(
    wasmClient,
    wasmCw721_v16,
    wasmCw721OutgoingProxy,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);

  // Relay and verify we got an error
  t.log("relaying packets");
  info = await otherChannel.link.relayAll();
  assertAckErrors(info.acksFromA);
  // assert no change before and after relay
  const afterWasmOutgoingClassTokenToChannelList = await outgoingChannels(
    wasmClient,
    wasmIcs721
  );
  const afterWasmIncomingClassTokenToChannelList = await incomingChannels(
    wasmClient,
    wasmIcs721
  );
  const afterWasmNftContractsToClassIdList = await nftContracts(
    wasmClient,
    wasmIcs721
  );
  t.deepEqual(
    beforeWasmOutgoingClassTokenToChannelList,
    afterWasmOutgoingClassTokenToChannelList,
    `outgoing channels must be unchanged:
- wasm before: ${JSON.stringify(beforeWasmOutgoingClassTokenToChannelList)}
- wasm after: ${JSON.stringify(afterWasmOutgoingClassTokenToChannelList)}`
  );
  t.deepEqual(
    beforeWasmIncomingClassTokenToChannelList,
    afterWasmIncomingClassTokenToChannelList,
    `incoming channels must be unchanged:
- wasm before: ${JSON.stringify(beforeWasmIncomingClassTokenToChannelList)}
- wasm after: ${JSON.stringify(afterWasmIncomingClassTokenToChannelList)}`
  );
  t.deepEqual(
    beforeWasmNftContractsToClassIdList,
    afterWasmNftContractsToClassIdList,
    `nft contracts must be unchanged:
- wasm before: ${JSON.stringify(beforeWasmNftContractsToClassIdList)}
- wasm after: ${JSON.stringify(afterWasmNftContractsToClassIdList)}`
  );
  const afterOsmoOutgoingClassTokenToChannelList = await outgoingChannels(
    osmoClient,
    osmoIcs721
  );
  const afterOsmoIncomingClassTokenToChannelList = await incomingChannels(
    osmoClient,
    osmoIcs721
  );
  const afterOsmoNftContractsToClassIdList = await nftContracts(
    osmoClient,
    osmoIcs721
  );
  t.deepEqual(
    beforeOsmoOutgoingClassTokenToChannelList,
    afterOsmoOutgoingClassTokenToChannelList,
    `outgoing channels must be unchanged:
- osmo before: ${JSON.stringify(beforeOsmoOutgoingClassTokenToChannelList)}
- osmo after: ${JSON.stringify(afterOsmoOutgoingClassTokenToChannelList)}`
  );
  t.deepEqual(
    beforeOsmoIncomingClassTokenToChannelList,
    afterOsmoIncomingClassTokenToChannelList,
    `incoming channels must be unchanged:
- osmo before: ${JSON.stringify(beforeOsmoIncomingClassTokenToChannelList)}
- osmo after: ${JSON.stringify(afterOsmoIncomingClassTokenToChannelList)}`
  );
  t.deepEqual(
    beforeOsmoNftContractsToClassIdList,
    afterOsmoNftContractsToClassIdList,
    `nft contracts must be unchanged:
- osmo before: ${JSON.stringify(beforeOsmoNftContractsToClassIdList)}
- osmo after: ${JSON.stringify(afterOsmoNftContractsToClassIdList)}`
  );

  // assert NFT on chain A is returned to owner
  tokenOwner = await ownerOf(wasmClient, wasmCw721_v16, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

  // ==== test transfer NFT to osmo chain via channel WLed ONLY on osmo incoming proxy and back to wasm chain ====
  tokenId = "3";
  await mint(wasmClient, wasmCw721_v16, tokenId, wasmAddr, undefined);
  // assert token is minted
  tokenOwner = await ownerOf(wasmClient, wasmCw721_v16, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

  // test transfer NFT to osmo chain
  t.log(
    `transfering to osmo chain via ${onlyOsmoIncomingChannel.channel.src.channelId}`
  );
  ibcMsg = {
    receiver: osmoAddr,
    channel_id: onlyOsmoIncomingChannel.channel.src.channelId,
    timeout: {
      block: {
        revision: 1,
        height: 90000,
      },
    },
  };
  transferResponse = await sendNft(
    wasmClient,
    wasmCw721_v16,
    wasmCw721OutgoingProxy,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);

  // Relay and verify we got a success
  t.log("relaying packets");
  info = await onlyOsmoIncomingChannel.link.relayAll();
  assertAckSuccess(info.acksFromA);

  // assert 1 entry for outgoing channels
  let wasmOutgoingClassTokenToChannelList = await outgoingChannels(
    wasmClient,
    wasmIcs721
  );
  t.log(
    `- outgoing channels: ${JSON.stringify(
      wasmOutgoingClassTokenToChannelList
    )}`
  );
  t.true(
    wasmOutgoingClassTokenToChannelList.length === 1,
    `outgoing channels must have one entry: ${JSON.stringify(
      wasmOutgoingClassTokenToChannelList
    )}`
  );

  // assert NFT minted on chain B
  osmoClassId = `${onlyOsmoIncomingChannel.channel.dest.portId}/${onlyOsmoIncomingChannel.channel.dest.channelId}/${wasmCw721_v16}`;
  osmoCw721 = await osmoClient.sign.queryContractSmart(osmoIcs721, {
    nft_contract: { class_id: osmoClassId },
  });
  allNFTs = await allTokens(osmoClient, osmoCw721);
  t.true(allNFTs.tokens.length === 1);
  // assert NFT on chain B is owned by osmoAddr
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);
  // assert NFT on chain A is locked/owned by ICS contract
  tokenOwner = await ownerOf(wasmClient, wasmCw721_v16, tokenId);
  t.is(wasmIcs721, tokenOwner.owner);
  // assert NFT on chain B is owned by osmoAddr
  osmoClassId = `${onlyOsmoIncomingChannel.channel.dest.portId}/${onlyOsmoIncomingChannel.channel.dest.channelId}/${wasmCw721_v16}`;
  osmoCw721 = await osmoClient.sign.queryContractSmart(osmoIcs721, {
    nft_contract: { class_id: osmoClassId },
  });
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);

  // test back transfer NFT to wasm chain, where onlyOsmoIncomingChannel is not WLed on wasm chain
  t.log(
    `transfering back to wasm chain via unknown ${onlyOsmoIncomingChannel.channel.dest.channelId}`
  );
  transferResponse = await sendNft(
    osmoClient,
    osmoCw721,
    t.context.osmoCw721OutgoingProxy,
    {
      receiver: wasmAddr,
      channel_id: onlyOsmoIncomingChannel.channel.dest.channelId,
      timeout: {
        block: {
          revision: 1,
          height: 90000,
        },
      },
    },
    tokenId
  );
  t.truthy(transferResponse);
  // before relay NFT escrowed by ICS721
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoIcs721, tokenOwner.owner);

  // Relay and verify we got an error
  t.log("relaying packets");
  info = await onlyOsmoIncomingChannel.link.relayAll();
  for (const ack of info.acksFromB) {
    const parsed = JSON.parse(fromUtf8(ack.acknowledgement));
    t.log(`- ack: ${JSON.stringify(parsed)}`);
  }
  assertAckErrors(info.acksFromB);

  // assert after failed relay, NFT on chain B is returned to owner
  allNFTs = await allTokens(osmoClient, osmoCw721);
  t.true(allNFTs.tokens.length === 1);
  // assert NFT is returned to sender on osmo chain
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);

  // ==== WL channel on wasm chain and test back transfer again ====
  t.log(
    `migrate ${wasmCw721IncomingProxy} contract and add channel ${onlyOsmoIncomingChannel.channel.src.channelId}`
  );
  await migrateIncomingProxy(
    wasmClient,
    wasmCw721IncomingProxy,
    wasmCw721IncomingProxyId,
    [
      channel.channel.src.channelId,
      onlyOsmoIncomingChannel.channel.src.channelId,
    ]
  );

  // test back transfer NFT to wasm chain, where onlyOsmoIncomingChannel is not WLed on wasm chain
  t.log(
    `transfering back to wasm chain via WLed ${onlyOsmoIncomingChannel.channel.dest.channelId}`
  );
  transferResponse = await sendNft(
    osmoClient,
    osmoCw721,
    t.context.osmoCw721OutgoingProxy,
    {
      receiver: wasmAddr,
      channel_id: onlyOsmoIncomingChannel.channel.dest.channelId,
      timeout: {
        block: {
          revision: 1,
          height: 90000,
        },
      },
    },
    tokenId
  );
  t.truthy(transferResponse);
  // before relay NFT escrowed by ICS721
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoIcs721, tokenOwner.owner);

  allNFTs = await allTokens(osmoClient, osmoCw721);
  t.log(`- all tokens: ${JSON.stringify(allNFTs)}`);

  // query nft contracts
  let nftContractsToClassIdList = await nftContracts(wasmClient, wasmIcs721);
  t.log(`- nft contracts: ${JSON.stringify(nftContractsToClassIdList)}`);
  t.true(
    nftContractsToClassIdList.length === 1,
    `nft contracts must have exactly one entry: ${JSON.stringify(
      nftContractsToClassIdList
    )}`
  );

  // Relay and verify success
  t.log("relaying packets");
  info = await onlyOsmoIncomingChannel.link.relayAll();
  for (const ack of info.acksFromB) {
    const parsed = JSON.parse(fromUtf8(ack.acknowledgement));
    t.log(`- ack: ${JSON.stringify(parsed)}`);
  }
  assertAckSuccess(info.acksFromB);

  // assert outgoing channels is empty
  wasmOutgoingClassTokenToChannelList = await outgoingChannels(
    wasmClient,
    wasmIcs721
  );
  t.true(
    wasmOutgoingClassTokenToChannelList.length === 0,
    `outgoing channels not empty: ${JSON.stringify(
      wasmOutgoingClassTokenToChannelList
    )}`
  );

  // assert after success relay, NFT on chain B is burned
  allNFTs = await allTokens(osmoClient, osmoCw721);
  t.log(`- all tokens: ${JSON.stringify(allNFTs)}`);
  t.true(allNFTs.tokens.length === 0);
  // assert list is unchanged
  nftContractsToClassIdList = await nftContracts(wasmClient, wasmIcs721);
  t.log(`- nft contracts: ${JSON.stringify(nftContractsToClassIdList)}`);
  t.true(
    nftContractsToClassIdList.length === 1,
    `nft contracts must have exactly one entry: ${JSON.stringify(
      nftContractsToClassIdList
    )}`
  );
  // assert NFT is returned to sender on wasm chain
  tokenOwner = await ownerOf(wasmClient, wasmCw721_v16, tokenId);
  t.is(wasmAddr, tokenOwner.owner);
});

test.serial("admin unescrow and burn NFT: wasmd -> osmo", async (t) => {
  await standardSetup(t);

  const {
    wasmClient,
    wasmAddr,
    wasmCw721,
    wasmIcs721,
    wasmCw721OutgoingProxy,
    osmoClient,
    osmoAddr,
    osmoIcs721,
    channel,
  } = t.context;

  const tokenId = "1";
  await mint(wasmClient, wasmCw721, tokenId, wasmAddr, undefined);
  // assert token is minted
  let tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

  // ==== happy path: transfer NFT to osmo chain ====
  // test transfer NFT to osmo chain
  t.log(`transfering to osmo chain via ${channel.channel.src.channelId}`);
  const ibcMsg = {
    receiver: osmoAddr,
    channel_id: channel.channel.src.channelId,
    timeout: {
      block: {
        revision: 1,
        height: 90000,
      },
    },
  };
  const transferResponse = await sendNft(
    wasmClient,
    wasmCw721,
    wasmCw721OutgoingProxy,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);

  // Relay and verify we got a success
  t.log("relaying packets");
  const info = await channel.link.relayAll();
  assertAckSuccess(info.acksFromA);

  // assert NFT on chain A is locked/owned by ICS contract
  tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmIcs721, tokenOwner.owner);
  // assert NFT minted on chain B
  const osmoClassId = `${channel.channel.dest.portId}/${channel.channel.dest.channelId}/${wasmCw721}`;
  const osmoCw721 = await osmoClient.sign.queryContractSmart(osmoIcs721, {
    nft_contract: { class_id: osmoClassId },
  });
  let allNFTs = await allTokens(osmoClient, osmoCw721);
  t.is(allNFTs.tokens.length, 1, `all tokens: ${JSON.stringify(allNFTs)}`);
  // assert NFT on chain B is owned by osmoAddr
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);

  const beforeWasmOutgoingClassTokenToChannelList = await outgoingChannels(
    wasmClient,
    wasmIcs721
  );
  // there should be one outgoing channel entry
  t.deepEqual(
    beforeWasmOutgoingClassTokenToChannelList,
    [[[wasmCw721, tokenId], channel.channel.src.channelId]],
    `wasm outgoing channels before:
- before: ${JSON.stringify(beforeWasmOutgoingClassTokenToChannelList)}`
  );
  // no incoming channel entry
  const beforeWasmIncomingClassTokenToChannelList = await incomingChannels(
    wasmClient,
    wasmIcs721
  );
  t.deepEqual(
    beforeWasmIncomingClassTokenToChannelList,
    [],
    `wasm incoming channels before:
- before: ${JSON.stringify(beforeWasmIncomingClassTokenToChannelList)}`
  );
  // one nft contract entry
  const beforeWasmNftContractsToClassIdList = await nftContracts(
    wasmClient,
    wasmIcs721
  );
  t.deepEqual(
    beforeWasmNftContractsToClassIdList,
    [[wasmCw721, wasmCw721]],
    `wasm nft contracts before:
- before: ${JSON.stringify(beforeWasmNftContractsToClassIdList)}`
  );

  // no outgoing channel entry
  const beforeOsmoOutgoingClassTokenToChannelList = await outgoingChannels(
    osmoClient,
    osmoIcs721
  );
  t.deepEqual(
    beforeOsmoOutgoingClassTokenToChannelList,
    [],
    `osmo outgoing channels before:
- before: ${JSON.stringify(beforeOsmoOutgoingClassTokenToChannelList)}`
  );
  // there should be one incoming channel entry
  const beforeOsmoIncomingClassTokenToChannelList = await incomingChannels(
    osmoClient,
    osmoIcs721
  );
  t.deepEqual(
    beforeOsmoIncomingClassTokenToChannelList,
    [[[osmoClassId, tokenId], channel.channel.dest.channelId]],
    `osmo incoming channels before:
- before: ${JSON.stringify(beforeOsmoIncomingClassTokenToChannelList)}`
  );
  // one nft contract entry
  const beforeOsmoNftContractsToClassIdList = await nftContracts(
    osmoClient,
    osmoIcs721
  );
  t.deepEqual(
    beforeOsmoNftContractsToClassIdList,
    [[osmoClassId, osmoCw721]],
    `osmo incoming channels before:
- before: ${JSON.stringify(beforeOsmoNftContractsToClassIdList)}`
  );

  // ==== test unescrow NFT on wasm chain ====
  t.log(`unescrow NFT on wasm chain`);
  await adminCleanAndUnescrowNft(
    wasmClient,
    wasmIcs721,
    wasmAddr,
    tokenId,
    wasmCw721,
    wasmCw721
  );
  // there should be no outgoing channel entry
  const afterWasmOutgoingClassTokenToChannelList = await outgoingChannels(
    wasmClient,
    wasmIcs721
  );
  t.deepEqual(
    afterWasmOutgoingClassTokenToChannelList,
    [],
    `wasm outgoing channels after:
- after: ${JSON.stringify(afterWasmOutgoingClassTokenToChannelList)}`
  );
  // assert NFT on chain A is owned by wasmAddr
  tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

  // ==== test burn NFT on osmo chain ====
  // we need to approve the contract to burn the NFT
  t.log(`approve NFT on osmo chain`);
  const response = await approve(osmoClient, osmoCw721, osmoIcs721, tokenId);
  t.log(`- response: ${JSON.stringify(response, bigIntReplacer, 2)}`);
  t.log(`burn NFT on osmo chain`);
  await adminCleanAndBurnNft(
    osmoClient,
    osmoIcs721,
    osmoAddr,
    tokenId,
    osmoClassId,
    osmoCw721
  );
  t.log(`- response: ${JSON.stringify(response, bigIntReplacer, 2)}`);
  allNFTs = await allTokens(osmoClient, osmoCw721);
  t.is(allNFTs.tokens.length, 0);
  // there should be no incoming channel entry
  const afterOsmoIncomingClassTokenToChannelList = await incomingChannels(
    osmoClient,
    osmoIcs721
  );
  t.deepEqual(
    afterOsmoIncomingClassTokenToChannelList,
    [],
    `osmo incoming channels after:
- after: ${JSON.stringify(afterOsmoIncomingClassTokenToChannelList)}`
  );
});

test.serial("malicious NFT", async (t) => {
  await standardSetup(t);
  const {
    wasmClient,
    wasmAddr,
    wasmIcs721,
    wasmCw721OutgoingProxy,
    osmoClient,
    osmoAddr,
    osmoIcs721,
    osmoCw721OutgoingProxy,
    channel,
  } = t.context;
  const tokenId = "1";

  // instantiate malicious cw721 contract
  const res = await uploadAndInstantiate(wasmClient, {
    cw721_gas_tester: {
      path: MALICIOUS_CW721,
      instantiateMsg: {
        name: "evil",
        symbol: "evil",
        minter: wasmClient.senderAddress,
        banned_recipient: "banned_recipient", // panic every time, on back transfer, when ICS721 tries to transfer/unescrow NFT to this address
      },
    },
  });
  const cw721 = res.cw721_gas_tester.address as string;

  // ==== test malicious NFT transfer to osmo chain ====
  await mint(wasmClient, cw721, tokenId, wasmAddr, undefined);
  t.log("transferring to osmo chain");
  let ibcMsg = {
    receiver: osmoAddr,
    channel_id: channel.channel.src.channelId,
    timeout: {
      block: {
        revision: 1,
        height: 90000,
      },
    },
  };
  let transferResponse = await sendNft(
    wasmClient,
    cw721,
    wasmCw721OutgoingProxy,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);

  t.log("relaying packets");
  let info = await channel.link.relayAll();
  assertAckSuccess(info.acksFromB);

  // assert NFT on chain A is locked/owned by ICS contract
  let tokenOwner = await ownerOf(wasmClient, cw721, tokenId);
  t.is(wasmIcs721, tokenOwner.owner);
  // assert NFT on chain B is owned by osmoAddr
  const osmoClassId = `${t.context.channel.channel.dest.portId}/${t.context.channel.channel.dest.channelId}/${cw721}`;
  const osmoCw721 = await osmoClient.sign.queryContractSmart(osmoIcs721, {
    nft_contract: { class_id: osmoClassId },
  });
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);

  // ==== test malicious NFT back transfer to banned recipient on wasm chain ====
  t.log("transferring back to wasm chain to banned recipient");
  ibcMsg = {
    receiver: "banned_recipient",
    channel_id: channel.channel.dest.channelId,
    timeout: {
      block: {
        revision: 1,
        height: 90000,
      },
    },
  };
  transferResponse = await sendNft(
    osmoClient,
    osmoCw721,
    osmoCw721OutgoingProxy,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);
  // before relay NFT escrowed by ICS721
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoIcs721, tokenOwner.owner);

  t.log("relaying packets");
  let pending = await channel.link.getPendingPackets("B");
  t.is(pending.length, 1);
  // Despite the transfer panicking, a fail ack should be returned.
  info = await channel.link.relayAll();
  assertAckErrors(info.acksFromA);
  // assert after failed relay, NFT on chain B is returned to owner
  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);
  t.log(`NFT #${tokenId} returned to owner`);

  // ==== test malicious NFT transfer to regular recipient wasm chain ====
  t.log("transferring back to wasm chain to recipient", wasmAddr);
  ibcMsg = {
    receiver: wasmAddr,
    channel_id: channel.channel.dest.channelId,
    timeout: {
      block: {
        revision: 1,
        height: 90000,
      },
    },
  };

  transferResponse = await sendNft(
    osmoClient,
    osmoCw721,
    osmoCw721OutgoingProxy,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);

  // Relay and verify we got a success
  t.log("relaying packets");
  pending = await channel.link.getPendingPackets("B");
  t.is(pending.length, 1);
  info = await channel.link.relayAll();
  assertAckSuccess(info.acksFromB);

  // assert NFT on chain A is returned to owner
  tokenOwner = await ownerOf(wasmClient, cw721, tokenId);
  t.is(wasmAddr, tokenOwner.owner);
});
