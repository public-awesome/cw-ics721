package e2e_test

type InstantiateICS721Bridge struct {
	CW721CodeID uint64 `json:"cw721_base_code_id"`
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

// Owner query for bridge contract.
type OwnerQueryData struct {
	TokenID string `json:"token_id"`
	ClassID string `json:"class_id"`
}
type OwnerQuery struct {
	Owner OwnerQueryData `json:"owner"`
}

// Bridge query for obtaining a NFT contract address given a class ID.
type NftContractForClassIdQueryData struct {
	ClassID string `json:"class_id"`
}
type NftContractForClassIdQuery struct {
	NftContractForClassId NftContractForClassIdQueryData `json:"nft_contract_for_class_id"`
}

// Query for getting class ID given NFT contract.
type ClassIdForNFTContractQueryData struct {
	Contract string `json:"contract"`
}
type ClassIdForNFTContractQuery struct {
	ClassIdForNFTContract ClassIdForNFTContractQueryData `json:"class_id_for_nft_contract"`
}

// Query for getting metadata for a class ID from the bridge.
type MetadataQueryData struct {
	ClassId string `json:"class_id"`
}
type MetadataQuery struct {
	Metadata MetadataQueryData `json:"metadata"`
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
