package e2e

import (
	"encoding/json"
	"fmt"
	"github.com/public-awesome/ics721/e2e/test_suite"
	"testing"

	wasmibctesting "github.com/CosmWasm/wasmd/x/wasm/ibctesting"
	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	channeltypes "github.com/cosmos/ibc-go/v4/modules/core/04-channel/types"
	ibctesting "github.com/cosmos/ibc-go/v4/testing"
	"github.com/stretchr/testify/require"
	"github.com/stretchr/testify/suite"
)

type MigrateUpdateTestSuite struct {
	suite.Suite

	coordinator *wasmibctesting.Coordinator

	// testing chains used for convenience and readability
	chainA *wasmibctesting.TestChain
	chainB *wasmibctesting.TestChain

	bridgeA sdk.AccAddress
	bridgeB sdk.AccAddress

	pathAB *wasmibctesting.Path

	testerA sdk.AccAddress
	testerB sdk.AccAddress

	cw721A sdk.AccAddress
	cw721B sdk.AccAddress
}

func TestMigrateWithUpgrade(t *testing.T) {
	suite.Run(t, new(MigrateUpdateTestSuite))
}

func (suite *MigrateUpdateTestSuite) SetupTest() {
	suite.coordinator = wasmibctesting.NewCoordinator(suite.T(), 3)
	suite.chainA = suite.coordinator.GetChain(wasmibctesting.GetChainID(0))
	suite.chainB = suite.coordinator.GetChain(wasmibctesting.GetChainID(1))

	// Store codes and instantiate contracts
	storeCodes := func(chain *wasmibctesting.TestChain, bridge *sdk.AccAddress, tester *sdk.AccAddress, num int) {
		resp := chain.StoreCodeFile("../artifacts/ics721_base.wasm")
		require.Equal(suite.T(), uint64(1), resp.CodeID)

		resp = chain.StoreCodeFile("../external-wasms/cw721_base_v0.16.0.wasm")
		require.Equal(suite.T(), uint64(2), resp.CodeID)

		resp = chain.StoreCodeFile("../artifacts/ics721_base_tester.wasm")
		require.Equal(suite.T(), uint64(3), resp.CodeID)

		// store a newer version of the cw721 contract
		resp = chain.StoreCodeFile("../external-wasms/cw721_base_v0.17.0.wasm")
		require.Equal(suite.T(), uint64(4), resp.CodeID)

		// init dummy contracts based on how much we need
		for i := 0; i < num; i++ {
			cw721Instantiate := test_suite.InstantiateCw721v16{
				Name:   "bad/kids",
				Symbol: "bad/kids",
				Minter: suite.chainA.SenderAccount.GetAddress().String(),
			}
			instantiateRaw, err := json.Marshal(cw721Instantiate)
			require.NoError(suite.T(), err)

			chain.InstantiateContract(2, instantiateRaw)
		}

		// init ics721
		instantiateBridge := test_suite.InstantiateICS721Bridge{
			CW721CodeID:   2,
			OutgoingProxy: nil,
			IncomingProxy: nil,
			Pauser:        nil,
		}
		instantiateBridgeRaw, err := json.Marshal(instantiateBridge)
		require.NoError(suite.T(), err)

		*bridge = chain.InstantiateContract(1, instantiateBridgeRaw)

		// init tester
		instantiateBridgeTester := test_suite.InstantiateBridgeTester{
			AckMode: "success",
			Ics721:  bridge.String(),
		}
		instantiateBridgeTesterRaw, err := json.Marshal(instantiateBridgeTester)
		require.NoError(suite.T(), err)

		*tester = chain.InstantiateContract(3, instantiateBridgeTesterRaw)
	}

	storeCodes(suite.chainA, &suite.bridgeA, &suite.testerA, 0)
	storeCodes(suite.chainB, &suite.bridgeB, &suite.testerB, 3)

	// Helper function to init ibc paths
	initPath := func(chain1 *wasmibctesting.TestChain, chain2 *wasmibctesting.TestChain, contract1 sdk.AccAddress, contract2 sdk.AccAddress) *wasmibctesting.Path {
		sourcePortID := chain1.ContractInfo(contract1).IBCPortID
		counterpartPortID := chain2.ContractInfo(contract2).IBCPortID
		path := wasmibctesting.NewPath(chain1, chain2)
		path.EndpointA.ChannelConfig = &ibctesting.ChannelConfig{
			PortID:  sourcePortID,
			Version: "ics721-1",
			Order:   channeltypes.UNORDERED,
		}
		path.EndpointB.ChannelConfig = &ibctesting.ChannelConfig{
			PortID:  counterpartPortID,
			Version: "ics721-1",
			Order:   channeltypes.UNORDERED,
		}
		suite.coordinator.SetupConnections(path)
		suite.coordinator.CreateChannels(path)
		return path
	}

	// init ibc path between chains
	suite.pathAB = initPath(suite.chainA, suite.chainB, suite.bridgeA, suite.bridgeB)

	// init cw721 on chain A
	cw721Instantiate := test_suite.InstantiateCw721v16{
		Name:   "bad/kids",
		Symbol: "bad/kids",
		Minter: suite.chainA.SenderAccount.GetAddress().String(),
	}
	instantiateRaw, err := json.Marshal(cw721Instantiate)
	require.NoError(suite.T(), err)

	suite.cw721A = suite.chainA.InstantiateContract(2, instantiateRaw)

	_, err = suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: suite.cw721A.String(),
		Msg:      []byte(fmt.Sprintf(`{ "mint": { "token_id": "1", "owner": "%s" } }`, suite.chainA.SenderAccount.GetAddress().String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	//Send NFT to chain B
	test_suite.Ics721TransferNft(suite.T(), suite.chainA, suite.pathAB, suite.coordinator, suite.cw721A.String(), "1", suite.bridgeA, suite.chainA.SenderAccount.GetAddress(), suite.chainB.SenderAccount.GetAddress(), "")

	classIdChainB := fmt.Sprintf("%s/%s/%s", suite.pathAB.EndpointB.ChannelConfig.PortID, suite.pathAB.EndpointB.ChannelID, suite.cw721A.String())
	addr := test_suite.QueryGetNftForClass(suite.T(), suite.chainB, suite.bridgeB.String(), classIdChainB)
	suite.cw721B, err = sdk.AccAddressFromBech32(addr)
	require.NoError(suite.T(), err)

	// mint 2x working NFT to tester
	_, err = suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: suite.cw721A.String(),
		Msg:      []byte(fmt.Sprintf(`{ "mint": { "token_id": "2", "owner": "%s" } }`, suite.testerA.String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	_, err = suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: suite.cw721A.String(),
		Msg:      []byte(fmt.Sprintf(`{ "mint": { "token_id": "3", "owner": "%s" } }`, suite.testerA.String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	suite.T().Logf("chain A bridge = (%s)", suite.bridgeA.String())
	suite.T().Logf("chain B bridge = (%s)", suite.bridgeB.String())
	suite.T().Logf("chain A tester = (%s)", suite.testerA.String())
	suite.T().Logf("chain B tester = (%s)", suite.testerB.String())
	suite.T().Logf("chain A cw721) = (%s)", suite.cw721A.String())
	suite.T().Logf("chain B cw721) = (%s)", suite.cw721B.String())
}

func (suite *MigrateUpdateTestSuite) TestSuccessfulTransferWithMigrateUpdate() {
	memo := test_suite.CreateCallbackMemo(test_suite.NftCallbackSent(), "", test_suite.NftCallbackReceived(), "")

	// A -> B token_id 2
	test_suite.SendIcsFromChainToChain(suite.T(), suite.coordinator, suite.chainA, suite.bridgeA, suite.testerA, suite.testerB, suite.pathAB, suite.pathAB.EndpointA, suite.cw721A.String(), "2", memo, true)

	// Query the owner of NFT on cw721
	chainAOwner := test_suite.QueryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String(), "2")
	require.Equal(suite.T(), chainAOwner, suite.testerA.String())
	chainBOwner := test_suite.QueryGetOwnerOf(suite.T(), suite.chainB, suite.cw721B.String(), "2")
	require.Equal(suite.T(), chainBOwner, suite.testerB.String())

	// We query the data we have on the tester contract
	// This ensures that the callbacks are called after all the messages was completed
	// and the transfer was successful
	testerDataOwnerA := test_suite.QueryTesterSent(suite.T(), suite.chainA, suite.testerA.String())
	require.Equal(suite.T(), testerDataOwnerA, suite.bridgeA.String())
	testerNftContract := test_suite.QueryTesterNftContract(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerNftContract, suite.cw721B.String())
	testerDataOwnerB := test_suite.QueryTesterReceived(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerDataOwnerB, suite.testerB.String())

	// Execute a migration over the ics721 but keeping the same actual code ID. We just want to pass the with_update struct changing the cw721BaseCodeId
	test_suite.MigrateWIthUpdate(suite.T(), suite.chainA, suite.bridgeA.String(), 1, 4)

	// A -> B token_id 5
	test_suite.SendIcsFromChainToChain(suite.T(), suite.coordinator, suite.chainA, suite.bridgeA, suite.testerA, suite.testerB, suite.pathAB, suite.pathAB.EndpointA, suite.cw721A.String(), "5", memo, true)

	// Query the owner of NFT on cw721
	chainAOwner = test_suite.QueryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String(), "3")
	require.Equal(suite.T(), chainAOwner, suite.bridgeA.String())
	chainBOwner = test_suite.QueryGetOwnerOf(suite.T(), suite.chainB, suite.cw721B.String(), "3")
	require.Equal(suite.T(), chainBOwner, suite.testerB.String())

	// We query the data we have on the tester contract
	// This ensures that the callbacks are called after all the messages was completed
	// and the transfer was successful
	testerDataOwnerA = test_suite.QueryTesterSent(suite.T(), suite.chainA, suite.testerA.String())
	require.Equal(suite.T(), testerDataOwnerA, suite.bridgeA.String())
	testerNftContract = test_suite.QueryTesterNftContract(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerNftContract, suite.cw721B.String())
	testerDataOwnerB = test_suite.QueryTesterReceived(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerDataOwnerB, suite.testerB.String())
}
