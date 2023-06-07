package e2e_test

import (
	b64 "encoding/base64"
	"encoding/json"
	"fmt"
	"testing"

	wasmibctesting "github.com/CosmWasm/wasmd/x/wasm/ibctesting"
	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	channeltypes "github.com/cosmos/ibc-go/v3/modules/core/04-channel/types"
	ibctesting "github.com/cosmos/ibc-go/v3/testing"
	"github.com/stretchr/testify/require"
	"github.com/stretchr/testify/suite"
)

type CbTestSuite struct {
	suite.Suite

	coordinator *wasmibctesting.Coordinator

	// testing chains used for convenience and readability
	chainA *wasmibctesting.TestChain
	chainB *wasmibctesting.TestChain

	bridgeA sdk.AccAddress
	bridgeB sdk.AccAddress

	path *wasmibctesting.Path

	testerA sdk.AccAddress
	testerB sdk.AccAddress

	cw721A sdk.AccAddress
	cw721B sdk.AccAddress
}

func TestCallbacks(t *testing.T) {
	suite.Run(t, new(CbTestSuite))
}

func (suite *CbTestSuite) SetupTest() {
	suite.coordinator = wasmibctesting.NewCoordinator(suite.T(), 2)
	suite.chainA = suite.coordinator.GetChain(wasmibctesting.GetChainID(0))
	suite.chainB = suite.coordinator.GetChain(wasmibctesting.GetChainID(1))
	// suite.coordinator.CommitBlock(suite.chainA, suite.chainB)

	// Store codes and instantiate contracts
	storeCodes := func(chain *wasmibctesting.TestChain, bridge *sdk.AccAddress, tester *sdk.AccAddress) {
		resp := chain.StoreCodeFile("../artifacts/cw_ics721_bridge.wasm")
		require.Equal(suite.T(), uint64(1), resp.CodeID)

		resp = chain.StoreCodeFile("../external-wasms/cw721_base_v0.15.0.wasm")
		require.Equal(suite.T(), uint64(2), resp.CodeID)

		resp = chain.StoreCodeFile("../artifacts/cw_ics721_bridge_tester.wasm")
		require.Equal(suite.T(), uint64(3), resp.CodeID)

		// init ics721
		instantiateBridge := InstantiateICS721Bridge{
			2,
			nil,
			nil,
		}
		instantiateBridgeRaw, err := json.Marshal(instantiateBridge)
		require.NoError(suite.T(), err)

		*bridge = chain.InstantiateContract(1, instantiateBridgeRaw)

		// init tester
		instantiateBridgeTester := InstantiateBridgeTester{
			"success",
			bridge.String(),
		}
		instantiateBridgeTesterRaw, err := json.Marshal(instantiateBridgeTester)
		require.NoError(suite.T(), err)

		*tester = chain.InstantiateContract(3, instantiateBridgeTesterRaw)
	}

	storeCodes(suite.chainA, &suite.bridgeA, &suite.testerA)
	storeCodes(suite.chainB, &suite.bridgeB, &suite.testerB)

	// init ibc path between chains
	sourcePortID := suite.chainA.ContractInfo(suite.bridgeA).IBCPortID
	counterpartPortID := suite.chainB.ContractInfo(suite.bridgeB).IBCPortID
	suite.path = wasmibctesting.NewPath(suite.chainA, suite.chainB)
	suite.path.EndpointA.ChannelConfig = &ibctesting.ChannelConfig{
		PortID:  sourcePortID,
		Version: "ics721-1",
		Order:   channeltypes.UNORDERED,
	}
	suite.path.EndpointB.ChannelConfig = &ibctesting.ChannelConfig{
		PortID:  counterpartPortID,
		Version: "ics721-1",
		Order:   channeltypes.UNORDERED,
	}
	suite.coordinator.SetupConnections(suite.path)
	suite.coordinator.CreateChannels(suite.path)

	// init cw721 on chain A
	cw721Instantiate := InstantiateCw721{
		"bad/kids",
		"bad/kids",
		suite.chainA.SenderAccount.GetAddress().String(),
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
	ics721Nft(suite.T(), suite.chainA, suite.path, suite.coordinator, suite.cw721A.String(), "1", suite.bridgeA, suite.chainA.SenderAccount.GetAddress(), suite.chainB.SenderAccount.GetAddress(), "")

	classIdChainB := fmt.Sprintf("%s/%s/%s", suite.path.EndpointB.ChannelConfig.PortID, suite.path.EndpointB.ChannelID, suite.cw721A.String())
	addr := queryGetNftForClass(suite.T(), suite.chainB, suite.bridgeB.String(), classIdChainB)
	suite.cw721B, err = sdk.AccAddressFromBech32(addr)
	require.NoError(suite.T(), err)

	// mint working NFT to tester
	_, err = suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: suite.cw721A.String(),
		Msg:      []byte(fmt.Sprintf(`{ "mint": { "token_id": "2", "owner": "%s" } }`, suite.testerA.String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	suite.T().Logf("(chain A bridge, chain B bridge) = (%s, %s)", suite.bridgeA.String(), suite.bridgeB.String())
	suite.T().Logf("(chain A tester, chain B tester) = (%s, %s)", suite.testerA.String(), suite.testerB.String())
	suite.T().Logf("(chain A cw721, chain B cw721) = (%s, %s)", suite.cw721A.String(), suite.cw721B.String())
}

func callbackMemo(src_cb, dest_cb string) string {
	src_cb = parseOptional(src_cb)
	dest_cb = parseOptional(dest_cb)
	memo := fmt.Sprintf(`{ "callbacks": { "src_callback_msg": %s, "dest_callback_msg": %s } }`, src_cb, dest_cb)
	return b64.StdEncoding.EncodeToString([]byte(memo))
}

func nftSentCb() string {
	return b64.StdEncoding.EncodeToString([]byte(`{ "nft_sent": {}}`))
}

func nftReceivedCb() string {
	return b64.StdEncoding.EncodeToString([]byte(`{ "nft_received": {}}`))
}

func failedCb() string {
	cb := fmt.Sprintf(`{ "fail_callback": {}}`)
	return b64.StdEncoding.EncodeToString([]byte(cb))
}

func sendIcsFromChainA(suite *CbTestSuite, nft string, token_id string, memo string) {
	msg := fmt.Sprintf(`{ "send_nft": {"cw721": "%s", "ics721": "%s", "token_id": "%s", "recipient":"%s", "channel_id":"%s", "memo":"%s"}}`, nft, suite.bridgeA.String(), token_id, suite.testerB.String(), suite.path.EndpointA.ChannelID, memo)
	_, err := suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: suite.testerA.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	relayPackets(suite, suite.path)
}

func sendIcsFromChainB(suite *CbTestSuite, nft string, token_id string, memo string) {
	msg := fmt.Sprintf(`{ "send_nft": {"cw721": "%s", "ics721": "%s", "token_id": "%s", "recipient":"%s", "channel_id":"%s", "memo":"%s"}}`, nft, suite.bridgeB.String(), token_id, suite.testerA.String(), suite.path.EndpointB.ChannelID, memo)
	_, err := suite.chainB.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainB.SenderAccount.GetAddress().String(),
		Contract: suite.testerB.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	relayPackets(suite, suite.path.Invert())
}

func relaySend(suite *CbTestSuite, path *wasmibctesting.Path) error {
	// get all the packet to relay src->dest
	src := path.EndpointA
	dest := path.EndpointB
	toSend := src.Chain.PendingSendPackets
	suite.T().Logf("Relay %d Packets A->B\n", len(toSend))

	// send this to the other side
	suite.coordinator.IncrementTime()
	suite.coordinator.CommitBlock(src.Chain)
	err := dest.UpdateClient()
	if err != nil {
		return err
	}
	for _, packet := range toSend {
		err = dest.RecvPacket(packet)
		if err != nil {
			return err
		}
	}
	src.Chain.PendingSendPackets = nil
	return nil
}

func relayAck(suite *CbTestSuite, path *wasmibctesting.Path) error {
	src := path.EndpointA
	dest := path.EndpointB
	toAck := dest.Chain.PendingAckPackets
	suite.T().Logf("Ack %d Packets B->A\n", len(toAck))

	// send the ack back from dest -> src
	suite.coordinator.IncrementTime()
	suite.coordinator.CommitBlock(dest.Chain)
	err := src.UpdateClient()
	if err != nil {
		return err
	}
	for _, ack := range toAck {
		err = src.AcknowledgePacket(ack.Packet, ack.Ack)
		suite.T().Logf("Ack %s", ack.Ack)
		if err != nil {
			return err
		}
	}
	dest.Chain.PendingAckPackets = nil
	return nil
}

func relayPackets(suite *CbTestSuite, path *wasmibctesting.Path) {
	err := relaySend(suite, path)
	require.NoError(suite.T(), err)
	err = relayAck(suite, path)
	require.NoError(suite.T(), err)
}

func queryTesterSent(t *testing.T, chain *wasmibctesting.TestChain, tester string) string {
	resp := TesterResponse{}
	testerSentQuery := TesterSentQuery{
		GetSentCallback: EmptyData{},
	}
	err := chain.SmartQuery(tester, testerSentQuery, &resp)
	require.NoError(t, err)
	return *resp.Owner
}

func queryTesterReceived(t *testing.T, chain *wasmibctesting.TestChain, tester string) string {
	resp := TesterResponse{}
	testerReceivedQuery := TesterReceivedQuery{
		GetReceivedCallback: EmptyData{},
	}
	err := chain.SmartQuery(tester, testerReceivedQuery, &resp)
	require.NoError(t, err)
	return *resp.Owner
}

/// This we need to test

/*
Things we need to test for the callbacks

ACK:
 1. successful transfer
 2. failed transfer
 3. Failed callback does nothing (owners are still the same)
    * both owner in the event is the owner that it should be
 4. Transfer back from chainB to chainA if the transfer is successful then the NFT should be burned on chainB
 5. Failed callback does nothing, the NFT should still be burned.

RECEIVE:
 1. successful transfer
 2. failed transfer does nothing
 3. failed callback revert the transfer
*/
func (suite *CbTestSuite) TestSuccessfulTransfer() {
	memo := callbackMemo(nftSentCb(), nftReceivedCb())
	sendIcsFromChainA(suite, suite.cw721A.String(), "2", memo)

	// suite.coordinator.IncrementTimeBy(time.Second * 1001)

	// Query the owner of NFT on cw721
	chainAOwner := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String(), "2")
	chainBOwner := queryGetOwnerOf(suite.T(), suite.chainB, suite.cw721B.String(), "2")
	require.Equal(suite.T(), chainAOwner, suite.bridgeA.String())
	require.Equal(suite.T(), chainBOwner, suite.testerB.String())

	// We query the data we have on the tester contract
	// This ensures that the callbacks are called after all the messages was completed
	// and the transfer was successful
	testerDataOwnerA := queryTesterSent(suite.T(), suite.chainA, suite.testerA.String())
	testerDataOwnerB := queryTesterReceived(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerDataOwnerA, suite.bridgeA.String())
	require.Equal(suite.T(), testerDataOwnerB, suite.testerB.String())
}
