package e2e_test

import (
	b64 "encoding/base64"
	"encoding/json"
	"fmt"
	"testing"
	"time"

	wasmibctesting "github.com/CosmWasm/wasmd/x/wasm/ibctesting"
	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	channeltypes "github.com/cosmos/ibc-go/v4/modules/core/04-channel/types"
	ibctesting "github.com/cosmos/ibc-go/v4/testing"
	"github.com/stretchr/testify/require"
	"github.com/stretchr/testify/suite"
)

type CbTestSuite struct {
	suite.Suite

	coordinator *wasmibctesting.Coordinator

	// testing chains used for convenience and readability
	chainA *wasmibctesting.TestChain
	chainB *wasmibctesting.TestChain
	chainC *wasmibctesting.TestChain

	bridgeA sdk.AccAddress
	bridgeB sdk.AccAddress
	bridgeC sdk.AccAddress

	pathAB *wasmibctesting.Path
	pathAC *wasmibctesting.Path
	pathBC *wasmibctesting.Path

	testerA sdk.AccAddress
	testerB sdk.AccAddress
	testerC sdk.AccAddress

	cw721A sdk.AccAddress
	cw721B sdk.AccAddress
}

func TestCallbacks(t *testing.T) {
	suite.Run(t, new(CbTestSuite))
}

func (suite *CbTestSuite) SetupTest() {
	suite.coordinator = wasmibctesting.NewCoordinator(suite.T(), 3)
	suite.chainA = suite.coordinator.GetChain(wasmibctesting.GetChainID(0))
	suite.chainB = suite.coordinator.GetChain(wasmibctesting.GetChainID(1))
	suite.chainC = suite.coordinator.GetChain(wasmibctesting.GetChainID(2))
	// suite.coordinator.CommitBlock(suite.chainA, suite.chainB)

	// Store codes and instantiate contracts
	storeCodes := func(chain *wasmibctesting.TestChain, bridge *sdk.AccAddress, tester *sdk.AccAddress, num int) {
		resp := chain.StoreCodeFile("../artifacts/ics721_base.wasm")
		require.Equal(suite.T(), uint64(1), resp.CodeID)

		resp = chain.StoreCodeFile("../external-wasms/cw721_base_v0.18.0.wasm")
		require.Equal(suite.T(), uint64(2), resp.CodeID)

		resp = chain.StoreCodeFile("../artifacts/ics721_base_tester.wasm")
		require.Equal(suite.T(), uint64(3), resp.CodeID)

		// init dummy contracts based on how much we need
		for i := 0; i < num; i++ {
			cw721Instantiate := InstantiateCw721{
				"bad/kids",
				"bad/kids",
				suite.chainA.SenderAccount.GetAddress().String(),
				nil,
			}
			instantiateRaw, err := json.Marshal(cw721Instantiate)
			require.NoError(suite.T(), err)

			chain.InstantiateContract(2, instantiateRaw)
		}

		// init ics721
		instantiateBridge := InstantiateICS721Bridge{
			2,
			nil,
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

	storeCodes(suite.chainA, &suite.bridgeA, &suite.testerA, 0)
	storeCodes(suite.chainB, &suite.bridgeB, &suite.testerB, 3)
	storeCodes(suite.chainC, &suite.bridgeC, &suite.testerC, 6)

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
	suite.pathAC = initPath(suite.chainA, suite.chainC, suite.bridgeA, suite.bridgeC)
	suite.pathBC = initPath(suite.chainB, suite.chainC, suite.bridgeB, suite.bridgeC)

	// init cw721 on chain A
	cw721Instantiate := InstantiateCw721{
		"bad/kids",
		"bad/kids",
		suite.chainA.SenderAccount.GetAddress().String(),
		nil,
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
	ics721Nft(suite.T(), suite.chainA, suite.pathAB, suite.coordinator, suite.cw721A.String(), "1", suite.bridgeA, suite.chainA.SenderAccount.GetAddress(), suite.chainB.SenderAccount.GetAddress(), "")

	classIdChainB := fmt.Sprintf("%s/%s/%s", suite.pathAB.EndpointB.ChannelConfig.PortID, suite.pathAB.EndpointB.ChannelID, suite.cw721A.String())
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

	// mint another NFT to tester
	_, err = suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: suite.cw721A.String(),
		Msg:      []byte(fmt.Sprintf(`{ "mint": { "token_id": "4", "owner": "%s" } }`, suite.testerA.String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	// mint NFT to sender
	_, err = suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: suite.cw721A.String(),
		Msg:      []byte(fmt.Sprintf(`{ "mint": { "token_id": "3", "owner": "%s" } }`, suite.chainA.SenderAccount.GetAddress().String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	suite.T().Logf("chain A bridge = (%s)", suite.bridgeA.String())
	suite.T().Logf("chain B bridge = (%s)", suite.bridgeB.String())
	suite.T().Logf("chain C bridge = (%s)", suite.bridgeC.String())
	suite.T().Logf("chain A tester = (%s)", suite.testerA.String())
	suite.T().Logf("chain B tester = (%s)", suite.testerB.String())
	suite.T().Logf("chain C tester = (%s)", suite.testerC.String())
	suite.T().Logf("chain A cw721) = (%s)", suite.cw721A.String())
	suite.T().Logf("chain B cw721) = (%s)", suite.cw721B.String())
}

func callbackMemo(src_cb, src_receiver, dest_cb, dest_receiver string) string {
	src_cb = parseOptional(src_cb)
	src_receiver = parseOptional(src_receiver)
	dest_cb = parseOptional(dest_cb)
	dest_receiver = parseOptional(dest_receiver)
	memo := fmt.Sprintf(`{ "callbacks": { "ack_callback_data": %s, "ack_callback_addr": %s, "receive_callback_data": %s, "receive_callback_addr": %s } }`, src_cb, src_receiver, dest_cb, dest_receiver)
	return b64.StdEncoding.EncodeToString([]byte(memo))
}

func nftSentCb() string {
	return b64.StdEncoding.EncodeToString([]byte(`{ "nft_sent": {}}`))
}

func nftReceivedCb() string {
	return b64.StdEncoding.EncodeToString([]byte(`{ "nft_received": {}}`))
}

func failedCb() string {
	return b64.StdEncoding.EncodeToString([]byte(`{ "fail_callback": {}}`))
}

func sendIcsFromChainAToB(suite *CbTestSuite, nft, token_id, memo string, relay bool) {
	msg := fmt.Sprintf(`{ "send_nft": {"cw721": "%s", "ics721": "%s", "token_id": "%s", "recipient":"%s", "channel_id":"%s", "memo":"%s"}}`, nft, suite.bridgeA.String(), token_id, suite.testerB.String(), suite.pathAB.EndpointA.ChannelID, memo)
	_, err := suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: suite.testerA.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	if relay {
		suite.coordinator.UpdateTime()
		suite.coordinator.RelayAndAckPendingPackets(suite.pathAB)
		suite.coordinator.UpdateTime()
	}
}

func sendIcsFromChainAToC(suite *CbTestSuite, nft, token_id, memo string, relay bool) {
	msg := fmt.Sprintf(`{ "send_nft": {"cw721": "%s", "ics721": "%s", "token_id": "%s", "recipient":"%s", "channel_id":"%s", "memo":"%s"}}`, nft, suite.bridgeA.String(), token_id, suite.testerC.String(), suite.pathAC.EndpointA.ChannelID, memo)
	_, err := suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: suite.testerA.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	if relay {
		suite.coordinator.UpdateTime()
		suite.coordinator.RelayAndAckPendingPackets(suite.pathAC)
		suite.coordinator.UpdateTime()
	}
}

func sendIcsFromChainBToA(suite *CbTestSuite, nft, token_id, memo string, relay bool) {
	msg := fmt.Sprintf(`{ "send_nft": {"cw721": "%s", "ics721": "%s", "token_id": "%s", "recipient":"%s", "channel_id":"%s", "memo":"%s"}}`, nft, suite.bridgeB.String(), token_id, suite.testerA.String(), suite.pathAB.EndpointB.ChannelID, memo)
	_, err := suite.chainB.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainB.SenderAccount.GetAddress().String(),
		Contract: suite.testerB.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	if relay {
		suite.coordinator.UpdateTime()
		suite.coordinator.RelayAndAckPendingPackets(suite.pathAB.Invert())
	}
}

func sendIcsFromChainBToC(suite *CbTestSuite, nft, token_id, memo string, relay bool) {
	msg := fmt.Sprintf(`{ "send_nft": {"cw721": "%s", "ics721": "%s", "token_id": "%s", "recipient":"%s", "channel_id":"%s", "memo":"%s"}}`, nft, suite.bridgeB.String(), token_id, suite.testerC.String(), suite.pathBC.EndpointA.ChannelID, memo)
	_, err := suite.chainB.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainB.SenderAccount.GetAddress().String(),
		Contract: suite.testerB.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	if relay {
		suite.coordinator.UpdateTime()
		suite.coordinator.RelayAndAckPendingPackets(suite.pathBC)
		suite.coordinator.UpdateTime()
	}
}

func sendIcsFromChainCToB(suite *CbTestSuite, nft, token_id, memo string, relay bool) {
	msg := fmt.Sprintf(`{ "send_nft": {"cw721": "%s", "ics721": "%s", "token_id": "%s", "recipient":"%s", "channel_id":"%s", "memo":"%s"}}`, nft, suite.bridgeC.String(), token_id, suite.testerB.String(), suite.pathBC.EndpointB.ChannelID, memo)
	_, err := suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.testerC.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	if relay {
		suite.coordinator.UpdateTime()
		suite.coordinator.RelayAndAckPendingPackets(suite.pathBC.Invert())
		suite.coordinator.UpdateTime()
	}
}

func sendIcsFromChainCToA(suite *CbTestSuite, nft, token_id, memo string, relay bool) {
	msg := fmt.Sprintf(`{ "send_nft": {"cw721": "%s", "ics721": "%s", "token_id": "%s", "recipient":"%s", "channel_id":"%s", "memo":"%s"}}`, nft, suite.bridgeC.String(), token_id, suite.testerA.String(), suite.pathAC.EndpointB.ChannelID, memo)
	_, err := suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.testerC.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	if relay {
		suite.coordinator.UpdateTime()
		suite.coordinator.RelayAndAckPendingPackets(suite.pathAC.Invert())
	}
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

func queryTesterNftContract(t *testing.T, chain *wasmibctesting.TestChain, tester string) string {
	resp := ""
	testerReceivedQuery := TesterNftContractQuery{
		GetNftContract: EmptyData{},
	}
	err := chain.SmartQuery(tester, testerReceivedQuery, &resp)
	require.NoError(t, err)
	return resp
}

func queryTesterReceivedErr(t *testing.T, chain *wasmibctesting.TestChain, tester string) error {
	resp := TesterResponse{}
	testerReceivedQuery := TesterReceivedQuery{
		GetReceivedCallback: EmptyData{},
	}
	err := chain.SmartQuery(tester, testerReceivedQuery, &resp)
	return err
}

func (suite *CbTestSuite) TestSuccessfulTransfer() {
	memo := callbackMemo(nftSentCb(), "", nftReceivedCb(), "")
	sendIcsFromChainAToB(suite, suite.cw721A.String(), "2", memo, true)

	// Query the owner of NFT on cw721
	chainAOwner := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String(), "2")
	require.Equal(suite.T(), chainAOwner, suite.bridgeA.String())
	chainBOwner := queryGetOwnerOf(suite.T(), suite.chainB, suite.cw721B.String(), "2")
	require.Equal(suite.T(), chainBOwner, suite.testerB.String())

	// We query the data we have on the tester contract
	// This ensures that the callbacks are called after all the messages was completed
	// and the transfer was successful
	testerDataOwnerA := queryTesterSent(suite.T(), suite.chainA, suite.testerA.String())
	require.Equal(suite.T(), testerDataOwnerA, suite.bridgeA.String())
	testerNftContract := queryTesterNftContract(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerNftContract, suite.cw721B.String())
	testerDataOwnerB := queryTesterReceived(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerDataOwnerB, suite.testerB.String())
}

func (suite *CbTestSuite) TestSuccessfulTransferWithReceivers() {
	memo := callbackMemo(nftSentCb(), suite.testerA.String(), nftReceivedCb(), suite.testerB.String())

	// Send NFT to chain B
	ics721Nft(suite.T(), suite.chainA, suite.pathAB, suite.coordinator, suite.cw721A.String(), "3", suite.bridgeA, suite.chainA.SenderAccount.GetAddress(), suite.chainB.SenderAccount.GetAddress(), memo)

	// Query the owner of NFT on cw721
	chainAOwner := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String(), "3")
	require.Equal(suite.T(), chainAOwner, suite.bridgeA.String())
	chainBOwner := queryGetOwnerOf(suite.T(), suite.chainB, suite.cw721B.String(), "3")
	require.Equal(suite.T(), chainBOwner, suite.chainB.SenderAccount.GetAddress().String())

	// We query the data we have on the tester contract
	// This ensures that the callbacks are called after all the messages was completed
	// and the transfer was successful
	testerDataOwnerA := queryTesterSent(suite.T(), suite.chainA, suite.testerA.String())
	require.Equal(suite.T(), testerDataOwnerA, suite.bridgeA.String())
	testerNftContract := queryTesterNftContract(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerNftContract, suite.cw721B.String())
	testerDataOwnerB := queryTesterReceived(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerDataOwnerB, suite.chainB.SenderAccount.GetAddress().String())
}

func (suite *CbTestSuite) TestTimeoutTransfer() {
	memo := callbackMemo(nftSentCb(), "", nftReceivedCb(), "")
	sendIcsFromChainAToB(suite, suite.cw721A.String(), "2", memo, false)
	suite.coordinator.IncrementTimeBy(time.Second * 2001)
	suite.coordinator.UpdateTime()
	suite.coordinator.TimeoutPendingPackets(suite.pathAB)

	// Query the owner of NFT on cw721
	chainAOwner := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String(), "2")
	require.Equal(suite.T(), chainAOwner, suite.testerA.String())
	err := queryGetOwnerOfErr(suite.T(), suite.chainB, suite.cw721B.String(), "2")
	require.Error(suite.T(), err)

	// callbacks should update sender contract of the failed transfer
	// so we query the contract to see who is the new owner
	// if the query is working and owner is correct, we can confirm the callback was called successfully
	testerDataOwnerA := queryTesterSent(suite.T(), suite.chainA, suite.testerA.String())
	require.Equal(suite.T(), testerDataOwnerA, suite.testerA.String())

	// Querying the receving end, should fail because we did not receive the NFT
	// so the callback should not have been called.
	err = queryTesterReceivedErr(suite.T(), suite.chainB, suite.testerB.String())
	require.Error(suite.T(), err)
}

func (suite *CbTestSuite) TestFailedCallbackTransfer() {
	memo := callbackMemo(nftSentCb(), "", failedCb(), "")
	sendIcsFromChainAToB(suite, suite.cw721A.String(), "2", memo, true)

	// Query the owner of NFT on cw721
	chainAOwner := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String(), "2")
	require.Equal(suite.T(), chainAOwner, suite.testerA.String())
	err := queryGetOwnerOfErr(suite.T(), suite.chainB, suite.cw721B.String(), "2")
	require.Error(suite.T(), err)

	// callbacks should update sender contract of the failed transfer
	// so we query the contract to see who is the new owner
	// if the query is working and owner is correct, we can confirm the callback was called successfully
	testerDataOwnerA := queryTesterSent(suite.T(), suite.chainA, suite.testerA.String())
	require.Equal(suite.T(), testerDataOwnerA, suite.testerA.String())

	// Querying the receving end, should fail because we did not receive the NFT
	// so the callback should not have been called.
	err = queryTesterReceivedErr(suite.T(), suite.chainB, suite.testerB.String())
	require.Error(suite.T(), err)
}

func (suite *CbTestSuite) TestFailedCallbackOnAck() {
	// Transfer to chain B
	memo := callbackMemo("", "", "", "")
	sendIcsFromChainAToB(suite, suite.cw721A.String(), "2", memo, true)

	// Transfer from B to chain A,
	// We fail the ack callback and see if the NFT was burned or not
	// Because the transfer should be successful even if the ack callback is failing
	// we make sure that the NFT was burned on chain B, and that the owner is correct on chain A
	memo = callbackMemo(failedCb(), "", "", "")
	sendIcsFromChainBToA(suite, suite.cw721B.String(), "2", memo, true)

	// Transfer was successful, so the owner on chain A should be the testerA
	chainAOwner := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String(), "2")
	require.Equal(suite.T(), chainAOwner, suite.testerA.String())

	// Transfer was successful, so nft "2" should be burned and fail the query
	err := queryGetOwnerOfErr(suite.T(), suite.chainB, suite.cw721B.String(), "2")
	require.Error(suite.T(), err)

	// We don't do any query on tester, because we don't have receive callback set
	// and the ack callback should fail, so no data to show.
}

func (suite *CbTestSuite) TestMultipleChainsTransfers() {
	confirmNftContracts := func(ackChain *wasmibctesting.TestChain, receiveChain *wasmibctesting.TestChain, testerAck string, testerReceive string, expectAck string, expectReceive string) {
		ackContract := queryTesterNftContract(suite.T(), ackChain, testerAck)
		require.Equal(suite.T(), ackContract, expectAck)

		receiveContract := queryTesterNftContract(suite.T(), receiveChain, testerReceive)
		require.Equal(suite.T(), receiveContract, expectReceive)
	}

	memo := callbackMemo(nftSentCb(), "", nftReceivedCb(), "")
	sendIcsFromChainAToB(suite, suite.cw721A.String(), "2", memo, true)

	// Owner should be the bridge on chain A
	chainAOwner := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String(), "2")
	require.Equal(suite.T(), chainAOwner, suite.bridgeA.String())

	chainBOwner := queryGetOwnerOf(suite.T(), suite.chainB, suite.cw721B.String(), "2")
	require.Equal(suite.T(), chainBOwner, suite.testerB.String())

	confirmNftContracts(suite.chainA, suite.chainB, suite.testerA.String(), suite.testerB.String(), suite.cw721A.String(), suite.cw721B.String())

	// Send from ChainB to ChainC
	sendIcsFromChainBToC(suite, suite.cw721B.String(), "2", memo, true)

	// Get the cw721 address on ChainC when received from ChainB
	BCClassId := fmt.Sprintf("%s/%s/%s/%s/%s", suite.pathBC.EndpointB.ChannelConfig.PortID, suite.pathBC.EndpointB.ChannelID, suite.pathAB.EndpointB.ChannelConfig.PortID, suite.pathAB.EndpointB.ChannelID, suite.cw721A)
	BCCw721 := queryGetNftForClass(suite.T(), suite.chainC, suite.bridgeC.String(), BCClassId)

	chainBOwner = queryGetOwnerOf(suite.T(), suite.chainB, suite.cw721B.String(), "2")
	require.Equal(suite.T(), chainBOwner, suite.bridgeB.String())

	// Make sure the transfer was correct and successful
	chainCOwner := queryGetOwnerOf(suite.T(), suite.chainC, BCCw721, "2")
	require.Equal(suite.T(), chainCOwner, suite.testerC.String())

	confirmNftContracts(suite.chainB, suite.chainC, suite.testerB.String(), suite.testerC.String(), suite.cw721B.String(), BCCw721)

	// Send from ChainA to ChainC
	sendIcsFromChainAToC(suite, suite.cw721A.String(), "4", memo, true)

	// Get the cw721 address on ChainC when received from ChainB
	ACClassId := fmt.Sprintf("%s/%s/%s", suite.pathAC.EndpointB.ChannelConfig.PortID, suite.pathAC.EndpointB.ChannelID, suite.cw721A)
	ACCw721 := queryGetNftForClass(suite.T(), suite.chainC, suite.bridgeC.String(), ACClassId)

	// Confirm tester is the owner on Chain C of the nft id "4"
	chainCOwner = queryGetOwnerOf(suite.T(), suite.chainC, ACCw721, "4")
	require.Equal(suite.T(), chainCOwner, suite.testerC.String())

	confirmNftContracts(suite.chainA, suite.chainC, suite.testerA.String(), suite.testerC.String(), suite.cw721A.String(), ACCw721)

	// Let send back all NFTs to Chain A
	sendIcsFromChainCToA(suite, ACCw721, "4", memo, true)
	confirmNftContracts(suite.chainC, suite.chainA, suite.testerC.String(), suite.testerA.String(), ACCw721, suite.cw721A.String())

	sendIcsFromChainCToB(suite, BCCw721, "2", memo, true)
	confirmNftContracts(suite.chainC, suite.chainB, suite.testerC.String(), suite.testerB.String(), BCCw721, suite.cw721B.String())

	sendIcsFromChainBToA(suite, suite.cw721B.String(), "2", memo, true)
	confirmNftContracts(suite.chainB, suite.chainA, suite.testerB.String(), suite.testerA.String(), suite.cw721B.String(), suite.cw721A.String())

	chainAOwner1 := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String(), "2")
	require.Equal(suite.T(), chainAOwner1, suite.testerA.String())
	chainAOwner2 := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String(), "4")
	require.Equal(suite.T(), chainAOwner2, suite.testerA.String())

	// NFTs should no exist on Chain B and Chain C, they should be burned and query for owner should error
	err := queryGetOwnerOfErr(suite.T(), suite.chainB, suite.cw721B.String(), "2")
	require.Error(suite.T(), err)
	err = queryGetOwnerOfErr(suite.T(), suite.chainC, BCCw721, "2")
	require.Error(suite.T(), err)
	err = queryGetOwnerOfErr(suite.T(), suite.chainC, ACCw721, "4")
	require.Error(suite.T(), err)
}
