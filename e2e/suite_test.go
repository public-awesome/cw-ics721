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

	src := path.EndpointA
	dest := path.EndpointB
	toSend := src.Chain.PendingSendPackets

	suite.T().Logf("Relay %d Packets A->B\n", len(toSend))

	// send this to the other side
	suite.coordinator.IncrementTime()
	suite.coordinator.CommitBlock(src.Chain)
	err = dest.UpdateClient()
	require.NoError(suite.T(), err)

	for _, packet := range toSend {
		// Data does appear to be correctly encoded
		// here. Removing the `from_binary` call in
		// `do_ibc_packet_receive` and replacing it with a
		// hardcoded packet does not change the error.
		suite.T().Logf("Packet data: %v", string(packet.Data))
		err = dest.RecvPacket(packet)
		require.NoError(suite.T(), err)
	}
	src.Chain.PendingSendPackets = nil

	// get all the acks to relay dest->src
	toAck := dest.Chain.PendingAckPackets
	// TODO: assert >= len(toSend)?
	suite.T().Logf("Ack %d Packets B->A\n", len(toAck))
	ackData := dest.Chain.PendingAckPackets[0].Ack
	suite.T().Logf("ack: %v", string(ackData))

	// err = suite.coordinator.RelayAndAckPendingPackets(path)
	// require.NoError(suite.T(), err)

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

	getOwnerQuery := GetOwnerQuery{
		GetOwner: GetOwnerQueryData{
			TokenID: "1",
			ClassID: fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, cw721.String()),
		},
	}

	err = suite.chainB.SmartQuery(suite.chainBBridge.String(), getOwnerQuery, &resp)
	require.NoError(suite.T(), err)
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
