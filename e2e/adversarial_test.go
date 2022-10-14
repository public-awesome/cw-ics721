package e2e_test

import (
	"encoding/json"
	"fmt"
	"testing"
	"time"

	wasmibctesting "github.com/CosmWasm/wasmd/x/wasm/ibctesting"
	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	channeltypes "github.com/cosmos/ibc-go/v3/modules/core/04-channel/types"
	ibctesting "github.com/cosmos/ibc-go/v3/testing"
	"github.com/stretchr/testify/require"
	"github.com/stretchr/testify/suite"
)

// Assembles three chains in a little formation for the ics721
// olympics.
//
//	      +----------------+
//	      |                |
//	      | bridge-tester  |
//	      | chain: C       |
//	      |                |
//	      +----------------+
//		         ^
//		         |
//		         v
//		+----------------+             +-----------------+
//		|                |             |                 |
//		| bridge         |             | bridge          |
//		| chain: A       |<----------->| chain: B        |
//		| nftA           |             |                 |
//		+----------------+             +-----------------+
type AdversarialTestSuite struct {
	suite.Suite

	coordinator *wasmibctesting.Coordinator

	chainA *wasmibctesting.TestChain
	chainB *wasmibctesting.TestChain
	chainC *wasmibctesting.TestChain

	pathAB *wasmibctesting.Path
	pathAC *wasmibctesting.Path

	bridgeA sdk.AccAddress
	bridgeB sdk.AccAddress
	bridgeC sdk.AccAddress

	cw721A   sdk.AccAddress
	tokenIdA string
}

func TestIcs721Olympics(t *testing.T) {
	suite.Run(t, new(AdversarialTestSuite))
}

func (suite *AdversarialTestSuite) SetupTest() {
	suite.coordinator = wasmibctesting.NewCoordinator(suite.T(), 3)
	suite.chainA = suite.coordinator.GetChain(wasmibctesting.GetChainID(0))
	suite.chainB = suite.coordinator.GetChain(wasmibctesting.GetChainID(1))
	suite.chainC = suite.coordinator.GetChain(wasmibctesting.GetChainID(2))

	storeCodes := func(chain *wasmibctesting.TestChain, bridge *sdk.AccAddress) {
		resp := chain.StoreCodeFile("../artifacts/cw_ics721_bridge.wasm")
		require.Equal(suite.T(), uint64(1), resp.CodeID)

		resp = chain.StoreCodeFile("../external-wasms/cw721_base_v0.15.0.wasm")
		require.Equal(suite.T(), uint64(2), resp.CodeID)

		resp = chain.StoreCodeFile("../artifacts/cw_ics721_bridge_tester.wasm")
		require.Equal(suite.T(), uint64(3), resp.CodeID)

		instantiateBridge := InstantiateICS721Bridge{
			2,
			nil,
			nil,
		}
		instantiateBridgeRaw, err := json.Marshal(instantiateBridge)
		require.NoError(suite.T(), err)

		*bridge = chain.InstantiateContract(1, instantiateBridgeRaw)
	}

	storeCodes(suite.chainA, &suite.bridgeA)
	storeCodes(suite.chainB, &suite.bridgeB)
	storeCodes(suite.chainC, &suite.bridgeC)

	instantiateBridgeTester := InstantiateBridgeTester{
		"success",
	}
	instantiateBridgeTesterRaw, err := json.Marshal(instantiateBridgeTester)
	require.NoError(suite.T(), err)
	suite.bridgeC = suite.chainC.InstantiateContract(3, instantiateBridgeTesterRaw)

	suite.cw721A = instantiateCw721(suite.T(), suite.chainA)
	suite.tokenIdA = "bad kid 1"
	mintNFT(suite.T(), suite.chainA, suite.cw721A.String(), suite.tokenIdA, suite.chainA.SenderAccount.GetAddress())

	makePath := func(chainA, chainB *wasmibctesting.TestChain, bridgeA, bridgeB sdk.AccAddress) (path *wasmibctesting.Path) {
		sourcePortID := chainA.ContractInfo(bridgeA).IBCPortID
		counterpartPortID := chainB.ContractInfo(bridgeB).IBCPortID
		path = wasmibctesting.NewPath(chainA, chainB)
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
		return
	}

	suite.pathAB = makePath(suite.chainA, suite.chainB, suite.bridgeA, suite.bridgeB)
	suite.pathAC = makePath(suite.chainA, suite.chainC, suite.bridgeA, suite.bridgeC)
}

// How does the ics721-bridge contract respond if the other side
// closes the connection?
//
// It should:
//
//   - Return any NFTs that are pending transfer.
//   - Reject any future NFT transfers over the channel.
//   - Allow the channel to be closed on its side.
func (suite *AdversarialTestSuite) TestUnexpectedClose() {
	// Make a pending IBC message across the AC path, but do not
	// relay it.
	msg := getCw721SendIbcAwayMessage(suite.pathAC, suite.coordinator, suite.tokenIdA, suite.bridgeA, suite.chainC.SenderAccount.GetAddress(), suite.coordinator.CurrentTime.Add(time.Second*4).UnixNano())
	_, err := suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: suite.cw721A.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	// Close the channel from chain C.
	_, err = suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.bridgeC.String(),
		Msg:      []byte(fmt.Sprintf(`{"close_channel": { "channel_id": "%s" }}`, suite.pathAC.Invert().EndpointA.ChannelID)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	// Relay packets. This should cause the sent-but-not-relayed
	// packet above to get timed out and returned.
	suite.coordinator.TimeoutPendingPackets(suite.pathAC)
	suite.pathAC.EndpointA.ChanCloseConfirm()

	owner := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String())
	require.Equal(suite.T(), suite.chainA.SenderAccount.GetAddress().String(), owner)

	require.Equal(suite.T(), channeltypes.CLOSED, suite.pathAC.Invert().EndpointA.GetChannel().State)
	require.Equal(suite.T(), channeltypes.CLOSED, suite.pathAC.EndpointA.GetChannel().State)

	// Attempt to send again. Expect this to fail as the channel
	// is now closed.
	//
	// As there is no falliable version of SendMsgs, we've got to
	// use our in house edition.
	newAcc := CreateAndFundAccount(suite.T(), suite.chainA, 10)
	mintNFT(suite.T(), suite.chainA, suite.cw721A.String(), "bad kid 2", newAcc.Address)

	msg = getCw721SendIbcAwayMessage(suite.pathAC, suite.coordinator, "bad kid 2", suite.bridgeA, suite.chainC.SenderAccount.GetAddress(), suite.coordinator.CurrentTime.Add(time.Second*4).UnixNano())
	_, err = SendMsgsFromAccount(suite.T(), suite.chainA, newAcc, false, &wasmtypes.MsgExecuteContract{
		Sender:   newAcc.Address.String(),
		Contract: suite.cw721A.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.Error(suite.T(), err)
}

// How does the ics721-bridge contract respond if the other side sends
// a class ID corresponding to a class ID that is valid on a different
// channel but not on its channel?
//
// It should:
//   - Respond with ACK success.
//   - Not move the NFT on the different chain.
//   - Mint a new NFT corresponding to the sending chain.
//   - Allow returning the minted NFT to its source chain.
//
// This test also tests the setting, queryability, and behavior of
// metadata when a new packet comes in with conflicting information.
func (suite *AdversarialTestSuite) TestInvalidOnMineValidOnTheirs() {
	// Send a NFT to chain B from A.
	ics721Nft(suite.T(), suite.chainA, suite.pathAB, suite.coordinator, suite.cw721A.String(), suite.bridgeA, suite.chainA.SenderAccount.GetAddress(), suite.chainB.SenderAccount.GetAddress())

	chainBClassId := fmt.Sprintf("%s/%s/%s", suite.pathAB.EndpointB.ChannelConfig.PortID, suite.pathAB.EndpointB.ChannelID, suite.cw721A.String())

	// Check that the NFT has been received on chain B.
	chainBCw721 := queryGetNftForClass(suite.T(), suite.chainB, suite.bridgeB.String(), chainBClassId)
	chainBOwner := queryGetOwnerOf(suite.T(), suite.chainB, chainBCw721)
	require.Equal(suite.T(), suite.chainB.SenderAccount.GetAddress().String(), chainBOwner)

	// From chain C send a message using the chain B class ID to
	// unlock the NFT and send it to chain A's sender account.
	_, err := suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.bridgeC.String(),
		Msg:      []byte(fmt.Sprintf(`{ "send_packet": { "channel_id": "%s", "timeout": { "timestamp": "%d" }, "data": {"classId":"%s","classUri":"https://metadata-url.com/my-metadata","tokenIds":["%s"],"tokenUris":["https://metadata-url.com/my-metadata1"],"sender":"%s","receiver":"%s"} }}`, suite.pathAC.Invert().EndpointA.ChannelID, suite.coordinator.CurrentTime.Add(time.Hour*100).UnixNano(), chainBClassId, suite.tokenIdA, suite.chainC.SenderAccount.GetAddress().String(), suite.chainA.SenderAccount.GetAddress().String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.UpdateTime()
	suite.coordinator.RelayAndAckPendingPackets(suite.pathAC.Invert())

	// NFT should still be owned by the bridge on chain A.
	chainAOwner := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String())
	require.Equal(suite.T(), suite.bridgeA.String(), chainAOwner)

	// A new NFT should have been minted on chain A.
	chainAClassId := fmt.Sprintf("%s/%s/%s", suite.pathAC.EndpointA.ChannelConfig.PortID, suite.pathAC.EndpointA.ChannelID, chainBClassId)
	chainACw721 := queryGetNftForClass(suite.T(), suite.chainA, suite.bridgeA.String(), chainAClassId)
	chainAOwner = queryGetOwnerOf(suite.T(), suite.chainA, chainACw721)
	require.Equal(suite.T(), suite.chainA.SenderAccount.GetAddress().String(), chainAOwner)

	// Metadata should be set.
	var metadata string
	err = suite.chainA.SmartQuery(suite.bridgeA.String(), MetadataQuery{
		Metadata: MetadataQueryData{
			ClassId: chainAClassId,
		},
	}, &metadata)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), "https://metadata-url.com/my-metadata", metadata)

	// The newly minted NFT should be returnable to the source
	// chain and cause a burn when returned.
	ics721Nft(suite.T(), suite.chainA, suite.pathAC, suite.coordinator, chainACw721, suite.bridgeA, suite.chainA.SenderAccount.GetAddress(), suite.chainC.SenderAccount.GetAddress())

	err = suite.chainA.SmartQuery(chainACw721, OwnerOfQuery{OwnerOf: OwnerOfQueryData{TokenID: suite.tokenIdA}}, &OwnerOfResponse{})
	require.ErrorContains(suite.T(), err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>> not found")

	// Send the NFT back, this time setting new metadata for the
	// class ID.
	_, err = suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.bridgeC.String(),
		Msg:      []byte(fmt.Sprintf(`{ "send_packet": { "channel_id": "%s", "timeout": { "timestamp": "%d" }, "data": {"classId":"%s","classUri":"https://moonphase.is","tokenIds":["%s"],"tokenUris":["https://metadata-url.com/my-metadata1"],"sender":"%s","receiver":"%s"} }}`, suite.pathAC.Invert().EndpointA.ChannelID, suite.coordinator.CurrentTime.Add(time.Hour*100).UnixNano(), chainBClassId, suite.tokenIdA, suite.chainC.SenderAccount.GetAddress().String(), suite.chainA.SenderAccount.GetAddress().String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.UpdateTime()
	suite.coordinator.RelayAndAckPendingPackets(suite.pathAC.Invert())

	// Metadata should be set to the most up to date value.
	err = suite.chainA.SmartQuery(suite.bridgeA.String(), MetadataQuery{
		Metadata: MetadataQueryData{
			ClassId: chainAClassId,
		},
	}, &metadata)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), "https://moonphase.is", metadata)
}

// How does the ics721-bridge contract respond if the other side sends
// IBC messages where the class ID is empty?
//
// It should:
//   - Accept the message and mint a new NFT on the receiving chain.
//   - Metadata and NFT contract queries should still work.
//   - The NFT should be returnable.
//
// However, for reasons entirely beyond me, the SDK does it's own
// validation on our data field and errors if the class ID is empty,
// so this test capitulates and just tests that we handle the SDK
// error correctly.
func (suite *AdversarialTestSuite) TestEmptyClassId() {
	_, err := suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.bridgeC.String(),
		Msg:      []byte(fmt.Sprintf(`{ "send_packet": { "channel_id": "%s", "timeout": { "timestamp": "%d" }, "data": {"classId":"","classUri":"https://metadata-url.com/my-metadata","tokenIds":["%s"],"tokenUris":["https://metadata-url.com/my-metadata1"],"sender":"%s","receiver":"%s"} }}`, suite.pathAC.Invert().EndpointA.ChannelID, suite.coordinator.CurrentTime.Add(time.Hour*100).UnixNano(), suite.tokenIdA, suite.chainC.SenderAccount.GetAddress().String(), suite.chainA.SenderAccount.GetAddress().String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.UpdateTime()
	suite.coordinator.RelayAndAckPendingPackets(suite.pathAC.Invert())

	// Make sure we got the weird SDK error.
	var lastAck string
	err = suite.chainC.SmartQuery(suite.bridgeC.String(), LastAckQuery{LastAck: LastAckQueryData{}}, &lastAck)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), "error", lastAck)

	// Make sure a NFT was not minted in spite of the weird SDK
	// error.
	chainAClassId := fmt.Sprintf("%s/%s/%s", suite.pathAC.EndpointA.ChannelConfig.PortID, suite.pathAC.EndpointA.ChannelID, "")
	chainACw721 := queryGetNftForClass(suite.T(), suite.chainA, suite.bridgeA.String(), chainAClassId)
	require.Equal(suite.T(), "", chainACw721)
}

// Are ACK fails returned by this contract parseable?
//
// Sends a message with an invalid receiver and then checks that the
// testing contract can process the ack. The testing contract uses the
// same ACK processing logic as the bridge contract so this tests that
// by proxy.
func (suite *AdversarialTestSuite) TestSimpleAckFail() {
	// Send a NFT with an invalid receiver address.
	_, err := suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.bridgeC.String(),
		Msg:      []byte(fmt.Sprintf(`{ "send_packet": { "channel_id": "%s", "timeout": { "timestamp": "%d" }, "data": {"classId":"class","classUri":"https://metadata-url.com/my-metadata","tokenIds":["%s"],"tokenUris":["https://metadata-url.com/my-metadata1"],"sender":"%s","receiver":"%s"} }}`, suite.pathAC.Invert().EndpointA.ChannelID, suite.coordinator.CurrentTime.Add(time.Hour*100).UnixNano(), suite.tokenIdA, suite.chainC.SenderAccount.GetAddress().String(), "i am invalid")),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.UpdateTime()
	suite.coordinator.RelayAndAckPendingPackets(suite.pathAC.Invert())

	// Make sure we responded with an ACK success.
	var lastAck string
	err = suite.chainC.SmartQuery(suite.bridgeC.String(), LastAckQuery{LastAck: LastAckQueryData{}}, &lastAck)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), "error", lastAck)
}

// Are ACK successes returned by this contract parseable?
//
// Sends a valid message and then checks that the testing contract can
// process the ack. The testing contract uses the same ACK processing
// logic as the bridge contract so this tests that by proxy.
func (suite *AdversarialTestSuite) TestSimpleAckSuccess() {
	// Send a valid NFT message.
	_, err := suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.bridgeC.String(),
		Msg:      []byte(fmt.Sprintf(`{ "send_packet": { "channel_id": "%s", "timeout": { "timestamp": "%d" }, "data": {"classId":"%s","classUri":"https://metadata-url.com/my-metadata","tokenIds":["%s"],"tokenUris":["https://metadata-url.com/my-metadata1"],"sender":"%s","receiver":"%s"} }}`, suite.pathAC.Invert().EndpointA.ChannelID, suite.coordinator.CurrentTime.Add(time.Hour*100).UnixNano(), "classID", suite.tokenIdA, suite.chainC.SenderAccount.GetAddress().String(), suite.chainA.SenderAccount.GetAddress().String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.UpdateTime()
	suite.coordinator.RelayAndAckPendingPackets(suite.pathAC.Invert())

	// Make sure we responded with an ACK success.
	var lastAck string
	err = suite.chainC.SmartQuery(suite.bridgeC.String(), LastAckQuery{LastAck: LastAckQueryData{}}, &lastAck)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), "success", lastAck)
}

// How does the ics721-bridge contract respond if the other side sends
// IBC messages where the token URIs and IDs have different lengths?
//
// It should:
//   - Do nothing and respond with ack_fail.
func (suite *AdversarialTestSuite) TestDifferentUriAndIdLengths() {
	// Send a valid NFT message.
	_, err := suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.bridgeC.String(),
		Msg:      []byte(fmt.Sprintf(`{ "send_packet": { "channel_id": "%s", "timeout": { "timestamp": "%d" }, "data": {"classId":"%s","classUri":"https://metadata-url.com/my-metadata","tokenIds":["%s"],"tokenUris":[],"sender":"%s","receiver":"%s"} }}`, suite.pathAC.Invert().EndpointA.ChannelID, suite.coordinator.CurrentTime.Add(time.Hour*100).UnixNano(), "classID", suite.tokenIdA, suite.chainC.SenderAccount.GetAddress().String(), suite.chainA.SenderAccount.GetAddress().String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.UpdateTime()
	suite.coordinator.RelayAndAckPendingPackets(suite.pathAC.Invert())

	// Make sure we responded with an ACK fail.
	var lastAck string
	err = suite.chainC.SmartQuery(suite.bridgeC.String(), LastAckQuery{LastAck: LastAckQueryData{}}, &lastAck)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), "error", lastAck)
}

// How does the ics721-bridge contract respond if two identical
// transfer messages are sent to the source chain?
//
// It should:
//   - Mint a new NFT from the first message.
//   - ACK fail the second message.
func (suite *AdversarialTestSuite) TestSendReplayAttack() {
	msg := fmt.Sprintf(`{ "send_packet": { "channel_id": "%s", "timeout": { "timestamp": "%d" }, "data": {"classId":"%s","classUri":"https://metadata-url.com/my-metadata","tokenIds":["%s"],"tokenUris":["https://metadata-url.com/my-metadata1"],"sender":"%s","receiver":"%s"} }}`, suite.pathAC.Invert().EndpointA.ChannelID, suite.coordinator.CurrentTime.Add(time.Hour*100).UnixNano(), "classID", suite.tokenIdA, suite.chainC.SenderAccount.GetAddress().String(), suite.chainA.SenderAccount.GetAddress().String())
	_, err := suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.bridgeC.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	suite.coordinator.UpdateTime()
	suite.coordinator.RelayAndAckPendingPackets(suite.pathAC.Invert())

	// First one should work.
	var lastAck string
	err = suite.chainC.SmartQuery(suite.bridgeC.String(), LastAckQuery{LastAck: LastAckQueryData{}}, &lastAck)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), "success", lastAck)

	_, err = suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.bridgeC.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	suite.coordinator.UpdateTime()
	suite.coordinator.RelayAndAckPendingPackets(suite.pathAC.Invert())

	// Second one should fail as the NFT has already been sent.
	err = suite.chainC.SmartQuery(suite.bridgeC.String(), LastAckQuery{LastAck: LastAckQueryData{}}, &lastAck)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), "error", lastAck)

	// Make sure the receiver is the owner of the token.
	chainAClassId := fmt.Sprintf("%s/%s/%s", suite.pathAC.EndpointA.ChannelConfig.PortID, suite.pathAC.EndpointA.ChannelID, "classID")
	chainACw721 := queryGetNftForClass(suite.T(), suite.chainA, suite.bridgeA.String(), chainAClassId)
	chainAOwner := queryGetOwnerOf(suite.T(), suite.chainA, chainACw721)
	require.Equal(suite.T(), suite.chainA.SenderAccount.GetAddress().String(), chainAOwner)
}

// How does the ics721-bridge contract respond if the same token is
// sent twice in one transfer message?
//
// It should:
//   - Ack fail the entire transaction and not mint any new NFTs.
func (suite *AdversarialTestSuite) TestDoubleSendInSingleMessage() {
	// Two of the same token IDs in one message.
	_, err := suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.bridgeC.String(),
		Msg:      []byte(fmt.Sprintf(`{ "send_packet": { "channel_id": "%s", "timeout": { "timestamp": "%d" }, "data": {"classId":"%s","classUri":"https://metadata-url.com/my-metadata","tokenIds":["ekez", "ekez"],"tokenUris":["https://metadata-url.com/my-metadata1", "https://moonphase.is/image.svg"],"sender":"%s","receiver":"%s"} }}`, suite.pathAC.Invert().EndpointA.ChannelID, suite.coordinator.CurrentTime.Add(time.Hour*100).UnixNano(), "classID", suite.chainC.SenderAccount.GetAddress().String(), suite.chainA.SenderAccount.GetAddress().String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.UpdateTime()
	suite.coordinator.RelayAndAckPendingPackets(suite.pathAC.Invert())

	// Should fail.
	var lastAck string
	err = suite.chainC.SmartQuery(suite.bridgeC.String(), LastAckQuery{LastAck: LastAckQueryData{}}, &lastAck)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), "error", lastAck)

	// No NFT should have been created.
	chainAClassId := fmt.Sprintf("%s/%s/%s", suite.pathAC.EndpointA.ChannelConfig.PortID, suite.pathAC.EndpointA.ChannelID, "classID")
	chainACw721 := queryGetNftForClass(suite.T(), suite.chainA, suite.bridgeA.String(), chainAClassId)
	require.Equal(suite.T(), "", chainACw721)
}

func (suite *AdversarialTestSuite) TestReceiveMultipleNtsDifferentActions() {
	// Send a NFT from chain A to the evil chain.
	ics721Nft(suite.T(), suite.chainA, suite.pathAC, suite.coordinator, suite.cw721A.String(), suite.bridgeA, suite.chainA.SenderAccount.GetAddress(), suite.chainB.SenderAccount.GetAddress())

	pathCA := suite.pathAC.Invert()
	chainCClassId := fmt.Sprintf("%s/%s/%s", pathCA.EndpointA.ChannelConfig.PortID, pathCA.EndpointA.ChannelID, suite.cw721A)

	// Evil chain responds with:
	//
	// class ID: class ID of sent NFT
	// token IDs: [chainAToken, chainAToken]
	_, err := suite.chainC.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainC.SenderAccount.GetAddress().String(),
		Contract: suite.bridgeC.String(),
		Msg:      []byte(fmt.Sprintf(`{ "send_packet": { "channel_id": "%s", "timeout": { "timestamp": "%d" }, "data": {"classId":"%s","classUri":"https://metadata-url.com/my-metadata","tokenIds":["%s", "%s"],"tokenUris":["https://metadata-url.com/my-metadata1", "https://moonphase.is/image.svg"],"sender":"%s","receiver":"%s"} }}`, pathCA.EndpointA.ChannelID, suite.coordinator.CurrentTime.Add(time.Hour*100).UnixNano(), chainCClassId, suite.tokenIdA, suite.tokenIdA, suite.chainC.SenderAccount.GetAddress().String(), suite.chainA.SenderAccount.GetAddress().String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.UpdateTime()
	suite.coordinator.RelayAndAckPendingPackets(suite.pathAC.Invert())

	// All assumptions have now been violated.
	//
	// 1. Remote chain says it has minted a new version of our
	//    local NFT on its chain.
	// 2. Remote chian says that there are two NFTs belonging to
	//    the same collection with the same token ID.
	//
	// Bridge contract is a based and does not care what other
	// chain's NFT rules are. Only rule is that NFTs on bridge
	// contract's chain follow bridge contract's chain's NFT
	// rules. Bridge contract says:
	//
	// > I know one of those tokens is valid and corresponds to the
	// > NFT I previously sent away so I will return that one to
	// > the recipient. For all I know chain C social norms allow
	// > for more than one collection with the same ID, so for
	// > that one I will create a new collection (so that it
	// > follows my chain's social norms) and give a token for
	// > that collection for the receiver.
	chainAOwner := queryGetOwnerOf(suite.T(), suite.chainA, suite.cw721A.String())
	require.Equal(suite.T(), suite.chainA.SenderAccount.GetAddress().String(), chainAOwner)

	chainAClassId := fmt.Sprintf("%s/%s/%s/%s/%s", suite.pathAC.EndpointA.ChannelConfig.PortID, suite.pathAC.EndpointA.ChannelID, pathCA.EndpointA.ChannelConfig.PortID, pathCA.EndpointA.ChannelID, suite.cw721A)
	chainANft := queryGetNftForClass(suite.T(), suite.chainA, suite.bridgeA.String(), chainAClassId)
	chainAOwner = queryGetOwnerOf(suite.T(), suite.chainA, chainANft)
	require.Equal(suite.T(), suite.chainA.SenderAccount.GetAddress().String(), chainAOwner)
}
