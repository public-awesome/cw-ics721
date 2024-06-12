package test_suite

import (
	"testing"

	wasmibctesting "github.com/CosmWasm/wasmd/x/wasm/ibctesting"
	// "github.com/cosmos/interchain-accounts/app"
	"github.com/public-awesome/stargaze/v12/app"

	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/stretchr/testify/require"
)

func QueryGetNftForClass(t *testing.T, chain *wasmibctesting.TestChain, bridge, classID string) string {
	getClassQuery := NftContractQuery{
		NftContractForClassId: NftContractQueryData{
			ClassID: classID,
		},
	}
	cw721 := ""
	err := chain.SmartQuery(bridge, getClassQuery, &cw721)
	require.NoError(t, err)
	return cw721
}

func QueryGetNftContracts(t *testing.T, chain *wasmibctesting.TestChain, bridge string) [][]string {
	getClassQuery := NftContractsQuery{
		NftContracts: NftContractsQueryData{},
	}
	var cw721 [][]string
	err := chain.SmartQuery(bridge, getClassQuery, &cw721)
	require.NoError(t, err)
	return cw721
}

func QueryGetOwnerOf(t *testing.T, chain *wasmibctesting.TestChain, nft string, tokenId string) string {
	resp := OwnerOfResponse{}
	ownerOfQuery := OwnerOfQuery{
		OwnerOf: OwnerOfQueryData{
			TokenID: tokenId,
		},
	}
	err := chain.SmartQuery(nft, ownerOfQuery, &resp)
	require.NoError(t, err)
	return resp.Owner
}

func QueryGetOwnerOfErr(t *testing.T, chain *wasmibctesting.TestChain, nft string, tokenId string) error {
	resp := OwnerOfResponse{}
	ownerOfQuery := OwnerOfQuery{
		OwnerOf: OwnerOfQueryData{
			TokenID: tokenId,
		},
	}
	err := chain.SmartQuery(nft, ownerOfQuery, &resp)
	return err
}

// Tester queries and Tester responses

func QueryTesterSent(t *testing.T, chain *wasmibctesting.TestChain, tester string) string {
	resp := TesterResponse{}
	testerSentQuery := TesterSentQuery{
		GetSentCallback: EmptyData{},
	}
	err := chain.SmartQuery(tester, testerSentQuery, &resp)
	require.NoError(t, err)
	return *resp.Owner
}

func QueryTesterReceived(t *testing.T, chain *wasmibctesting.TestChain, tester string) string {
	resp := TesterResponse{}
	testerReceivedQuery := TesterReceivedQuery{
		GetReceivedCallback: EmptyData{},
	}
	err := chain.SmartQuery(tester, testerReceivedQuery, &resp)
	require.NoError(t, err)
	return *resp.Owner
}

func QueryTesterNftContract(t *testing.T, chain *wasmibctesting.TestChain, tester string) string {
	resp := ""
	testerReceivedQuery := TesterNftContractQuery{
		GetNftContract: EmptyData{},
	}
	err := chain.SmartQuery(tester, testerReceivedQuery, &resp)
	require.NoError(t, err)
	return resp
}

func QueryTesterReceivedErr(_ *testing.T, chain *wasmibctesting.TestChain, tester string) error {
	resp := TesterResponse{}
	testerReceivedQuery := TesterReceivedQuery{
		GetReceivedCallback: EmptyData{},
	}
	err := chain.SmartQuery(tester, testerReceivedQuery, &resp)
	return err
}

// TODO Renamce Run to Query and remove redundancies

func RunQueryEmpty(t *testing.T, ctx sdk.Context, app *app.App,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, creator Account, queryMsgRaw []byte) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, err := app.WasmKeeper.QuerySmart(
		ctx, addr, queryMsgRaw)
	expectedResult := ""
	require.Equal(t, string(result), expectedResult)
	require.EqualError(t, err, "cw721_base_ibc::state::TokenInfo<cosmwasm_std::results::empty::Empty> not found: query wasm contract failed")
}

func RunGetOwner(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, getOwnerMsgRaw []byte, expectedResponse string) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, _ := app.WasmKeeper.QuerySmart(
		ctx, addr, getOwnerMsgRaw)
	require.Equal(t, string(result), expectedResponse)
}

func RunGetNFTInfo(t *testing.T, ctx sdk.Context, app *app.App, msgServer wasmtypes.MsgServer, accs []Account,
	instantiateRes *wasmtypes.MsgInstantiateContractResponse, getNFTInfoMsgRaw []byte, expectedResponse string) {
	escrow721Address := instantiateRes.Address

	addr, _ := sdk.AccAddressFromBech32(escrow721Address)
	result, _ := app.WasmKeeper.QuerySmart(
		ctx, addr, getNFTInfoMsgRaw)

	require.Equal(t, string(result), expectedResponse)
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
