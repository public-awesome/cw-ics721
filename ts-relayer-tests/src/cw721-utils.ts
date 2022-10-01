import { CosmWasmSigner } from "@confio/relayer";

// ######### execute

export function mint(
  client: CosmWasmSigner,
  cw721Contract: string,
  token_id: string,
  owner: string,
  token_uri: string | undefined
) {
  const msg = {
    mint: { token_id, owner, token_uri },
  };
  return client.sign.execute(
    client.senderAddress,
    cw721Contract,
    msg,
    "auto", // fee
    undefined, // no memo
    undefined // no funds
  );
}

export function transfer(
  client: CosmWasmSigner,
  cw721Contract: string,
  targetContract: string,
  targetMsg: Record<string, unknown>,
  token_id: string
) {
  const encode = btoa(JSON.stringify(targetMsg));
  // msg to be executed on cw721 contract
  const msg = {
    send_nft: {
      contract: targetContract, // send to target from cw721 contract
      token_id,
      msg: encode, //above ibc msg to be passed to ics721 contract on wasm side
    },
  };
  return client.sign.execute(
    client.senderAddress,
    cw721Contract,
    msg,
    "auto", // fee
    undefined, // no memo
    undefined // no funds
  );
}

// ######### query
export function allTokens(
  client: CosmWasmSigner,
  cw721Contract: string
): Promise<{ tokens: string[] }> {
  const msg = {
    all_tokens: {},
  };
  return client.sign.queryContractSmart(cw721Contract, msg);
}

export function nftInfo(
  client: CosmWasmSigner,
  cw721Contract: string,
  token_id: string
) {
  const msg = {
    nft_info: {
      token_id,
    },
  };
  return client.sign.queryContractSmart(cw721Contract, msg);
}

export function allNftInfo(
  client: CosmWasmSigner,
  cw721Contract: string,
  token_id: string,
  include_expired = true
) {
  const msg = {
    all_nft_info: {
      token_id,
      include_expired,
    },
  };
  return client.sign.queryContractSmart(cw721Contract, msg);
}

export function ownerOf(
  client: CosmWasmSigner,
  cw721Contract: string,
  token_id: string,
  include_expired = true
): Promise<{
  owner: string;
  approvals: { spender: string; expires: unknown }[];
}> {
  const msg = {
    owner_of: {
      token_id,
      include_expired,
    },
  };
  return client.sign.queryContractSmart(cw721Contract, msg);
}
