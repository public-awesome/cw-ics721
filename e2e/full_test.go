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
	"github.com/public-awesome/stargaze/v4/app"
	"github.com/public-awesome/stargaze/v4/testutil/simapp"
	"github.com/stretchr/testify/require"
	"github.com/tendermint/tendermint/crypto/secp256k1"
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

func LoadChain(t *testing.T) (addr1 sdk.AccAddress, ctx sdk.Context, app *app.App, accs []Account) {
	accs = GetAccounts()
	genAccs, balances := GetAccountsAndBalances(accs)

	app = simapp.SetupWithGenesisAccounts(t, t.TempDir(), genAccs, balances...)

	startDateTime, err := time.Parse(time.RFC3339Nano, "2022-03-11T20:59:00Z")
	require.NoError(t, err)
	ctx = app.BaseApp.NewContext(false, tmproto.Header{Height: 1, ChainID: "stargaze-1", Time: startDateTime})

	// wasm params
	wasmParams := app.WasmKeeper.GetParams(ctx)
	wasmParams.CodeUploadAccess = wasmtypes.AllowEverybody
	wasmParams.MaxWasmCodeSize = 1000 * 1024 * 4 // 4MB
	app.WasmKeeper.SetParams(ctx, wasmParams)

	priv1 := secp256k1.GenPrivKey()
	pub1 := priv1.PubKey()
	addr1 = sdk.AccAddress(pub1.Address())
	return addr1, ctx, app, accs
}

func LoadICS721(t *testing.T, addr1 sdk.AccAddress, ctx sdk.Context, app *app.App) (
	msgServer wasmtypes.MsgServer, err error) {
	b, err := ioutil.ReadFile("contracts/ics721.wasm")
	require.NoError(t, err)

	msgServer = wasmkeeper.NewMsgServerImpl(wasmkeeper.NewDefaultPermissionKeeper(app.WasmKeeper))
	res, err := msgServer.StoreCode(sdk.WrapSDKContext(ctx), &wasmtypes.MsgStoreCode{
		Sender:       addr1.String(),
		WASMByteCode: b,
	})
	require.NoError(t, err)
	require.NotNil(t, res)
	require.Equal(t, res.CodeID, uint64(1))
	println("ICS721.wasm has loaded!")
	return msgServer, err
}

func LoadEscrow721(t *testing.T, addr1 sdk.AccAddress, ctx sdk.Context,
	app *app.App, msgServer wasmtypes.MsgServer) {
	b, err := ioutil.ReadFile("contracts/escrow721.wasm")
	require.NoError(t, err)

	res, err := msgServer.StoreCode(sdk.WrapSDKContext(ctx), &wasmtypes.MsgStoreCode{
		Sender:       addr1.String(),
		WASMByteCode: b,
	})
	require.NoError(t, err)
	require.NotNil(t, res)
	require.Equal(t, res.CodeID, uint64(2))
	println("escrow721.wasm has loaded!")
}

func MintTwoNFTs(t *testing.T) (
	app *app.App, ctx sdk.Context,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, accs []Account,
	msgServer wasmtypes.MsgServer, err error) {
	addr1, ctx, app, accs := LoadChain(t)
	msgServer, err = LoadICS721(t, addr1, ctx, app)
	LoadEscrow721(t, addr1, ctx, app, msgServer)
	instantiateRes = InstantiateEscrow721(t, ctx, msgServer, accs)
	creator := accs[0]

	mintMsgRaw := []byte(
		fmt.Sprintf(escrow721MintTemplate,
			"omni/stars/transfer-nft",
			"1",
			creator.Address.String(),
		),
	)
	ExecuteMint(t, ctx, msgServer, accs, instantiateRes, mintMsgRaw, err)
	mintMsgRaw = []byte(
		fmt.Sprintf(escrow721MintTemplate,
			"omni/stars/transfer-nft",
			"2",
			creator.Address.String(),
		),
	)
	ExecuteMint(t, ctx, msgServer, accs, instantiateRes, mintMsgRaw, err)
	return app, ctx, instantiateRes, accs, msgServer, err
}

func SaveClass(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, err error) {
	saveClassMsgRaw := []byte(fmt.Sprintf(escrow721SaveClassTemplate,
		"omni/stars/transfer-nft",
		"abc123_class_uri",
	))
	ExecuteSaveClass(t, ctx, app, msgServer, accs, instantiateRes, saveClassMsgRaw, err)
}

func TestLoadChain(t *testing.T) {
	LoadChain(t)
}

func TestMinting(t *testing.T) {
	MintTwoNFTs(t)
}

func TestBurn(t *testing.T) {
	app, ctx, instantiateRes, accs, msgServer, err := MintTwoNFTs(t)
	burnMsgRaw := []byte(
		fmt.Sprintf(escrow721BurnTemplate,
			"omni/stars/transfer-nft",
			"1",
		),
	)
	ExecuteBurn(t, ctx, msgServer, accs, instantiateRes, burnMsgRaw, err)
	RunQueryEmpty(t, ctx, app, instantiateRes, accs[0])
	ExecuteBurnError(t, ctx, msgServer, accs, instantiateRes, burnMsgRaw, err)

	burnMsgRawFake := []byte(
		fmt.Sprintf(escrow721BurnTemplate,
			"super_fake_class",
			"1",
		),
	)
	ExecuteBurnError(t, ctx, msgServer, accs, instantiateRes, burnMsgRawFake, err)
}

func TestGetOwner(t *testing.T) {
	app, ctx, instantiateRes, accs, msgServer, _ := MintTwoNFTs(t)
	getOwnerMsgRaw := []byte(fmt.Sprintf(escrow721GetOwnerTemplate,
		"omni/stars/transfer-nft",
		"1",
	))
	RunGetOwner(t, ctx, app, msgServer, accs, instantiateRes, getOwnerMsgRaw, accs[0].Address)

}

func TestGetNFTInfo(t *testing.T) {
	app, ctx, instantiateRes, accs, msgServer, err := MintTwoNFTs(t)
	getNFTInfoMsgRaw := []byte(fmt.Sprintf(escrow721GetNFTInfoTemplate,
		"omni/stars/transfer-nft",
		"1",
	))
	RunGetNFTInfo(t, ctx, app, msgServer, accs, instantiateRes, getNFTInfoMsgRaw, err)

}

func TestTransfer(t *testing.T) {
	app, ctx, instantiateRes, accs, msgServer, err := MintTwoNFTs(t)
	transferMsgRaw := []byte(fmt.Sprintf(escrow721TransferNFTTemplate,
		"omni/stars/transfer-nft",
		"1",
		accs[1].Address.String(),
	))
	ExecuteTransferNFT(t, ctx, app, msgServer, accs, instantiateRes, transferMsgRaw, err)

	getOwnerMsgRaw := []byte(fmt.Sprintf(escrow721GetOwnerTemplate,
		"omni/stars/transfer-nft",
		"1",
	))
	RunGetOwner(t, ctx, app, msgServer, accs, instantiateRes, getOwnerMsgRaw, accs[1].Address)
}

func TestSaveClass(t *testing.T) {
	app, ctx, instantiateRes, accs, msgServer, err := MintTwoNFTs(t)
	SaveClass(t, ctx, app, msgServer, accs, instantiateRes, err)
}

func TestHasClassTrue(t *testing.T) {
	app, ctx, instantiateRes, accs, msgServer, err := MintTwoNFTs(t)
	SaveClass(t, ctx, app, msgServer, accs, instantiateRes, err)

	hasClassMsgRaw := []byte(fmt.Sprintf(escrow721HasClassTemplate,
		"omni/stars/transfer-nft",
	))
	RunHasClass(t, ctx, app, msgServer, accs, instantiateRes, hasClassMsgRaw, "true")
}

func TestHasClassFalse(t *testing.T) {
	app, ctx, instantiateRes, accs, msgServer, err := MintTwoNFTs(t)
	SaveClass(t, ctx, app, msgServer, accs, instantiateRes, err)

	hasClassMsgRaw := []byte(fmt.Sprintf(escrow721HasClassTemplate,
		"omni/fake-channel/transfer-nft",
	))
	RunHasClass(t, ctx, app, msgServer, accs, instantiateRes, hasClassMsgRaw, "false")
}
