package e2e

var (
	escrow721Template = `
		{
			"name": "escrow721Channel1transfer-nft",
			"symbol": "esw721_1_transfer-nft",
			"minter": "%s" 
		}	  
		`

	escrow721MintTemplate = `
	{ "mint": {
		"class_id": "%s",
		"token_id": "%s",
		"owner": "%s",
		"token_uri": "ipfs://abc123",
		"extension": {}
		}
	}
	`

	escrow721BurnTemplate = `
	{ "burn": {
		"class_id": "%s",
		"token_id": "%s"
		}
	}
	`

	escrow721GetOwnerTemplate = `
	{
		"owner_of": { 
			"class_id": "%s",
			 "token_id": "%s"}
			}
	`

	escrow721GetNFTInfoTemplate = `
	{
		"nft_info": { 
			"class_id": "%s",
			 "token_id": "%s"}
			}
	`

	escrow721TransferNFTTemplate = `
	{
		"transfer_nft": { 
			"class_id": "%s",
			"token_id": "%s",
			"recipient": "%s"}
	}
	`
	escrow721SaveClassTemplate = `
	{
		"save_class": { 
			"class_id": "%s",
			"class_uri": "%s"}
	}
	`

	escrow721HasClassTemplate = `
	{
		"has_class": { 
			"class_id": "%s"}
	}
	`
	escrow721GetClassTemplate = `
	{
		"get_class": { 
			"class_id": "%s"}
	}
	`
)
