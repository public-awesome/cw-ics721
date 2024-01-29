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

export function adminCleanAndUnescrowNft(
  client: CosmWasmSigner,
  contractAddress: string,
  recipient: string,
  token_id: string,
  class_id: string,
  collection: string
) {
  const msg = {
    admin_clean_and_unescrow_nft: {
      recipient,
      token_id,
      class_id,
      collection,
    },
  };
  return client.sign.execute(
    client.senderAddress,
    contractAddress,
    msg,
    "auto",
    undefined
  );
}

export function adminCleanAndBurnNft(
  client: CosmWasmSigner,
  contractAddress: string,
  owner: string,
  token_id: string,
  class_id: string,
  collection: string
) {
  const msg = {
    admin_clean_and_burn_nft: {
      owner,
      token_id,
      class_id,
      collection,
    },
  };
  return client.sign.execute(
    client.senderAddress,
    contractAddress,
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

export function incomingChannels(
  client: CosmWasmSigner,
  contractAddress: string
): Promise<[[string, string], string][]> {
  const msg = {
    incoming_channels: {},
  };
  return client.sign.queryContractSmart(contractAddress, msg);
}
