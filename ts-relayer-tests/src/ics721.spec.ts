import { CosmWasmSigner } from "@confio/relayer";
import anyTest, { ExecutionContext, TestFn } from "ava";
import { Order } from "cosmjs-types/ibc/core/channel/v1/channel";

import { instantiateContract } from "./controller";
import { mint, ownerOf, sendNft } from "./cw721-utils";
import {
  assertAckErrors,
  assertAckSuccess,
  ChannelInfo,
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

  osmoIcs721: string;

  channel: ChannelInfo;
}

const test = anyTest as TestFn<TestContext>;

const WASM_FILE_CW721 = "./internal/cw721_base_v0.18.0.wasm";
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
    ics721: {
      path: WASM_FILE_CW_ICS721_ICS721,
      instantiateMsg: undefined,
    },
  };
  const osmoContracts: Record<string, ContractMsg> = {
    cw721: {
      path: WASM_FILE_CW721,
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

  const wasmIcs721Id = info.wasmContractInfos.ics721.codeId;
  const osmoIcs721Id = info.osmoContractInfos.ics721.codeId;

  t.context.wasmCw721 = info.wasmContractInfos.cw721.address as string;

  t.log(`instantiating wasm ICS721 contract (${wasmIcs721Id})`);

  const { contractAddress: wasmIcs721 } = await instantiateContract(
    wasmClient,
    wasmIcs721Id,
    { cw721_base_code_id: wasmCw721Id },
    "label ics721"
  );
  t.context.wasmIcs721 = wasmIcs721;

  t.log(`instantiating osmo ICS721 contract (${osmoIcs721Id})`);

  const { contractAddress: osmoIcs721 } = await instantiateContract(
    osmoClient,
    osmoIcs721Id,
    { cw721_base_code_id: osmoCw721Id },
    "label ics721"
  );
  t.context.osmoIcs721 = osmoIcs721;

  const channelInfo = await createIbcConnectionAndChannel(
    wasmClient,
    osmoClient,
    wasmIcs721,
    osmoIcs721,
    Order.ORDER_UNORDERED,
    "ics721-1"
  );

  t.context.channel = channelInfo;

  t.pass();
};

test.serial("transfer NFT", async (t) => {
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

  t.log("transfering to osmo chain");

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

  t.context.channel.channel.dest.channelId;

  const osmoClassId = `${t.context.channel.channel.dest.portId}/${t.context.channel.channel.dest.channelId}/${t.context.wasmCw721}`;
  const osmoCw721 = await osmoClient.sign.queryContractSmart(osmoIcs721, {
    nft_contract: { class_id: osmoClassId },
  });

  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);
});

test.serial("malicious NFT", async (t) => {
  await standardSetup(t);

  const {
    wasmClient,
    osmoClient,
    channel,
    osmoAddr,
    wasmAddr,
    wasmIcs721,
    osmoIcs721,
  } = t.context;
  const tokenId = "1";

  const res = await uploadAndInstantiate(wasmClient, {
    cw721_gas_tester: {
      path: MALICIOUS_CW721,
      instantiateMsg: {
        name: "evil",
        symbol: "evil",
        minter: wasmClient.senderAddress,
        target: wasmIcs721, // panic every time the ICS721 contract tries to return a NFT.
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

  t.log("transfering to osmo chain");

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

  const osmoClassId = `${t.context.channel.channel.dest.portId}/${t.context.channel.channel.dest.channelId}/${cw721}`;
  const osmoCw721 = await osmoClient.sign.queryContractSmart(osmoIcs721, {
    nft_contract: { class_id: osmoClassId },
  });

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
    osmoIcs721,
    ibcMsg,
    tokenId
  );
  t.truthy(transferResponse);

  t.log("relaying packets");

  const pending = await channel.link.getPendingPackets("B");
  t.is(pending.length, 1);

  // Despite the transfer panicing, a fail ack should be returned.
  info = await channel.link.relayAll();
  assertAckErrors(info.acksFromA);
});
