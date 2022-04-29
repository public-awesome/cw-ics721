package e2e_test

import (
	"fmt"
	"io/ioutil"
	"testing"
	"time"

	wasmkeeper "github.com/CosmWasm/wasmd/x/wasm/keeper"
	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	authtypes "github.com/cosmos/cosmos-sdk/x/auth/types"
	banktypes "github.com/cosmos/cosmos-sdk/x/bank/types"
	"github.com/public-awesome/stargaze/v4/testutil/simapp"
	"github.com/stretchr/testify/require"
	"github.com/tendermint/tendermint/crypto"
	"github.com/tendermint/tendermint/crypto/secp256k1"
	tmproto "github.com/tendermint/tendermint/proto/tendermint/types"
)

var (
	// whitelist

	instantiateMinterTemplate = `
		{
			"base_token_uri": "ipfs://...",
			"num_tokens": 100,
			"sg721_code_id": 3,
			"sg721_instantiate_msg": {
			  "name": "Collection Name",
			  "symbol": "SYM",
			  "minter": "%s",
			  "collection_info": {
				"contract_uri": "ipfs://...",
				"creator": "%s",
				"description": "Stargaze Monkeys",
				"image": "https://example.com/image.png",
				"external_link" : "https://stargaze.zone",
				"royalty_info": {
				  "payment_address": "%s",
				  "share": "0.1"
				}
			  }
			},
			"start_time": "%d",
			"whitelist" : %s, 
			"per_address_limit": %d,
			"unit_price": {
			  "amount": "100000000",
			  "denom": "ustars"
			}
		  }	  
		`

	escrow721Template = `
		{
			"name": "escrow721Channel1transfer-nft",
			"symbol": "esw721_1_transfer-nft",
			"minter": "%s" 
		}	  
		`
)

type Account struct {
	PrivKey secp256k1.PrivKey
	PubKey  crypto.PubKey
	Address sdk.AccAddress
}

func GetAccounts() []Account {
	accounts := make([]Account, 0, 150)
	for i := 0; i < 150; i++ {
		priv := secp256k1.GenPrivKey()
		pub := priv.PubKey()
		addr := sdk.AccAddress(pub.Address())
		acc := Account{
			PrivKey: priv,
			PubKey:  pub,
			Address: addr,
		}
		accounts = append(accounts, acc)
	}
	return accounts
}

func GetAccountsAndBalances(accs []Account) ([]authtypes.GenesisAccount, []banktypes.Balance) {
	genAccs := make([]authtypes.GenesisAccount, 0, len(accs))
	balances := make([]banktypes.Balance, 0, len(accs))
	for _, a := range accs {
		genAcc := authtypes.BaseAccount{
			Address: a.Address.String(),
		}
		balance := banktypes.Balance{
			Address: a.Address.String(),
			Coins:   sdk.NewCoins(sdk.NewInt64Coin("ustars", 2_000_000_000)),
		}
		genAccs = append(genAccs, &genAcc)
		balances = append(balances, balance)
	}
	return genAccs, balances
}
func TestICS721(t *testing.T) {
	accs := GetAccounts()

	genAccs, balances := GetAccountsAndBalances(accs)

	app := simapp.SetupWithGenesisAccounts(t, t.TempDir(), genAccs, balances...)

	startDateTime, err := time.Parse(time.RFC3339Nano, "2022-03-11T20:59:00Z")
	require.NoError(t, err)
	ctx := app.BaseApp.NewContext(false, tmproto.Header{Height: 1, ChainID: "stargaze-1", Time: startDateTime})

	// wasm params
	wasmParams := app.WasmKeeper.GetParams(ctx)
	wasmParams.CodeUploadAccess = wasmtypes.AllowEverybody
	wasmParams.MaxWasmCodeSize = 1000 * 1024 * 4 // 4MB
	app.WasmKeeper.SetParams(ctx, wasmParams)

	priv1 := secp256k1.GenPrivKey()
	pub1 := priv1.PubKey()
	addr1 := sdk.AccAddress(pub1.Address())

	b, err := ioutil.ReadFile("contracts/ics721.wasm")
	require.NoError(t, err)

	msgServer := wasmkeeper.NewMsgServerImpl(wasmkeeper.NewDefaultPermissionKeeper(app.WasmKeeper))
	res, err := msgServer.StoreCode(sdk.WrapSDKContext(ctx), &wasmtypes.MsgStoreCode{
		Sender:       addr1.String(),
		WASMByteCode: b,
	})
	require.NoError(t, err)
	require.NotNil(t, res)
	require.Equal(t, res.CodeID, uint64(1))
	println("ICS721.wasm has loaded!")

	b, err = ioutil.ReadFile("contracts/escrow721.wasm")
	require.NoError(t, err)

	res, err = msgServer.StoreCode(sdk.WrapSDKContext(ctx), &wasmtypes.MsgStoreCode{
		Sender:       addr1.String(),
		WASMByteCode: b,
	})
	require.NoError(t, err)
	require.NotNil(t, res)
	require.Equal(t, res.CodeID, uint64(2))
	println("escrow721.wasm has loaded!")

	creator := accs[0]

	instantiateMsgRaw := []byte(
		fmt.Sprintf(escrow721Template,
			creator.Address.String(),
		),
	)
	instantiateRes, err := msgServer.InstantiateContract(sdk.WrapSDKContext(ctx), &wasmtypes.MsgInstantiateContract{
		Sender: creator.Address.String(),
		Admin:  creator.Address.String(),
		CodeID: 2,
		Label:  "Escrow721",
		Msg:    instantiateMsgRaw,
		Funds:  sdk.NewCoins(sdk.NewInt64Coin("ustars", 1_000_000_000)),
	})
	require.NoError(t, err)
	require.NotNil(t, instantiateRes)
	require.NotEmpty(t, instantiateRes.Address)

	escrow721Address := instantiateRes.Address

	escrow721MintTemplate := `
	{ "mint": {
		"token_id": "%s",
		"owner": "%s",
		"token_uri": "ipfs://abc123",
		"extension": {}
		}
	}
	`
	mintMsgRaw := []byte(
		fmt.Sprintf(escrow721MintTemplate,
			"1",
			creator.Address.String(),
		),
	)
	_, mintErr := msgServer.ExecuteContract(sdk.WrapSDKContext(ctx), &wasmtypes.MsgExecuteContract{
		Contract: escrow721Address,
		Sender:   accs[0].Address.String(),
		Msg:      mintMsgRaw,
	})
	require.NoError(t, err)
	require.NotNil(t, instantiateRes)
	require.NotEmpty(t, instantiateRes.Address)
	require.NoError(t, mintErr)

	mintMsgRaw = []byte(
		fmt.Sprintf(escrow721MintTemplate,
			"2",
			creator.Address.String(),
		),
	)
	_, mintErr = msgServer.ExecuteContract(sdk.WrapSDKContext(ctx), &wasmtypes.MsgExecuteContract{
		Contract: escrow721Address,
		Sender:   creator.Address.String(),
		Msg:      mintMsgRaw,
	})
	require.NoError(t, err)
	require.NotNil(t, instantiateRes)
	require.NotEmpty(t, instantiateRes.Address)
	require.NoError(t, mintErr)

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, err := app.WasmKeeper.QuerySmart(ctx, addr, []byte(`{"owner_of": {"token_id": "1"}}`))
	expected_result := fmt.Sprintf("{\"owner\":\"%s\",\"approvals\":[]}", creator.Address.String())
	require.Equal(t, string(result), expected_result)
	require.NoError(t, err)

	// q.

	// pub struct MintMsg<T> {
	// 	/// Unique ID of the NFT
	// 	pub token_id: String,
	// 	/// The owner of the newly minter NFT
	// 	pub owner: String,
	// 	/// Universal resource identifier for this NFT
	// 	/// Should point to a JSON file that conforms to the ERC721
	// 	/// Metadata JSON Schema
	// 	pub token_uri: Option<String>,
	// 	/// Any custom extension used by this contract
	// 	pub extension: T,
	// }

}
