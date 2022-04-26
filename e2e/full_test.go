package e2e_test

import (
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

	instantiateWhiteListTemplate = `
		{
			"members":[%s],
			"start_time": "%d",
			"end_time": "%d",
			"unit_price": {
				"amount": "50000000",
				"denom": "ustars"
			},
			"per_address_limit": 1,
			"member_limit": 1000
		}
		`

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
func TestMinter(t *testing.T) {
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

	// minter
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
}
