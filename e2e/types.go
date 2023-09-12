package e2e_test

// The `Class` type as defined in `token_types.rs` and returned by the
// `class_metadata { class_id }` query.
type Class struct {
	ID   string  `json:"id"`
	URI  *string `json:"uri"`
	Data *string `json:"data"`
}

// The `Token` type as defined in `token_types.rs` and returned by the
// `token_metadata { class_id, token_id }` query.
type Token struct {
	ID   string  `json:"id"`
	URI  *string `json:"uri"`
	Data *string `json:"data"`
}

type ModuleInstantiateInfo struct {
	CodeID uint64 `json:"code_id"`
	Msg    string `json:"msg"`
	Admin  string `json:"admin"`
	Label  string `json:"label"`
}

type InstantiateICS721Bridge struct {
	CW721CodeID uint64                 `json:"cw721_base_code_id"`
	Proxy       *ModuleInstantiateInfo `json:"proxy"`
	Pauser      *string                `json:"pauser"`
}

type InstantiateCw721 struct {
	Name   string `json:"name"`
	Symbol string `json:"symbol"`
	Minter string `json:"minter"`
}

type InstantiateBridgeTester struct {
	AckMode string `json:"ack_mode"`
}

type OwnerOfResponse struct {
	Owner string `json:"owner"`
	// There is also an approvals field here but we don't care
	// about it so we just don't unmarshal.
}

// Owner query for ICS721 contract.
type OwnerQueryData struct {
	TokenID string `json:"token_id"`
	ClassID string `json:"class_id"`
}
type OwnerQuery struct {
	Owner OwnerQueryData `json:"owner"`
}

// ICS721 contract query for obtaining a NFT contract address given a class ID.
type NftContractQueryData struct {
	ClassID string `json:"class_id"`
}
type NftContractQuery struct {
	NftContractForClassId NftContractQueryData `json:"nft_contract"`
}

// Query for getting class ID given NFT contract.
type ClassIdQueryData struct {
	Contract string `json:"contract"`
}
type ClassIdQuery struct {
	ClassIdForNFTContract ClassIdQueryData `json:"class_id"`
}

// Query for getting metadata for a class ID from the ICS721 contract.
type ClassMetadataQueryData struct {
	ClassId string `json:"class_id"`
}
type ClassMetadataQuery struct {
	Metadata ClassMetadataQueryData `json:"class_metadata"`
}

// Query for getting token metadata.
type TokenMetadataQueryData struct {
	ClassId string `json:"class_id"`
	TokenId string `json:"token_id"`
}
type TokenMetadataQuery struct {
	Metadata TokenMetadataQueryData `json:"token_metadata"`
}

// Owner query for cw721 contract.
type OwnerOfQueryData struct {
	TokenID string `json:"token_id"`
}
type OwnerOfQuery struct {
	OwnerOf OwnerOfQueryData `json:"owner_of"`
}

// cw721 contract info query.
type ContractInfoQueryData struct{}
type ContractInfoQuery struct {
	ContractInfo ContractInfoQueryData `json:"contract_info"`
}
type ContractInfoResponse struct {
	Name   string `json:"name"`
	Symbol string `json:"symbol"`
}

// Query for getting last ACK from tester contract.
type LastAckQueryData struct{}
type LastAckQuery struct {
	LastAck LastAckQueryData `json:"last_ack"`
}

// cw721 token info query
type NftInfoQueryData struct {
	TokenID string `json:"token_id"`
}
type NftInfoQuery struct {
	Nftinfo NftInfoQueryData `json:"nft_info"`
}
type NftInfoQueryResponse struct {
	TokenURI *string `json:"token_uri"`
}
