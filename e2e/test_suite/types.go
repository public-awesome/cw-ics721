package test_suite

// Class The `Class` type as defined in `token_types.rs` and returned by the
// `class_metadata { class_id }` query.
type Class struct {
	ID   string  `json:"id"`
	URI  *string `json:"uri"`
	Data *string `json:"data"`
}

// Token The `Token` type as defined in `token_types.rs` and returned by the
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
	CW721CodeID   uint64                 `json:"cw721_base_code_id"`
	OutgoingProxy *ModuleInstantiateInfo `json:"outgoing_proxy"`
	IncomingProxy *ModuleInstantiateInfo `json:"incoming_proxy"`
	Pauser        *string                `json:"pauser"`
}

// InstantiateCw721v18 v18 introduced the withdraw_address field
type InstantiateCw721v18 struct {
	Name            string  `json:"name"`
	Symbol          string  `json:"symbol"`
	Minter          string  `json:"minter"`
	WithdrawAddress *string `json:"withdraw_address"`
}

// InstantiateCw721v16 valid for v17 too
type InstantiateCw721v16 struct {
	Name   string `json:"name"`
	Symbol string `json:"symbol"`
	Minter string `json:"minter"`
}

type InstantiateBridgeTester struct {
	AckMode string `json:"ack_mode"`
	Ics721  string `json:"ics721"`
}

type OwnerOfResponse struct {
	Owner string `json:"owner"`
	// There is also an approvals field here, but we don't care
	// about it, so we just don't unmarshal.
}

type TesterResponse struct {
	Owner *string `json:"owner"`
}

// OwnerQueryData Owner query for ICS721 contract.
type OwnerQueryData struct {
	TokenID string `json:"token_id"`
	ClassID string `json:"class_id"`
}
type OwnerQuery struct {
	Owner OwnerQueryData `json:"owner"`
}

// NftContractQueryData ICS721 contract query for obtaining a NFT contract address given a class ID.
type NftContractQueryData struct {
	ClassID string `json:"class_id"`
}

type NftContractsQueryData struct{}

type NftContractQuery struct {
	NftContractForClassId NftContractQueryData `json:"nft_contract"`
}

type NftContractsQuery struct {
	NftContracts NftContractsQueryData `json:"nft_contracts"`
}

// ClassIdQueryData Query for getting class ID given NFT contract.
type ClassIdQueryData struct {
	Contract string `json:"contract"`
}
type ClassIdQuery struct {
	ClassIdForNFTContract ClassIdQueryData `json:"class_id"`
}

// ClassMetadataQueryData Query for getting metadata for a class ID from the ICS721 contract.
type ClassMetadataQueryData struct {
	ClassId string `json:"class_id"`
}
type ClassMetadataQuery struct {
	Metadata ClassMetadataQueryData `json:"class_metadata"`
}

// TokenMetadataQueryData Query for getting token metadata.
type TokenMetadataQueryData struct {
	ClassId string `json:"class_id"`
	TokenId string `json:"token_id"`
}
type TokenMetadataQuery struct {
	Metadata TokenMetadataQueryData `json:"token_metadata"`
}

// OwnerOfQueryData Owner query for cw721 contract.
type OwnerOfQueryData struct {
	TokenID string `json:"token_id"`
}
type OwnerOfQuery struct {
	OwnerOf OwnerOfQueryData `json:"owner_of"`
}

type EmptyData struct{}

type TesterSentQuery struct {
	GetSentCallback EmptyData `json:"get_sent_callback"`
}

type TesterReceivedQuery struct {
	GetReceivedCallback EmptyData `json:"get_received_callback"`
}

type TesterNftContractQuery struct {
	GetNftContract EmptyData `json:"get_nft_contract"`
}

// ContractInfoQueryData cw721 contract info query.
type ContractInfoQueryData struct{}
type ContractInfoQuery struct {
	ContractInfo ContractInfoQueryData `json:"contract_info"`
}
type ContractInfoResponse struct {
	Name   string `json:"name"`
	Symbol string `json:"symbol"`
}

// LastAckQueryData Query for getting last ACK from tester contract.
type LastAckQueryData struct{}
type LastAckQuery struct {
	LastAck LastAckQueryData `json:"last_ack"`
}

// NftInfoQueryData cw721 token info query
type NftInfoQueryData struct {
	TokenID string `json:"token_id"`
}
type NftInfoQuery struct {
	Nftinfo NftInfoQueryData `json:"nft_info"`
}
type NftInfoQueryResponse struct {
	TokenURI *string `json:"token_uri"`
}
