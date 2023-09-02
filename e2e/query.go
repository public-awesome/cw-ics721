package e2e_test

import (
	"testing"

	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/public-awesome/stargaze/v12/app"
	"github.com/stretchr/testify/require"
)

func RunQueryEmpty(t *testing.T, ctx sdk.Context, app *app.App,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, creator Account, queryMsgRaw []byte) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, err := app.WasmKeeper.QuerySmart(
		ctx, addr, queryMsgRaw)
	expected_result := ""
	require.Equal(t, string(result), expected_result)
	require.EqualError(t, err, "cw721_base_ibc::state::TokenInfo<cosmwasm_std::results::empty::Empty> not found: query wasm contract failed")
}

func RunGetOwner(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, getOwnerMsgRaw []byte, expected_response string) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, _ := app.WasmKeeper.QuerySmart(
		ctx, addr, getOwnerMsgRaw)
	require.Equal(t, string(result), expected_response)
}

func RunGetNFTInfo(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, getNFTInfoMsgRaw []byte, expected_response string) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, _ := app.WasmKeeper.QuerySmart(
		ctx, addr, getNFTInfoMsgRaw)

	require.Equal(t, string(result), expected_response)
}

func RunHasClass(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, hasClassMsgRaw []byte, expected string) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, _ := app.WasmKeeper.QuerySmart(
		ctx, addr, hasClassMsgRaw)

	require.Equal(t, string(expected), string(result))
}

func RunGetClass(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, getClassMsgRaw []byte, expected string) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, _ := app.WasmKeeper.QuerySmart(
		ctx, addr, getClassMsgRaw)

	require.Equal(t, string(expected), string(result))
}

func RunGetClassError(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, getClassMsgRaw []byte, expected string) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	_, err := app.WasmKeeper.QuerySmart(
		ctx, addr, getClassMsgRaw)

	require.EqualError(t, err, expected)
}
