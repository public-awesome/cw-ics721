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
    mint: {
      token_id,
      owner,
      token_uri,
      extension: {
        description: "This is a test NFT",
        image: "https://ark.pass/image.png",
      },
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

/// valid for v16 - v19
export function nftInfo(
  client: CosmWasmSigner,
  cw721Contract: string,
  token_id: string
): Promise<{
  token_uri: string | null;
  extension: {
    image: string | null;
    image_data: string | null;
    external_url: string | null;
    description: string | null;
    name: string | null;
    attributes: null | Array<{
      trait_type: string;
      value: string;
      display_type: string | null;
    }>;
    background_color: string | null;
    animation_url: string | null;
    youtube_url: string | null;
  };
}> {
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

export function getCw721CollectionInfoAndExtension(
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

export function getCw721ContractInfo_v16(
  client: CosmWasmSigner,
  cw721Contract: string
): Promise<{
  name: string;
  symbol: string;
}> {
  const msg = {
    contract_info: {},
  };
  return client.sign.queryContractSmart(cw721Contract, msg);
}

export function getCw721MinterOwnership(
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

export function getCw721Minter_v16(
  client: CosmWasmSigner,
  cw721Contract: string
): Promise<{
  minter: string;
}> {
  const msg = {
    minter: {},
  };
  return client.sign.queryContractSmart(cw721Contract, msg);
}

export function getCw721CreatorOwnership(
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
