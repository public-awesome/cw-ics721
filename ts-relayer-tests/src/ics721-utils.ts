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
