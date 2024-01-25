import { CosmWasmSigner } from "@confio/relayer";
import anyTest, { ExecutionContext, TestFn } from "ava";
import { Order } from "cosmjs-types/ibc/core/channel/v1/channel";

import { instantiateContract } from "./controller";
import { mint, ownerOf, sendNft } from "./cw721-utils";
import { migrate } from "./ics721-utils";
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
  wasmIcs721: string;
  wasmCw721IncomingProxy: string;

  osmoCw721: string;
  osmoIcs721: string;
  osmoCw721OutgoingProxy: string;

  channel: ChannelAndLinkInfo;

  otherChannel: ChannelAndLinkInfo;
}

const test = anyTest as TestFn<TestContext>;

const WASM_FILE_CW721 = "./internal/cw721_base_v0.18.0.wasm";
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
        name: "ark",
        symbol: "ark",
        minter: wasmClient.senderAddress,
      },
    },
    cw721IncomingProxy: {
      path: WASM_FILE_CW721_INCOMING_PROXY,
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
        name: "ark",
        symbol: "ark",
        minter: osmoClient.senderAddress,
      },
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

  const wasmIcs721Id = info.wasmContractInfos.ics721.codeId;
  const osmoIcs721Id = info.osmoContractInfos.ics721.codeId;

  const osmoCw721OutgoingProxyId =
    info.osmoContractInfos.cw721OutgoingProxy.codeId;

  t.context.wasmCw721 = info.wasmContractInfos.cw721.address as string;
  t.context.osmoCw721 = info.osmoContractInfos.cw721.address as string;

  t.log(`instantiating wasm ICS721 contract (${wasmIcs721Id})`);
  const { contractAddress: wasmIcs721 } = await instantiateContract(
    wasmClient,
    wasmIcs721Id,
    { cw721_base_code_id: wasmCw721Id },
    "label ics721"
  );
  t.log(`- wasm ICS721 contract address: ${wasmIcs721}`);
  t.context.wasmIcs721 = wasmIcs721;

  t.log(`instantiating osmo ICS721 contract (${osmoIcs721Id})`);
  const { contractAddress: osmoIcs721 } = await instantiateContract(
    osmoClient,
    osmoIcs721Id,
    { cw721_base_code_id: osmoCw721Id },
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
  t.log(`- channel: ${JSON.stringify(channelInfo, bigIntReplacer, 2)}`);
  t.context.channel = channelInfo;

  t.log(
    `instantiating wasm cw721-incoming-proxy (${wasmCw721IncomingProxyId}) for channel ${channelInfo.channel.src.channelId}`
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
    `migrate ${wasmIcs721} contract to use incoming proxy ${wasmCw721IncomingProxy}`
  );
  await migrate(wasmClient, wasmIcs721, wasmIcs721Id, wasmCw721IncomingProxy);

  const per_block = 10; // use high rate limit to avoid test failures
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
    `migrate ${osmoIcs721} contract to use outgoing proxy ${osmoCw721OutgoingProxy}`
  );
  await migrate(
    osmoClient,
    osmoIcs721,
    osmoIcs721Id,
    undefined,
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
    osmoClient,
    osmoAddr,
    osmoIcs721,
    channel,
  } = t.context;

  t.log(JSON.stringify(wasmClient, undefined, 2));
  const tokenId = "1";
  await mint(wasmClient, wasmCw721, tokenId, wasmAddr, undefined);
  // assert token is minted
  let tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmAddr, tokenOwner.owner);

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

  t.log(`transfering to osmo chain via ${channel.channel.src.channelId}`);

  const transferResponse = await sendNft(
    wasmClient,
    wasmCw721,
    wasmIcs721,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);

  t.log("relaying packets");

  const info = await channel.link.relayAll();

  // Verify we got a success
  assertAckSuccess(info.acksFromB);

  // assert NFT on chain A is locked/owned by ICS contract
  tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
  t.is(wasmIcs721, tokenOwner.owner);

  const osmoClassId = `${t.context.channel.channel.dest.portId}/${t.context.channel.channel.dest.channelId}/${t.context.wasmCw721}`;
  const osmoCw721 = await osmoClient.sign.queryContractSmart(osmoIcs721, {
    nft_contract: { class_id: osmoClassId },
  });

  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);
});

test.serial(
  "transfer NFT with osmo outgoing and wasm incoming proxy",
  async (t) => {
    await standardSetup(t);

    const {
      wasmClient,
      wasmAddr,
      wasmIcs721,
      osmoClient,
      osmoAddr,
      osmoCw721,
      osmoIcs721,
      osmoCw721OutgoingProxy,
      channel,
      otherChannel,
    } = t.context;

    // test 1: transfer via outgoing proxy and using WLed channel by incoming proxy
    let tokenId = "1";
    t.log(`transferring NFT #${tokenId} from osmo to wasmd chain`);
    await mint(osmoClient, osmoCw721, tokenId, osmoAddr, undefined);
    // assert token is minted
    let tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
    t.is(osmoAddr, tokenOwner.owner);

    let ibcMsg = {
      receiver: wasmAddr,
      channel_id: channel.channel.dest.channelId,
      timeout: {
        block: {
          revision: 1,
          height: 90000,
        },
      },
    };

    t.log(
      `transfering to wasm chain via ${channel.channel.dest.channelId} and outgoing proxy ${osmoCw721OutgoingProxy}`
    );

    let transferResponse = await sendNft(
      osmoClient,
      osmoCw721,
      osmoCw721OutgoingProxy,
      ibcMsg,
      tokenId
    );
    t.truthy(transferResponse);

    t.log("relaying packets");

    let info = await channel.link.relayAll();

    // Verify we got a success
    assertAckSuccess(info.acksFromA);

    // assert NFT on chain A is locked/owned by ICS contract
    tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
    t.is(osmoIcs721, tokenOwner.owner);
    t.log(`NFT #${tokenId} locked by ICS721 contract`);

    const wasmClassId = `${t.context.channel.channel.src.portId}/${t.context.channel.channel.src.channelId}/${t.context.osmoCw721}`;
    const wasmCw721 = await wasmClient.sign.queryContractSmart(wasmIcs721, {
      nft_contract: { class_id: wasmClassId },
    });

    tokenOwner = await ownerOf(wasmClient, wasmCw721, tokenId);
    t.is(wasmAddr, tokenOwner.owner);
    t.log(`NFT #${tokenId} transferred to ${wasmAddr}`);

    // test 2: transfer via outgoing proxy and using unknown channel by incoming proxy
    tokenId = "2";
    t.log(`transferring NFT #${tokenId} from osmo to wasmd chain`);
    await mint(osmoClient, osmoCw721, tokenId, osmoAddr, undefined);
    // assert token is minted
    tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
    t.is(osmoAddr, tokenOwner.owner);

    ibcMsg = {
      receiver: wasmAddr,
      channel_id: otherChannel.channel.dest.channelId,
      timeout: {
        block: {
          revision: 1,
          height: 90000,
        },
      },
    };

    t.log(
      `transfering to wasm chain via ${otherChannel.channel.dest.channelId}`
    );

    transferResponse = await sendNft(
      osmoClient,
      osmoCw721,
      osmoCw721OutgoingProxy,
      ibcMsg,
      tokenId
    );
    t.truthy(transferResponse);

    t.log("relaying packets");

    info = await otherChannel.link.relayAll();

    // Verify we got an error
    assertAckErrors(info.acksFromA);

    // assert NFT on chain B is returned to owner
    tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
    t.is(osmoAddr, tokenOwner.owner);
    t.log(`NFT #${tokenId} returned to owner`);
  }
);

test.serial("malicious NFT", async (t) => {
  await standardSetup(t);

  const {
    wasmClient,
    wasmAddr,
    wasmIcs721,
    osmoClient,
    osmoAddr,
    osmoIcs721,
    osmoCw721OutgoingProxy,
    channel,
  } = t.context;
  const tokenId = "1";

  const res = await uploadAndInstantiate(wasmClient, {
    cw721_gas_tester: {
      path: MALICIOUS_CW721,
      instantiateMsg: {
        name: "evil",
        symbol: "evil",
        minter: wasmClient.senderAddress,
        banned_recipient: "banned_recipient", // panic every time the ICS721 contract tries to transfer NFT to this address
      },
    },
  });

  const cw721 = res.cw721_gas_tester.address as string;

  await mint(wasmClient, cw721, tokenId, wasmAddr, undefined);

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

  t.log("transferring to osmo chain");

  let transferResponse = await sendNft(
    wasmClient,
    cw721,
    wasmIcs721,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);

  t.log("relaying packets");

  let info = await channel.link.relayAll();

  assertAckSuccess(info.acksFromB);

  t.log("transferring back to wasm chain to banned recipient");

  const osmoClassId = `${t.context.channel.channel.dest.portId}/${t.context.channel.channel.dest.channelId}/${cw721}`;
  const osmoCw721 = await osmoClient.sign.queryContractSmart(osmoIcs721, {
    nft_contract: { class_id: osmoClassId },
  });

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

  t.log("relaying packets");

  let pending = await channel.link.getPendingPackets("B");
  t.is(pending.length, 1);

  // Despite the transfer panicking, a fail ack should be returned.
  info = await channel.link.relayAll();
  assertAckErrors(info.acksFromA);
  // assert NFT on chain B is returned to owner
  let tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);
  t.log(`NFT #${tokenId} returned to owner`);

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

  t.log("relaying packets");

  pending = await channel.link.getPendingPackets("B");
  t.is(pending.length, 1);

  // Verify we got a success
  info = await channel.link.relayAll();
  assertAckSuccess(info.acksFromB);

  // assert NFT on chain A is returned to owner
  tokenOwner = await ownerOf(wasmClient, cw721, tokenId);
  t.is(wasmAddr, tokenOwner.owner);
});
