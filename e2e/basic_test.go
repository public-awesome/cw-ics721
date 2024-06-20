package e2e

import (
	"encoding/json"
	"testing"

	wasmibctesting "github.com/CosmWasm/wasmd/x/wasm/ibctesting"
	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/public-awesome/ics721/e2e/test_suite"
	"github.com/stretchr/testify/require"
	"github.com/stretchr/testify/suite"
)

type BasicTestSuite struct {
	suite.Suite

	coordinator *wasmibctesting.Coordinator

	// testing chains used for convenience and readability
	chainA *wasmibctesting.TestChain

	chainABridge sdk.AccAddress
}

func TestBasic(t *testing.T) {
	suite.Run(t, new(BasicTestSuite))
}

func (suite *BasicTestSuite) SetupTest() {
	suite.coordinator = wasmibctesting.NewCoordinator(suite.T(), 2)
	suite.chainA = suite.coordinator.GetChain(wasmibctesting.GetChainID(0))
}

func (suite *BasicTestSuite) TestStoreCodes() {
	// Store the ICS721 contract.
	chainAStoreResp := suite.chainA.StoreCodeFile("../artifacts/ics721_base.wasm")
	require.Equal(suite.T(), uint64(1), chainAStoreResp.CodeID)

	// Store the cw721 contract.
	chainAStoreResp = suite.chainA.StoreCodeFile("../external-wasms/cw721_base_v0.18.0.wasm")
	require.Equal(suite.T(), uint64(2), chainAStoreResp.CodeID)
}

func (suite *BasicTestSuite) TestInstantiateIcs721() {
	// Store the ICS721 contract.
	chainAStoreResp := suite.chainA.StoreCodeFile("../artifacts/ics721_base.wasm")
	require.Equal(suite.T(), uint64(1), chainAStoreResp.CodeID)

	// Instantiate the ICS721 contract.
	instantiateICS721 := test_suite.InstantiateICS721Bridge{
		Cw721BaseCodeId: 1,
		// no pauser nor proxy by default.
		OutgoingProxy: nil,
		IncomingProxy: nil,
		Pauser:        nil,
	}
	instantiateICS721Raw, err := json.Marshal(instantiateICS721)
	require.NoError(suite.T(), err)
	suite.chainABridge = suite.chainA.InstantiateContract(1, instantiateICS721Raw)
}
