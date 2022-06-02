package e2e_test

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
)
