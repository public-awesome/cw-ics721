import { CosmWasmSigner } from "@confio/relayer";
import { ExecuteResult, InstantiateResult } from "@cosmjs/cosmwasm-stargate";
import { assert } from "@cosmjs/utils";

export async function instantiateContract(
  client: CosmWasmSigner,
  codeId: number,
  msg: Record<string, unknown>,
  label: string
): Promise<InstantiateResult> {
  const result = await client.sign.instantiate(
    client.senderAddress,
    codeId,
    msg,
    label,
    "auto"
  );
  assert(result.contractAddress);
  return result;
}

export async function getIbcPortId(
  client: CosmWasmSigner,
  contractAddress: string
) {
  const { ibcPortId } = await client.sign.getContract(contractAddress);
  console.debug(`IBC port id: ${ibcPortId}`);
  assert(ibcPortId);
  return ibcPortId;
}

export function executeContract(
  client: CosmWasmSigner,
  contractAddress: string,
  msg: Record<string, unknown>
): Promise<ExecuteResult> {
  return client.sign.execute(
    client.senderAddress,
    contractAddress,
    msg,
    "auto", // fee
    undefined, // no memo
    undefined // no funds
  );
}
