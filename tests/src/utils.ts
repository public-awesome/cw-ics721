import { readFileSync } from "fs";

import {
  AckWithMetadata,
  CosmWasmSigner,
  Link,
  Logger,
  RelayInfo,
  testutils,
} from "@confio/relayer";
import { ChannelPair } from "@confio/relayer/build/lib/link";
import { fromBase64, fromUtf8 } from "@cosmjs/encoding";
import { assert } from "@cosmjs/utils";
import { Order } from "cosmjs-types/ibc/core/channel/v1/channel";

import { instantiateContract } from "./controller";

const {
  fundAccount,
  generateMnemonic,
  osmosis: oldOsmo,
  signingCosmWasmClient,
  wasmd,
  setup,
} = testutils;

const osmosis = { ...oldOsmo, minFee: "0.025uosmo" };

export const MNEMONIC =
  "harsh adult scrub stadium solution impulse company agree tomorrow poem dirt innocent coyote slight nice digital scissors cool pact person item moon double wagon";

export interface ContractMsg {
  path: string;
  instantiateMsg: Record<string, unknown> | undefined;
}

export interface ChainInfo {
  wasmClient: CosmWasmSigner;
  osmoClient: CosmWasmSigner;
  wasmContractInfos: Record<string, ContractInfo>;
  osmoContractInfos: Record<string, ContractInfo>;
}

export interface ContractInfo {
  codeId: number;
  address: string | undefined;
}

export interface ChannelInfo {
  channel: ChannelPair;
  link: Link;
}

export async function uploadAndInstantiateAll(
  wasmClient: CosmWasmSigner,
  osmoClient: CosmWasmSigner,
  wasmContracts: Record<string, ContractMsg>,
  osmoContracts: Record<string, ContractMsg>
): Promise<ChainInfo> {
  console.debug("###### Upload contract to wasmd");
  const wasmContractInfos = await uploadAndInstantiate(
    wasmClient,
    wasmContracts
  );

  console.debug("###### Upload contract to osmo");
  const osmoContractInfos = await uploadAndInstantiate(
    osmoClient,
    osmoContracts
  );
  return {
    wasmClient,
    osmoClient,
    wasmContractInfos,
    osmoContractInfos,
  };
}

export async function uploadAndInstantiate(
  client: CosmWasmSigner,
  contracts: Record<string, ContractMsg>
): Promise<Record<string, ContractInfo>> {
  const contractInfos: Record<string, ContractInfo> = {};
  for (const name in contracts) {
    const contractMsg = contracts[name];
    console.debug(`storing ${name} contract from ${contractMsg.path}`);
    const wasm = await readFileSync(contractMsg.path);
    const receipt = await client.sign.upload(
      client.senderAddress,
      wasm,
      "auto", // auto fee
      `Upload ${name}` // memo
    );
    const codeId = receipt.codeId;
    assert(codeId);
    console.debug(`- code id: ${receipt.codeId}`);
    let address;
    if (contractMsg.instantiateMsg) {
      const { contractAddress } = await instantiateContract(
        client,
        codeId,
        contractMsg.instantiateMsg,
        "label " + name
      );
      console.debug(`- contract address: ${contractAddress}`);
      assert(contractAddress);
      address = contractAddress;
    }
    contractInfos[name] = { codeId, address };
  }
  return contractInfos;
}

export async function createIbcConnectionAndChannel(
  wasmClient: CosmWasmSigner,
  osmoClient: CosmWasmSigner,
  wasmContractAddress: string,
  osmoContractAddress: string,
  ordering: Order,
  version: string
): Promise<ChannelInfo> {
  const { ibcPortId: wasmContractIbcPortId } =
    await wasmClient.sign.getContract(wasmContractAddress);
  assert(wasmContractIbcPortId);
  const { ibcPortId: osmoContractIbcPortId } =
    await osmoClient.sign.getContract(osmoContractAddress);
  assert(osmoContractIbcPortId);
  // create a connection and channel
  const [src, dest] = await setup(wasmd, osmosis);
  const logger: Logger = {
    debug(message: string, meta?: Record<string, unknown>): Logger {
      const logMsg = meta ? message + ": " + JSON.stringify(meta) : message;
      console.debug("[relayer|debug]: " + logMsg);
      return this;
    },

    info(message: string, meta?: Record<string, unknown>): Logger {
      const logMsg = meta ? message + ": " + JSON.stringify(meta) : message;
      console.info("[relayer|info]: " + logMsg);
      return this;
    },

    error(message: string, meta?: Record<string, unknown>): Logger {
      const logMsg = meta ? message + ": " + JSON.stringify(meta) : message;
      console.error("[relayer|error]: " + logMsg);
      return this;
    },

    warn(message: string, meta?: Record<string, unknown>): Logger {
      const logMsg = meta ? message + ": " + JSON.stringify(meta) : message;
      console.warn("[relayer|warn]: " + logMsg);
      return this;
    },

    verbose(message: string, meta?: Record<string, unknown>): Logger {
      const logMsg = meta ? message + ": " + JSON.stringify(meta) : message;
      console.debug("[relayer|verbose]: " + logMsg);
      return this;
    },
  };
  const link = await Link.createWithNewConnections(src, dest, logger);
  const channel = await link.createChannel(
    "A",
    wasmContractIbcPortId,
    osmoContractIbcPortId,
    ordering,
    version
  );

  return { channel, link };
}

/**
 * This creates a client for the Wasmd chain, that can interact with contracts.
 *
 * @param mnemonic optional, by default it generates a mnemonic
 * @returns
 */
export async function setupWasmClient(
  mnemonic = generateMnemonic()
): Promise<CosmWasmSigner> {
  // create apps and fund an account
  const cosmwasm = await signingCosmWasmClient(wasmd, mnemonic);
  await fundAccount(wasmd, cosmwasm.senderAddress, "4000000");
  return cosmwasm;
}

/**
 * This creates a client for the Osmosis chain, that can interact with contracts.
 *
 * @param mnemonic optional, by default it generates a mnemonic
 * @returns
 */
export async function setupOsmosisClient(
  mnemonic = generateMnemonic()
): Promise<CosmWasmSigner> {
  // create apps and fund an account
  const cosmwasm = await signingCosmWasmClient(osmosis, mnemonic);
  await fundAccount(osmosis, cosmwasm.senderAddress, "4000000");
  return cosmwasm;
}

// throws error if not all are success
export function assertAckSuccess(acks: AckWithMetadata[]) {
  const parsedAcks = acks.map((ack) =>
    JSON.parse(fromUtf8(ack.acknowledgement))
  );
  console.debug(`Parsing acks: ${JSON.stringify(parsedAcks)}`);
  for (const parsed of parsedAcks) {
    if (parsed.error) {
      throw new Error(`Unexpected error in ack: ${parsed.error}`);
    }
    if (!parsed.result) {
      throw new Error(`Ack result unexpectedly empty: ${parsed}`);
    }
  }
}

// throws error if not all are errors
export function assertAckErrors(acks: AckWithMetadata[]) {
  for (const ack of acks) {
    const parsed = JSON.parse(fromUtf8(ack.acknowledgement));
    if (parsed.result) {
      throw new Error(`Ack result unexpectedly set`);
    }
    if (!parsed.error) {
      throw new Error(`Ack error unexpectedly empty`);
    }
  }
}

export function assertPacketsFromA(
  relay: RelayInfo,
  count: number,
  success: boolean
) {
  if (relay.packetsFromA !== count) {
    throw new Error(`Expected ${count} packets, got ${relay.packetsFromA}`);
  }
  if (relay.acksFromB.length !== count) {
    throw new Error(`Expected ${count} acks, got ${relay.acksFromB.length}`);
  }
  if (success) {
    assertAckSuccess(relay.acksFromB);
  } else {
    assertAckErrors(relay.acksFromB);
  }
}

export function assertPacketsFromB(
  relay: RelayInfo,
  count: number,
  success: boolean
) {
  if (relay.packetsFromB !== count) {
    throw new Error(`Expected ${count} packets, got ${relay.packetsFromB}`);
  }
  if (relay.acksFromA.length !== count) {
    throw new Error(`Expected ${count} acks, got ${relay.acksFromA.length}`);
  }
  if (success) {
    assertAckSuccess(relay.acksFromA);
  } else {
    assertAckErrors(relay.acksFromA);
  }
}

export function parseAcknowledgementSuccess<T>(ack: AckWithMetadata): T {
  const response = JSON.parse(fromUtf8(ack.acknowledgement));
  assert(response.result);
  return JSON.parse(fromUtf8(fromBase64(response.result)));
}
