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

export function sendNft(
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

export function approve(
  client: CosmWasmSigner,
  cw721Contract: string,
  spender: string,
  token_id: string
) {
  // msg to be executed on cw721 contract
  const msg = {
    approve: {
      token_id,
      spender,
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

export function getCollectionInfoAndExtension(
  client: CosmWasmSigner,
  cw721Contract: string
): Promise<{
  name: string;
  symbol: string;
  extension: {
    description: string;
    image: string;
    external_link: string | null;
    explicit_content: string | null;
    start_trading_time: string | null;
    royalty_info: {
      payment_address: string;
      share: string;
    } | null;
  } | null;
  updated_at: "1723541397075433676";
}> {
  const msg = {
    get_collection_info_and_extension: {},
  };
  return client.sign.queryContractSmart(cw721Contract, msg);
}

export function getMinterOwnership(
  client: CosmWasmSigner,
  cw721Contract: string
): Promise<{
  owner: string;
  pending_owner: string | null;
  pending_expiry: string | null;
}> {
  const msg = {
    get_minter_ownership: {},
  };
  return client.sign.queryContractSmart(cw721Contract, msg);
}

export function getCreatorOwnership(
  client: CosmWasmSigner,
  cw721Contract: string
): Promise<{
  owner: string;
  pending_owner: string | null;
  pending_expiry: string | null;
}> {
  const msg = {
    get_creator_ownership: {},
  };
  return client.sign.queryContractSmart(cw721Contract, msg);
}

export function numTokens(
  client: CosmWasmSigner,
  cw721Contract: string
): Promise<{ count: number }> {
  const msg = {
    num_tokens: {},
  };
  return client.sign.queryContractSmart(cw721Contract, msg);
}
