package e2e_test

import (
	"encoding/json"
	"testing"

	"fmt"

	b64 "encoding/base64"

	wasmibctesting "github.com/CosmWasm/wasmd/x/wasm/ibctesting"
	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	channeltypes "github.com/cosmos/ibc-go/v3/modules/core/04-channel/types"
	ibctesting "github.com/cosmos/ibc-go/v3/testing"
	"github.com/stretchr/testify/require"
	"github.com/stretchr/testify/suite"
)

type TransferTestSuite struct {
	suite.Suite

	coordinator *wasmibctesting.Coordinator

	// testing chains used for convenience and readability
	chainA *wasmibctesting.TestChain
	chainB *wasmibctesting.TestChain

	chainABridge sdk.AccAddress
	chainBBridge sdk.AccAddress
}

func (suite *TransferTestSuite) SetupTest() {
	suite.coordinator = wasmibctesting.NewCoordinator(suite.T(), 2)
	suite.chainA = suite.coordinator.GetChain(wasmibctesting.GetChainID(0))
	suite.chainB = suite.coordinator.GetChain(wasmibctesting.GetChainID(1))
	suite.coordinator.CommitBlock(suite.chainA, suite.chainB)

	// Store the bridge contract.
	chainAStoreResp := suite.chainA.StoreCodeFile("../artifacts/cw_ics721_bridge.wasm")
	require.Equal(suite.T(), uint64(1), chainAStoreResp.CodeID)
	chainBStoreResp := suite.chainB.StoreCodeFile("../artifacts/cw_ics721_bridge.wasm")
	require.Equal(suite.T(), uint64(1), chainBStoreResp.CodeID)

	// Store the escrow contract.
	chainAStoreResp = suite.chainA.StoreCodeFile("../artifacts/ics721_escrow.wasm")
	require.Equal(suite.T(), uint64(2), chainAStoreResp.CodeID)
	chainBStoreResp = suite.chainB.StoreCodeFile("../artifacts/ics721_escrow.wasm")
	require.Equal(suite.T(), uint64(2), chainBStoreResp.CodeID)

	// Store the cw721 contract.
	chainAStoreResp = suite.chainA.StoreCodeFile("../artifacts/cw721_base.wasm")
	require.Equal(suite.T(), uint64(3), chainAStoreResp.CodeID)
	chainBStoreResp = suite.chainB.StoreCodeFile("../artifacts/cw721_base.wasm")
	require.Equal(suite.T(), uint64(3), chainBStoreResp.CodeID)

	// Store the cw721_base contract.

	instantiateICS721 := InstantiateICS721Bridge{
		3,
		2,
	}
	instantiateICS721Raw, err := json.Marshal(instantiateICS721)
	require.NoError(suite.T(), err)
	suite.chainABridge = suite.chainA.InstantiateContract(1, instantiateICS721Raw)
	suite.chainBBridge = suite.chainB.InstantiateContract(1, instantiateICS721Raw)

	suite.T().Logf("(chain A bridge, chain B bridge) = (%s, %s)", suite.chainABridge.String(), suite.chainBBridge.String())
}

func (suite *TransferTestSuite) TestEstablishConnection() {
	var (
		sourcePortID      = suite.chainA.ContractInfo(suite.chainABridge).IBCPortID
		counterpartPortID = suite.chainB.ContractInfo(suite.chainBBridge).IBCPortID
	)
	suite.coordinator.CommitBlock(suite.chainA, suite.chainB)
	suite.coordinator.UpdateTime()

	require.Equal(suite.T(), suite.chainA.CurrentHeader.Time, suite.chainB.CurrentHeader.Time)
	path := wasmibctesting.NewPath(suite.chainA, suite.chainB)
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
}

func (suite *TransferTestSuite) TestIBCSendNFT() {
	var (
		sourcePortID      = suite.chainA.ContractInfo(suite.chainABridge).IBCPortID
		counterpartPortID = suite.chainB.ContractInfo(suite.chainBBridge).IBCPortID
	)
	suite.coordinator.CommitBlock(suite.chainA, suite.chainB)
	suite.coordinator.UpdateTime()

	require.Equal(suite.T(), suite.chainA.CurrentHeader.Time, suite.chainB.CurrentHeader.Time)
	path := wasmibctesting.NewPath(suite.chainA, suite.chainB)
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

	// Instantiate a cw721 to send on chain A.
	cw721Instantiate := InstantiateCw721{
		"bad/kids",
		"bad/kids",
		suite.chainA.SenderAccount.GetAddress().String(),
	}
	instantiateRaw, err := json.Marshal(cw721Instantiate)
	require.NoError(suite.T(), err)
	cw721 := suite.chainA.InstantiateContract(3, instantiateRaw)

	suite.T().Logf("chain A cw721: %s", cw721.String())

	// Mint a new NFT to be sent away.
	_, err = suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: cw721.String(),
		Msg:      []byte(fmt.Sprintf(`{ "mint": { "token_id": "1", "owner": "%s" } }`, suite.chainA.SenderAccount.GetAddress().String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	ibcAway := fmt.Sprintf(`{ "receiver": "%s", "channel_id": "%s", "timeout": { "timestamp": "%d" } }`, suite.chainB.SenderAccount.GetAddress().String(), path.EndpointA.ChannelID, suite.coordinator.CurrentTime.UnixNano()+1000000000000)
	ibcAwayEncoded := b64.StdEncoding.EncodeToString([]byte(ibcAway))

	// Send the NFT away to chain B.
	_, err = suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: cw721.String(),
		Msg:      []byte(fmt.Sprintf(`{ "send_nft": { "contract": "%s", "token_id": "1", "msg": "%s" } }`, suite.chainABridge.String(), ibcAwayEncoded)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	err = suite.coordinator.RelayAndAckPendingPackets(path)
	require.NoError(suite.T(), err)

	// Check that the NFT has been transfered away from the sender
	// on chain A.
	resp := OwnerOfResponse{}
	ownerOfQuery := OwnerOfQuery{
		OwnerOf: OwnerOfQueryData{
			TokenID: "1",
		},
	}
	err = suite.chainA.SmartQuery(cw721.String(), ownerOfQuery, &resp)
	require.NoError(suite.T(), err)
	require.NotEqual(suite.T(), suite.chainA.SenderAccount, resp.Owner)

	chainBClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, cw721.String())

	// Check that the receiver on the receiving chain now owns the NFT.
	getOwnerQuery := GetOwnerQuery{
		GetOwner: GetOwnerQueryData{
			TokenID: "1",
			ClassID: chainBClassID,
		},
	}
	err = suite.chainB.SmartQuery(suite.chainBBridge.String(), getOwnerQuery, &resp)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), suite.chainB.SenderAccount.GetAddress().String(), resp.Owner)

	// Get the address of the instantiated cw721.
	getClassQuery := GetClassQuery{
		GetClass: GetClassQueryData{
			ClassID: chainBClassID,
		},
	}
	chainBCw721 := ""

	err = suite.chainB.SmartQuery(suite.chainBBridge.String(), getClassQuery, &chainBCw721)
	require.NoError(suite.T(), err)
	suite.T().Logf("Chain B cw721: %s", chainBCw721)

	// Check that the contract info for the instantiated cw721 was
	// set correctly.
	contractInfo := ContractInfoResponse{}
	contractInfoQuery := ContractInfoQuery{
		ContractInfo: ContractInfoQueryData{},
	}
	err = suite.chainB.SmartQuery(chainBCw721, contractInfoQuery, &contractInfo)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), ContractInfoResponse{
		Name:   chainBClassID,
		Symbol: chainBClassID,
	}, contractInfo)

	//
	// Send the NFT back!
	//

	ibcAway = fmt.Sprintf(`{ "receiver": "%s", "channel_id": "%s", "timeout": { "timestamp": "%d" } }`, suite.chainA.SenderAccount.GetAddress().String(), path.EndpointB.ChannelID, suite.coordinator.CurrentTime.UnixNano()+1000000000000)
	ibcAwayEncoded = b64.StdEncoding.EncodeToString([]byte(ibcAway))

	// Send the NFT away to chain A.
	_, err = suite.chainB.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainB.SenderAccount.GetAddress().String(),
		Contract: chainBCw721,
		Msg:      []byte(fmt.Sprintf(`{ "send_nft": { "contract": "%s", "token_id": "1", "msg": "%s" } }`, suite.chainBBridge.String(), ibcAwayEncoded)),
		Funds:    []sdk.Coin{},
	})

	require.NoError(suite.T(), err)

	err = suite.coordinator.RelayAndAckPendingPackets(path.Invert())
	require.NoError(suite.T(), err)

	// Check that the NFT has been received on the other side.
	err = suite.chainA.SmartQuery(cw721.String(), ownerOfQuery, &resp)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), suite.chainA.SenderAccount.GetAddress().String(), resp.Owner)

	// Check that the GetClass query returns what we expect for
	// local NFTs.
	getClassQuery = GetClassQuery{
		GetClass: GetClassQueryData{
			ClassID: cw721.String(),
		},
	}
	getClassResp := ""

	err = suite.chainA.SmartQuery(suite.chainABridge.String(), getClassQuery, &getClassResp)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), cw721.String(), getClassResp)

	// FIXME: Why does uncommenting this query on chainA change
	// the error text in the burn check below?

	// getOwnerQuery = GetOwnerQuery{
	// 	GetOwner: GetOwnerQueryData{
	// 		TokenID: "1",
	// 		ClassID: cw721.String(),
	// 	},
	// }
	// err = suite.chainA.SmartQuery(suite.chainABridge.String(), getOwnerQuery, &resp)
	// require.NoError(suite.T(), err)
	// require.Equal(suite.T(), suite.chainA.SenderAccount.GetAddress().String(), resp.

	//
	// Check that the NFT was burned on the remote chain.
	//
	err = suite.chainB.SmartQuery(suite.chainBBridge.String(), getOwnerQuery, &resp)
	// This should fail as the NFT is burned and the load from
	// storage will cause it to error.
	require.ErrorContains(suite.T(), err, "wasm, code: 9: query wasm contract failed")
}

// FIXME: I am not sure we can actually catch the failure here as the
// underlying testing code will hit a require.NoError line. How can we
// write this test in such a way that failure to establish the channel
// will pass the test?

// func (suite *TransferTestSuite) TestEstablishConnectionFailsWhenOrdered() {

// 	var (
// 		sourcePortID      = suite.chainA.ContractInfo(suite.chainABridge).IBCPortID
// 		counterpartPortID = suite.chainB.ContractInfo(suite.chainBBridge).IBCPortID
// 	)
// 	suite.coordinator.CommitBlock(suite.chainA, suite.chainB)
// 	suite.coordinator.UpdateTime()

// 	require.Equal(suite.T(), suite.chainA.CurrentHeader.Time, suite.chainB.CurrentHeader.Time)
// 	path := wasmibctesting.NewPath(suite.chainA, suite.chainB)
// 	path.EndpointA.ChannelConfig = &ibctesting.ChannelConfig{
// 		PortID:  sourcePortID,
// 		Version: "ics721-1",
// 		Order:   channeltypes.ORDERED,
// 	}
// 	path.EndpointB.ChannelConfig = &ibctesting.ChannelConfig{
// 		PortID:  counterpartPortID,
// 		Version: "ics721-1",
// 		Order:   channeltypes.ORDERED,
// 	}

// 	suite.coordinator.SetupConnections(path)

// 	// Should fail as ordering is wrong.
// 	err := path.EndpointA.ChanOpenInit()
// 	require.True(suite.T(), err != nil)
// }

func TestIBC(t *testing.T) {
	suite.Run(t, new(TransferTestSuite))
}
