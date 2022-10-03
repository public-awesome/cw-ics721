import { CosmWasmSigner } from "@confio/relayer";
import anyTest, { TestFn } from "ava";
import { Order } from "cosmjs-types/ibc/core/channel/v1/channel";

import { instantiateContract } from "./controller";
import { mint, ownerOf, sendNft } from "./cw721-utils";
import {
  assertAckSuccess,
  ChannelInfo,
  ContractMsg,
  createIbcConnectionAndChannel,
  MNEMONIC,
  setupOsmosisClient,
  setupWasmClient,
  uploadAndInstantiateAll,
} from "./utils";

interface TestContext {
  wasmClient: CosmWasmSigner;
  wasmAddr: string;

  osmoClient: CosmWasmSigner;
  osmoAddr: string;

  wasmCw721: string;
  wasmBridge: string;

  osmoBridge: string;

  channel: ChannelInfo;
}

const test = anyTest as TestFn<TestContext>;

const WASM_FILE_CW721 = "./internal/cw721_base_v0.15.0.wasm";
const WASM_FILE_CW_ICS721_BRIDGE = "./internal/cw_ics721_bridge.wasm";

test.beforeEach(async (t) => {
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
      path: WASM_FILE_CW_ICS721_BRIDGE,
      instantiateMsg: undefined,
    },
  };
  const osmoContracts: Record<string, ContractMsg> = {
    cw721: {
      path: WASM_FILE_CW721,
      instantiateMsg: undefined,
    },
    ics721: {
      path: WASM_FILE_CW_ICS721_BRIDGE,
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

  const wasmBridgeId = info.wasmContractInfos.ics721.codeId;
  const osmoBridgeId = info.wasmContractInfos.ics721.codeId;

  t.context.wasmCw721 = info.wasmContractInfos.cw721.address as string;

  const { contractAddress: wasmBridge } = await instantiateContract(
    wasmClient,
    wasmBridgeId,
    { cw721_base_code_id: wasmCw721Id },
    "label ics721"
  );
  t.context.wasmBridge = wasmBridge;

  const { contractAddress: osmoBridge } = await instantiateContract(
    osmoClient,
    osmoBridgeId,
    { cw721_base_code_id: osmoCw721Id },
    "label ics721"
  );
  t.context.osmoBridge = osmoBridge;

  const channelInfo = await createIbcConnectionAndChannel(
    wasmClient,
    osmoClient,
    wasmBridge,
    osmoBridge,
    Order.ORDER_UNORDERED,
    "ics721-1"
  );

  t.context.channel = channelInfo;

  t.pass();
});

test.serial("transfer NFT", async (t) => {
  const {
    wasmClient,
    wasmAddr,
    wasmCw721,
    wasmBridge,
    osmoClient,
    osmoAddr,
    osmoBridge,
    channel,
  } = t.context;

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
    wasmBridge,
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
  t.is(wasmBridge, tokenOwner.owner);

  t.context.channel.channel.dest.channelId;

  const osmoClassId = `${t.context.channel.channel.dest.portId}/${t.context.channel.channel.dest.channelId}/${t.context.wasmCw721}`;
  const osmoCw721 = await osmoClient.sign.queryContractSmart(osmoBridge, {
    nft_contract_for_class_id: { class_id: osmoClassId },
  });

  tokenOwner = await ownerOf(osmoClient, osmoCw721, tokenId);
  t.is(osmoAddr, tokenOwner.owner);
});
