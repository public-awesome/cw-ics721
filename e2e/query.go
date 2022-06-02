package e2e_test

import (
	"fmt"
	"testing"

	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/public-awesome/stargaze/v4/app"
	"github.com/stretchr/testify/require"
)

func RunQuerySuccess(t *testing.T, ctx sdk.Context, app *app.App,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, creator Account) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, err := app.WasmKeeper.QuerySmart(
		ctx, addr, []byte(`{"owner_of": {"token_id": "1", "class_id": "omni/stars/transfer-nft"}}`))
	expected_result := fmt.Sprintf("{\"owner\":\"%s\",\"approvals\":[]}", creator.Address.String())
	require.Equal(t, string(result), expected_result)
	require.NoError(t, err)
}

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
