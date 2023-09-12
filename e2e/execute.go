package e2e_test

import (
	"fmt"
	"testing"

	wasmkeeper "github.com/CosmWasm/wasmd/x/wasm/keeper"
	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/public-awesome/stargaze/v12/app"
	"github.com/stretchr/testify/require"
)

func InstantiateBridge(t *testing.T, ctx sdk.Context, app *app.App, creatorAddress string, cw721CodeID uint64, bridgeCodeID uint64) *wasmtypes.MsgInstantiateContractResponse {
	msgServer := wasmkeeper.NewMsgServerImpl(wasmkeeper.NewDefaultPermissionKeeper(app.WasmKeeper))

	instantiateMsgRaw := []byte(fmt.Sprintf(`{ "cw721_base_code_id": %d }`, cw721CodeID))
	instantiateRes, err := msgServer.InstantiateContract(sdk.WrapSDKContext(ctx), &wasmtypes.MsgInstantiateContract{
		Sender: creatorAddress,
		Admin:  "",
		CodeID: bridgeCodeID,
		Label:  "ICS721 contract",
		Msg:    instantiateMsgRaw,
		Funds:  []sdk.Coin{},
	})
	require.NoError(t, err)
	return instantiateRes
}
