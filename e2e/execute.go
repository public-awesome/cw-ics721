package e2e_test

import (
	"fmt"
	"testing"

	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/public-awesome/stargaze/v4/app"
	"github.com/stretchr/testify/require"
)

func InstantiateEscrow721(t *testing.T, ctx sdk.Context,
	msgServer wasmtypes.MsgServer, accs []Account) (
	instantiateRes *wasmtypes.MsgInstantiateContractResponse) {
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
	return instantiateRes
}

func ExecuteMint(t *testing.T, ctx sdk.Context, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, mintMsgRaw []byte, err error) (mintErr error) {
	escrow721Address := instantiateRes.Address

	_, mintErr = msgServer.ExecuteContract(sdk.WrapSDKContext(ctx), &wasmtypes.MsgExecuteContract{
		Contract: escrow721Address,
		Sender:   accs[0].Address.String(),
		Msg:      mintMsgRaw,
	})
	require.NoError(t, err)
	require.NotNil(t, instantiateRes)
	require.NotEmpty(t, instantiateRes.Address)
	require.NoError(t, mintErr)
	return mintErr
}

func ExecuteBurn(t *testing.T, ctx sdk.Context, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, burnMsgRaw []byte, err error) (burnErr error) {
	escrow721Address := instantiateRes.Address

	_, burnErr = msgServer.ExecuteContract(sdk.WrapSDKContext(ctx), &wasmtypes.MsgExecuteContract{
		Contract: escrow721Address,
		Sender:   accs[0].Address.String(),
		Msg:      burnMsgRaw,
	})
	require.NoError(t, err)
	require.NotNil(t, instantiateRes)
	require.NotEmpty(t, instantiateRes.Address)
	require.NoError(t, burnErr)
	return burnErr
}

func ExecuteBurnError(t *testing.T, ctx sdk.Context, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, burnMsgRaw []byte, err error) (burnErr error) {
	escrow721Address := instantiateRes.Address

	_, burnErr = msgServer.ExecuteContract(sdk.WrapSDKContext(ctx), &wasmtypes.MsgExecuteContract{
		Contract: escrow721Address,
		Sender:   accs[0].Address.String(),
		Msg:      burnMsgRaw,
	})
	require.NoError(t, err)
	require.NotNil(t, instantiateRes)
	require.NotEmpty(t, instantiateRes.Address)
	require.EqualError(t, burnErr, "cw721_base_ibc::state::TokenInfo<cosmwasm_std::results::empty::Empty> not found: execute wasm contract failed")
	return burnErr
}

func ExecuteTransferNFT(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, transferMsgRaw []byte, err error) {
	escrow721Address := instantiateRes.Address

	_, transferErr := msgServer.ExecuteContract(sdk.WrapSDKContext(ctx), &wasmtypes.MsgExecuteContract{
		Contract: escrow721Address,
		Sender:   accs[0].Address.String(),
		Msg:      transferMsgRaw,
	})
	require.NoError(t, err)
	require.NotNil(t, instantiateRes)
	require.NotEmpty(t, instantiateRes.Address)
	require.NoError(t, transferErr)
}

func ExecuteSaveClass(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, saveClassMsgRaw []byte, err error) {
	escrow721Address := instantiateRes.Address

	_, transferErr := msgServer.ExecuteContract(sdk.WrapSDKContext(ctx), &wasmtypes.MsgExecuteContract{
		Contract: escrow721Address,
		Sender:   accs[0].Address.String(),
		Msg:      saveClassMsgRaw,
	})
	require.NoError(t, err)
	require.NotNil(t, instantiateRes)
	require.NotEmpty(t, instantiateRes.Address)
	require.NoError(t, transferErr)
}
