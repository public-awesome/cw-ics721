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

	// mint NFT to sender
	_, err = suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: suite.cw721A.String(),
		Msg:      []byte(fmt.Sprintf(`{ "mint": { "token_id": "3", "owner": "%s" } }`, suite.chainA.SenderAccount.GetAddress().String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	suite.T().Logf("(chain A bridge, chain B bridge) = (%s, %s)", suite.bridgeA.String(), suite.bridgeB.String())
	suite.T().Logf("(chain A tester, chain B tester) = (%s, %s)", suite.testerA.String(), suite.testerB.String())
	suite.T().Logf("(chain A cw721, chain B cw721) = (%s, %s)", suite.cw721A.String(), suite.cw721B.String())
}

func callbackMemo(src_cb, src_receiver, dest_cb, dest_receiver string) string {
	src_cb = parseOptional(src_cb)
	src_receiver = parseOptional(src_receiver)
	dest_cb = parseOptional(dest_cb)
	dest_receiver = parseOptional(dest_receiver)
	memo := fmt.Sprintf(`{ "callbacks": { "src_callback_msg": %s, "src_msg_receiver": %s, "dest_callback_msg": %s, "dest_msg_receiver": %s } }`, src_cb, src_receiver, dest_cb, dest_receiver)
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

func sendIcsFromChainA(suite *CbTestSuite, nft, token_id, memo string, relay bool) {
	msg := fmt.Sprintf(`{ "send_nft": {"cw721": "%s", "ics721": "%s", "token_id": "%s", "recipient":"%s", "channel_id":"%s", "memo":"%s"}}`, nft, suite.bridgeA.String(), token_id, suite.testerB.String(), suite.path.EndpointA.ChannelID, memo)
	_, err := suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: suite.testerA.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	if relay {
		suite.coordinator.UpdateTime()
		suite.coordinator.RelayAndAckPendingPackets(suite.path)
		suite.coordinator.UpdateTime()
	}
}

func sendIcsFromChainB(suite *CbTestSuite, nft, token_id, memo string, relay bool) {
	msg := fmt.Sprintf(`{ "send_nft": {"cw721": "%s", "ics721": "%s", "token_id": "%s", "recipient":"%s", "channel_id":"%s", "memo":"%s"}}`, nft, suite.bridgeB.String(), token_id, suite.testerA.String(), suite.path.EndpointB.ChannelID, memo)
	_, err := suite.chainB.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainB.SenderAccount.GetAddress().String(),
		Contract: suite.testerB.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	if relay {
		suite.coordinator.UpdateTime()
		suite.coordinator.RelayAndAckPendingPackets(suite.path.Invert())
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

func queryTesterReceivedNftContract(t *testing.T, chain *wasmibctesting.TestChain, tester string) string {
	resp := ""
	testerReceivedQuery := TesterReceivedNftContractQuery{
		GetReceivedNftContract: EmptyData{},
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
	sendIcsFromChainA(suite, suite.cw721A.String(), "2", memo, true)

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
	testerNftContract := queryTesterReceivedNftContract(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerNftContract, suite.cw721B.String())
	testerDataOwnerB := queryTesterReceived(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerDataOwnerB, suite.testerB.String())
}

func (suite *CbTestSuite) TestSuccessfulTransferWithReceivers() {
	memo := callbackMemo(nftSentCb(), suite.testerA.String(), nftReceivedCb(), suite.testerB.String())

	// Send NFT to chain B
	ics721Nft(suite.T(), suite.chainA, suite.path, suite.coordinator, suite.cw721A.String(), "3", suite.bridgeA, suite.chainA.SenderAccount.GetAddress(), suite.chainB.SenderAccount.GetAddress(), memo)

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
	testerNftContract := queryTesterReceivedNftContract(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerNftContract, suite.cw721B.String())
	testerDataOwnerB := queryTesterReceived(suite.T(), suite.chainB, suite.testerB.String())
	require.Equal(suite.T(), testerDataOwnerB, suite.chainB.SenderAccount.GetAddress().String())
}

func (suite *CbTestSuite) TestTimeoutedTransfer() {
	memo := callbackMemo(nftSentCb(), "", nftReceivedCb(), "")
	sendIcsFromChainA(suite, suite.cw721A.String(), "2", memo, false)
	suite.coordinator.IncrementTimeBy(time.Second * 2001)
	suite.coordinator.UpdateTime()
	suite.coordinator.TimeoutPendingPackets(suite.path)

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
	sendIcsFromChainA(suite, suite.cw721A.String(), "2", memo, true)

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
	sendIcsFromChainA(suite, suite.cw721A.String(), "2", memo, true)

	// Transfer from B to chain A,
	// We fail the ack callback and see if the NFT was burned or not
	// Because the transfer should be successful even if the ack callback is failing
	// we make sure that the NFT was burned on chain B, and that the owner is correct on chain A
	memo = callbackMemo(failedCb(), "", "", "")
	sendIcsFromChainB(suite, suite.cw721B.String(), "2", memo, true)

	// Transfer was successful, so the owner on chain A should be the testerA
	chainAOwner := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String(), "2")
	require.Equal(suite.T(), chainAOwner, suite.testerA.String())

	// Transfer was successful, so nft "2" should be burned and fail the query
	err := queryGetOwnerOfErr(suite.T(), suite.chainB, suite.cw721B.String(), "2")
	require.Error(suite.T(), err)

	// We don't do any query on tester, because we don't have receive callback set
	// and the ack callback should fail, so no data to show.
}
