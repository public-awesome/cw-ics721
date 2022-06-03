package e2e_test

import (
	"fmt"
	"testing"

	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/public-awesome/stargaze/v4/app"
	"github.com/stretchr/testify/require"
)

func RunQueryEmpty(t *testing.T, ctx sdk.Context, app *app.App,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, creator Account) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, err := app.WasmKeeper.QuerySmart(
		ctx, addr, []byte(`{"owner_of": {"token_id": "1", "class_id": "omni/stars/transfer-nft"}}`))
	expected_result := ""
	require.Equal(t, string(result), expected_result)
	require.EqualError(t, err, "cw721_base_ibc::state::TokenInfo<cosmwasm_std::results::empty::Empty> not found: query wasm contract failed")
}

func RunGetOwner(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, getOwnerMsgRaw []byte, owner sdk.Address) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, _ := app.WasmKeeper.QuerySmart(
		ctx, addr, getOwnerMsgRaw)
	expected_result := string(fmt.Sprintf(`{"owner":"%s","approvals":[]}`, owner.String()))
	require.Equal(t, string(result), expected_result)
}

func RunGetNFTInfo(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, getNFTInfoMsgRaw []byte, err error) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, _ := app.WasmKeeper.QuerySmart(
		ctx, addr, getNFTInfoMsgRaw)

	expected_result := string(`{"token_uri":"ipfs://abc123","extension":{}}`)
	require.Equal(t, string(result), expected_result)
}

func RunHasClass(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, hasClassMsgRaw []byte, expected string) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, _ := app.WasmKeeper.QuerySmart(
		ctx, addr, hasClassMsgRaw)

	require.Equal(t, string(expected), string(result))
}
