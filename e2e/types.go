package e2e_test

type InstantiateICS721 struct {
	DefaultTimeout uint64 `json:"default_timeout"`
	CW721CodeID    uint64 `json:"cw721_ibc_code_id"`
	Label          string `json:"label"`
}
