import { CosmWasmSigner } from "@confio/relayer";

// ######### execute

export function migrate(
  client: CosmWasmSigner,
  contractAddress: string,
  codeId: number,
  incoming_proxy?: string,
  outgoing_proxy?: string
) {
  const msg = {
    with_update: { incoming_proxy, outgoing_proxy },
  };
  return client.sign.migrate(
    client.senderAddress,
    contractAddress,
    codeId,
    msg,
    "auto",
    undefined
  );
}

export function migrateIncomingProxy(
  client: CosmWasmSigner,
  contractAddress: string,
  codeId: number,
  channels?: string[],
  origin?: string
) {
  const msg = {
    with_update: { origin, channels },
  };
  return client.sign.migrate(
    client.senderAddress,
    contractAddress,
    codeId,
    msg,
    "auto",
    undefined
  );
}

// ######### query
export function nftContracts(
  client: CosmWasmSigner,
  contractAddress: string
): Promise<[string, string][]> {
  const msg = {
    nft_contracts: {},
  };
  return client.sign.queryContractSmart(contractAddress, msg);
}

export function outgoingChannels(
  client: CosmWasmSigner,
  contractAddress: string
): Promise<[[string, string], string][]> {
  const msg = {
    outgoing_channels: {},
  };
  return client.sign.queryContractSmart(contractAddress, msg);
}
