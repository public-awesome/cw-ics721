package e2e_test

import (
	"io/ioutil"
	"testing"
	"time"

	wasmkeeper "github.com/CosmWasm/wasmd/x/wasm/keeper"
	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	"github.com/cosmos/cosmos-sdk/crypto/keys/secp256k1"
	sdk "github.com/cosmos/cosmos-sdk/types"
	authtypes "github.com/cosmos/cosmos-sdk/x/auth/types"
	banktypes "github.com/cosmos/cosmos-sdk/x/bank/types"
	"github.com/public-awesome/stargaze/v12/app"
	"github.com/public-awesome/stargaze/v12/testutil/simapp"
	"github.com/stretchr/testify/require"
	tmproto "github.com/tendermint/tendermint/proto/tendermint/types"
)

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

func LoadChain(t *testing.T) (sdk.AccAddress, sdk.Context, *app.App, []Account) {
	accs := GetAccounts()
	genAccs, balances := GetAccountsAndBalances(accs)

	app := simapp.SetupWithGenesisAccounts(t, t.TempDir(), genAccs, balances...)

	startDateTime, err := time.Parse(time.RFC3339Nano, "2022-03-11T20:59:00Z")
	require.NoError(t, err)
	ctx := app.BaseApp.NewContext(false, tmproto.Header{Height: 1, ChainID: "stargaze-1", Time: startDateTime})

	// wasm params
	wasmParams := app.WasmKeeper.GetParams(ctx)
	wasmParams.CodeUploadAccess = wasmtypes.AllowEverybody

	app.WasmKeeper.SetParams(ctx, wasmParams)

	priv1 := secp256k1.GenPrivKey()
	pub1 := priv1.PubKey()
	addr1 := sdk.AccAddress(pub1.Address())
	return addr1, ctx, app, accs
}

// Stores a WASM file given a FILE path. Tests are run from the e2e
// directory so paths should be relative to that.
func storeWasmFile(t *testing.T, file string, creator sdk.AccAddress, ctx sdk.Context, app *app.App) uint64 {
	b, err := ioutil.ReadFile(file)
	require.NoError(t, err)

	// For reasons entirely beyond me we need to create a new
	// message server for every store code operation. Otherwise,
	// every stored code will have a code ID of 1.
	msgServer := wasmkeeper.NewMsgServerImpl(wasmkeeper.NewDefaultPermissionKeeper(app.WasmKeeper))

	res, err := msgServer.StoreCode(sdk.WrapSDKContext(ctx), &wasmtypes.MsgStoreCode{
		Sender:       creator.String(),
		WASMByteCode: b,
	})
	require.NoError(t, err)
	require.NotNil(t, res)

	return res.CodeID

}

func StoreICS721Bridge(t *testing.T, creator sdk.AccAddress, ctx sdk.Context, app *app.App) uint64 {
	return storeWasmFile(t, "../artifacts/ics721_base.wasm", creator, ctx, app)
}

func StoreCw721Base(t *testing.T, creator sdk.AccAddress, ctx sdk.Context, app *app.App) uint64 {
	return storeWasmFile(t, "../external-wasms/cw721_base_v0.18.0.wasm", creator, ctx, app)
}

func TestLoadChain(t *testing.T) {
	LoadChain(t)
}

func TestStoreBridge(t *testing.T) {
	creator, ctx, app, _ := LoadChain(t)
	bridgeCodeID := StoreICS721Bridge(t, creator, ctx, app)
	require.Equal(t, uint64(1), bridgeCodeID)
}

func TestStoreCw721(t *testing.T) {
	creator, ctx, app, _ := LoadChain(t)
	cw721CodeID := StoreCw721Base(t, creator, ctx, app)
	require.Equal(t, uint64(1), cw721CodeID)
}

func TestStoreMultiple(t *testing.T) {
	creator, ctx, app, _ := LoadChain(t)

	bridgeCodeID := StoreICS721Bridge(t, creator, ctx, app)
	require.Equal(t, uint64(1), bridgeCodeID)

	cw721CodeID := StoreCw721Base(t, creator, ctx, app)
	require.Equal(t, uint64(2), cw721CodeID)
}

func TestInstantiateBridge(t *testing.T) {
	creator, ctx, app, _ := LoadChain(t)

	bridgeCodeID := StoreICS721Bridge(t, creator, ctx, app)
	require.Equal(t, uint64(1), bridgeCodeID)

	cw721CodeID := StoreCw721Base(t, creator, ctx, app)
	require.Equal(t, uint64(2), cw721CodeID)

	InstantiateBridge(t, ctx, app, creator.String(), cw721CodeID, bridgeCodeID)
}
