import { CosmWasmSigner } from "@confio/relayer";
import { ExecuteResult } from "@cosmjs/cosmwasm-stargate";

export interface ibcPingResponse {
  result: string;
}

export interface Connections {
  connections: string[];
}

export interface Counter {
  count: number;
}

export async function showConnections(
  cosmwasm: CosmWasmSigner,
  contractAddr: string
): Promise<Connections> {
  const query = { get_connections: {} };
  const res = await cosmwasm.sign.queryContractSmart(contractAddr, query);
  return res;
}

export async function showCounter(
  cosmwasm: CosmWasmSigner,
  contractAddr: string,
  channel: string
): Promise<Counter> {
  const query = { get_counter: { channel } };
  const res = await cosmwasm.sign.queryContractSmart(contractAddr, query);
  return res;
}

export async function sendPing(
  cosmwasm: CosmWasmSigner,
  contractAddr: string,
  channelId: string
): Promise<ExecuteResult> {
  const msg = {
    ping: {
      channel: channelId,
    },
  };

  const res = await cosmwasm.sign.execute(
    cosmwasm.senderAddress,
    contractAddr,
    msg,
    "auto",
    undefined,
    undefined
  );
  return res;
}
