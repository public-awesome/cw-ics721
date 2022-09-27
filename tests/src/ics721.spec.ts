import { CosmWasmSigner } from "@confio/relayer";
import test from "ava";
import { Order } from "cosmjs-types/ibc/core/channel/v1/channel";

import { mint, ownerOf, transfer } from "./cw721-utils";
import {
  assertAckSuccess,
  ChannelInfo,
  ContractInfo,
  ContractMsg,
  createIbcConnectionAndChannel,
  MNEMONIC,
  setupOsmosisClient,
  setupWasmClient,
  uploadAndInstantiateAll,
} from "./utils";

let wasmClient: CosmWasmSigner;
let wasmClientAddress: string;
let osmoClient: CosmWasmSigner;
let osmoClientAddress: string;

let wasmContractInfos: Record<string, ContractInfo> = {};
let osmoContractInfos: Record<string, ContractInfo> = {};
let wasmContractAddressCw721: string;
let wasmContractAddressIcs721: string;
let osmoContractAddressIcs721: string;

let channelInfo: ChannelInfo;

const WASM_FILE_CW721 = "./internal/cw721_base_v0.15.0.wasm";
const WASM_FILE_CW_ICS721_BRIDGE = "./internal/cw_ics721_bridge.wasm";

//Upload contracts to chains.
test.before(async (t) => {
  wasmClient = await setupWasmClient(MNEMONIC);
  wasmClientAddress = wasmClient.senderAddress;
  console.debug(
    `Wasm client ${wasmClientAddress}, balance: ${JSON.stringify(
      await wasmClient.sign.getBalance(wasmClientAddress, "ucosm")
    )}`
  );
  osmoClient = await setupOsmosisClient(MNEMONIC);
  osmoClientAddress = osmoClient.senderAddress;
  console.debug(
    `Osmo client ${osmoClientAddress}, balance: ${JSON.stringify(
      await osmoClient.sign.getBalance(osmoClientAddress, "uosmo")
    )}`
  );

  const wasmContracts: Record<string, ContractMsg> = {
    cw721: {
      path: WASM_FILE_CW721,
      instantiateMsg: {
        name: "ark",
        symbol: "ark",
        minter: wasmClientAddress,
      },
    },
    ics721: {
      path: WASM_FILE_CW_ICS721_BRIDGE,
      instantiateMsg: { cw721_base_code_id: 0 },
    },
  };
  const osmoContracts: Record<string, ContractMsg> = {
    cw721: {
      path: WASM_FILE_CW721,
      instantiateMsg: undefined,
    },
    ics721: {
      path: WASM_FILE_CW_ICS721_BRIDGE,
      instantiateMsg: { cw721_base_code_id: 0 },
    },
  };
  const chainInfo = await uploadAndInstantiateAll(
    wasmClient,
    osmoClient,
    wasmContracts,
    osmoContracts
  );
  wasmContractInfos = chainInfo.wasmContractInfos;
  wasmContractAddressCw721 = wasmContractInfos.cw721.address as string;
  wasmContractAddressIcs721 = wasmContractInfos.ics721.address as string;
  osmoContractInfos = chainInfo.osmoContractInfos;
  osmoContractAddressIcs721 = osmoContractInfos.ics721.address as string;

  channelInfo = await createIbcConnectionAndChannel(
    chainInfo.wasmClient,
    chainInfo.osmoClient,
    wasmContractAddressIcs721,
    osmoContractAddressIcs721,
    Order.ORDER_UNORDERED,
    "ics721-1"
  );
  // console.log(`Channel created: ${JSON.stringify(channelInfo)}`);

  t.pass();
});

test.serial("transfer NFT", async (t) => {
  const token_id = "1";
  await mint(
    wasmClient,
    wasmContractAddressCw721,
    token_id,
    wasmClientAddress,
    undefined
  );
  // assert token is minted
  let tokenOwner = await ownerOf(
    wasmClient,
    wasmContractAddressCw721,
    token_id
  );
  t.is(wasmClientAddress, tokenOwner.owner);

  const ibcMsg = {
    receiver: osmoClientAddress, // wallet address of new owner on other side (osmo)
    channel_id: channelInfo.channel.src.channelId,
    timeout: {
      block: {
        revision: 1,
        height: 90000, // set as high as possible for avoiding timeout
      },
    },
  };
  console.log("Transferring to Osmo chain");
  const transferResponse = await transfer(
    wasmClient,
    wasmContractAddressCw721,
    wasmContractAddressIcs721,
    ibcMsg,
    token_id
  );
  t.truthy(transferResponse);

  console.log("Start relaying");
  // relay
  const info = await channelInfo.link.relayAll();

  // Verify we got a success
  assertAckSuccess(info.acksFromB);

  // assert NFT on chain A is locked/owned by ICS contract
  tokenOwner = await ownerOf(wasmClient, wasmContractAddressCw721, token_id);
  t.is(wasmContractAddressIcs721, tokenOwner.owner);
});
