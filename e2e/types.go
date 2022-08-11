package e2e_test

type InstantiateICS721Bridge struct {
	CW721CodeID  uint64 `json:"cw721_base_code_id"`
	EscrowCodeID uint64 `json:"escrow_code_id"`
}

type InstantiateCw721 struct {
	Name   string `json:"name"`
	Symbol string `json:"symbol"`
	Minter string `json:"minter"`
}

type OwnerOfResponse struct {
	Owner string `json:"owner"`
	// There is also an approvals field here but we don't care
	// about it so we just don't unmarshal.
}

type GetOwnerQueryData struct {
	TokenID string `json:"token_id"`
	ClassID string `json:"class_id"`
}
type GetOwnerQuery struct {
	GetOwner GetOwnerQueryData `json:"get_owner"`
}

type OwnerOfQueryData struct {
	TokenID string `json:"token_id"`
}
type OwnerOfQuery struct {
	OwnerOf OwnerOfQueryData `json:"owner_of"`
}

type GetClassQueryData struct {
	ClassID string `json:"class_id"`
}
type GetClassQuery struct {
	GetClass GetClassQueryData `json:"get_class"`
}

type ContractInfoQueryData struct{}
type ContractInfoQuery struct {
	ContractInfo ContractInfoQueryData `json:"contract_info"`
}
type ContractInfoResponse struct {
	Name   string `json:"name"`
	Symbol string `json:"symbol"`
}
