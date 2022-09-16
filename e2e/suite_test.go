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
	// suite.coordinator.CommitBlock(suite.chainA, suite.chainB)

	// Store the bridge contract.
	chainAStoreResp := suite.chainA.StoreCodeFile("../artifacts/cw_ics721_bridge.wasm")
	require.Equal(suite.T(), uint64(1), chainAStoreResp.CodeID)
	chainBStoreResp := suite.chainB.StoreCodeFile("../artifacts/cw_ics721_bridge.wasm")
	require.Equal(suite.T(), uint64(1), chainBStoreResp.CodeID)

	// Store the cw721 contract.
	chainAStoreResp = suite.chainA.StoreCodeFile("../artifacts/cw721_base.wasm")
	require.Equal(suite.T(), uint64(2), chainAStoreResp.CodeID)
	chainBStoreResp = suite.chainB.StoreCodeFile("../artifacts/cw721_base.wasm")
	require.Equal(suite.T(), uint64(2), chainBStoreResp.CodeID)

	// Store the cw721_base contract.

	instantiateICS721 := InstantiateICS721Bridge{
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
	// suite.coordinator.CommitBlock(suite.chainA, suite.chainB)
	// suite.coordinator.UpdateTime()

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
	cw721 := suite.chainA.InstantiateContract(2, instantiateRaw)

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

	// Check that the classID for the contract has been set properly.
	getClassIDQuery := GetClassIDForNFTContractQuery{
		GetClassIDForNFTContract: GetClassIDForNFTContractQueryData{
			Contract: chainBCw721,
		},
	}
	getClassIDResponse := GetClassIDForNFTContractResponse{}
	err = suite.chainB.SmartQuery(suite.chainBBridge.String(), getClassIDQuery, &getClassIDResponse)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), fmt.Sprintf("%s/%s/%s", counterpartPortID, "channel-0", cw721), getClassIDResponse.ClassID)

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

// Instantiates a bridge contract on CHAIN. Returns the address of the
// instantiated contract.
func instantiateBridge(t *testing.T, chain *wasmibctesting.TestChain) sdk.AccAddress {
	// Store the contracts.
	bridgeresp := chain.StoreCodeFile("../artifacts/cw_ics721_bridge.wasm")
	cw721resp := chain.StoreCodeFile("../artifacts/cw721_base.wasm")

	// Instantiate the bridge contract.
	instantiateICS721 := InstantiateICS721Bridge{
		cw721resp.CodeID,
	}
	instantiateICS721Raw, err := json.Marshal(instantiateICS721)
	require.NoError(t, err)
	return chain.InstantiateContract(bridgeresp.CodeID, instantiateICS721Raw)
}

func instantiateCw721(t *testing.T, chain *wasmibctesting.TestChain) sdk.AccAddress {
	cw721resp := chain.StoreCodeFile("../artifacts/cw721_base.wasm")
	cw721Instantiate := InstantiateCw721{
		"bad/kids",
		"bad/kids",
		chain.SenderAccount.GetAddress().String(),
	}
	instantiateRaw, err := json.Marshal(cw721Instantiate)
	require.NoError(t, err)
	return chain.InstantiateContract(cw721resp.CodeID, instantiateRaw)
}

func mintNFT(t *testing.T, chain *wasmibctesting.TestChain, cw721 string, id string, receiver sdk.AccAddress) {
	_, err := chain.SendMsgs(&wasmtypes.MsgExecuteContract{
		// Sender account is the minter in our test universe.
		Sender:   chain.SenderAccount.GetAddress().String(),
		Contract: cw721,
		Msg:      []byte(fmt.Sprintf(`{ "mint": { "token_id": "%s", "owner": "%s" } }`, id, receiver.String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(t, err)
}

func ics721Nft(t *testing.T, chain *wasmibctesting.TestChain, path *wasmibctesting.Path, coordinator *wasmibctesting.Coordinator, nft string, bridge, sender, receiver sdk.AccAddress) {
	ibcAway := fmt.Sprintf(`{ "receiver": "%s", "channel_id": "%s", "timeout": { "timestamp": "%d" } }`, receiver.String(), path.EndpointA.ChannelID, coordinator.CurrentTime.UnixNano()+1000000000000)
	ibcAwayEncoded := b64.StdEncoding.EncodeToString([]byte(ibcAway))

	// Send the NFT away.
	_, err := chain.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   sender.String(),
		Contract: nft,
		Msg:      []byte(fmt.Sprintf(`{ "send_nft": { "contract": "%s", "token_id": "bad kid 1", "msg": "%s" } }`, bridge, ibcAwayEncoded)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(t, err)
	coordinator.RelayAndAckPendingPackets(path)
}

func queryGetClass(t *testing.T, chain *wasmibctesting.TestChain, bridge, classID string) string {
	getClassQuery := GetClassQuery{
		GetClass: GetClassQueryData{
			ClassID: classID,
		},
	}
	cw721 := ""
	err := chain.SmartQuery(bridge, getClassQuery, &cw721)
	require.NoError(t, err)
	return cw721
}

func queryGetOwner(t *testing.T, chain *wasmibctesting.TestChain, nft string) string {
	resp := OwnerOfResponse{}
	ownerOfQuery := OwnerOfQuery{
		OwnerOf: OwnerOfQueryData{
			TokenID: "bad kid 1",
		},
	}
	err := chain.SmartQuery(nft, ownerOfQuery, &resp)
	require.NoError(t, err)
	return resp.Owner
}

// Builds three identical chains A, B, and C then sends along the path
// A -> B -> C -> A -> C -> B -> A. If this works, likely most other
// things do too. :)
func TestSendBetweenThreeIdenticalChains(t *testing.T) {
	coordinator := wasmibctesting.NewCoordinator(t, 3)

	chainA := coordinator.GetChain(wasmibctesting.GetChainID(0))
	chainB := coordinator.GetChain(wasmibctesting.GetChainID(1))
	chainC := coordinator.GetChain(wasmibctesting.GetChainID(2))

	// Chains are identical, so only one bridge address.
	bridge := instantiateBridge(t, chainA)
	instantiateBridge(t, chainB)
	instantiateBridge(t, chainC)

	chainANft := instantiateCw721(t, chainA).String()
	mintNFT(t, chainA, chainANft, "bad kid 1", chainA.SenderAccount.GetAddress())

	type Connection struct {
		Start int
		End   int
	}
	paths := make(map[Connection]*wasmibctesting.Path)
	addPath := func(path *wasmibctesting.Path, start, end int) {
		paths[Connection{start, end}] = path
		paths[Connection{end, start}] = path.Invert()
	}
	var getPath func(start, end int) *wasmibctesting.Path
	getPath = func(start, end int) *wasmibctesting.Path {
		if path, ok := paths[Connection{start, end}]; ok {
			return path
		}
		startChain := coordinator.GetChain(wasmibctesting.GetChainID(start))
		endChain := coordinator.GetChain(wasmibctesting.GetChainID(end))

		sourcePortID := startChain.ContractInfo(bridge).IBCPortID
		counterpartPortID := endChain.ContractInfo(bridge).IBCPortID

		path := wasmibctesting.NewPath(startChain, endChain)
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

		coordinator.SetupConnections(path)
		coordinator.CreateChannels(path)

		addPath(path, start, end)
		return getPath(start, end)
	}

	// A -> B
	path := getPath(0, 1)
	ics721Nft(t, chainA, path, coordinator, chainANft, bridge, chainA.SenderAccount.GetAddress(), chainB.SenderAccount.GetAddress())

	// Check that chain B received the NFT.
	chainBClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, chainANft)
	chainBNft := queryGetClass(t, chainB, bridge.String(), chainBClassID)
	t.Logf("chain B cw721: %s", chainBNft)

	ownerB := queryGetOwner(t, chainB, chainBNft)
	require.Equal(t, chainB.SenderAccount.GetAddress().String(), ownerB)

	// Make sure chain A has the NFT in its bridge contract.
	ownerA := queryGetOwner(t, chainA, chainANft)
	require.Equal(t, ownerA, bridge.String())

	// B -> C
	path = getPath(1, 2)
	ics721Nft(t, chainB, path, coordinator, chainBNft, bridge, chainB.SenderAccount.GetAddress(), chainC.SenderAccount.GetAddress())

	chainCClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, chainBClassID)
	chainCNft := queryGetClass(t, chainC, bridge.String(), chainCClassID)
	t.Logf("chain C cw721: %s", chainCNft)

	ownerC := queryGetOwner(t, chainC, chainCNft)
	require.Equal(t, chainC.SenderAccount.GetAddress().String(), ownerC)

	// Make sure the NFT is locked in the bridge contract on chain B.
	ownerB = queryGetOwner(t, chainB, chainBNft)
	require.Equal(t, bridge.String(), ownerB)

	// C -> A
	path = getPath(2, 0)
	ics721Nft(t, chainC, path, coordinator, chainCNft, bridge, chainC.SenderAccount.GetAddress(), chainA.SenderAccount.GetAddress())
	chainAClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, chainCClassID)
	// This is a derivative and not actually the original chain A nft.
	chainANftDerivative := queryGetClass(t, chainA, bridge.String(), chainAClassID)
	require.NotEqual(t, chainANft, chainANftDerivative)
	t.Logf("chain A cw721 derivative: %s", chainANftDerivative)

	ownerA = queryGetOwner(t, chainA, chainANftDerivative)
	require.Equal(t, chainA.SenderAccount.GetAddress().String(), ownerA)

	// Make sure that the NFT is held in the bridge contract now.
	ownerC = queryGetOwner(t, chainC, chainCNft)
	require.Equal(t, bridge.String(), ownerC)

	// Now, lets unwind the stack.

	// A -> C
	path = getPath(0, 2)
	ics721Nft(t, chainA, path, coordinator, chainANftDerivative, bridge, chainA.SenderAccount.GetAddress(), chainC.SenderAccount.GetAddress())

	// NFT should now be burned on chain A. We can't ask the
	// contract "is this burned" so we just query and make sure it
	// now errors with a storage load failure.
	err := chainA.SmartQuery(chainANftDerivative, OwnerOfQuery{OwnerOf: OwnerOfQueryData{TokenID: "bad kid 1"}}, &OwnerOfResponse{})
	require.ErrorContains(t, err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>> not found")

	// NFT should belong to chainC sender on chain C.
	ownerC = queryGetOwner(t, chainC, chainCNft)
	require.Equal(t, chainC.SenderAccount.GetAddress().String(), ownerC)

	// C -> B
	path = getPath(2, 1)
	ics721Nft(t, chainC, path, coordinator, chainCNft, bridge, chainC.SenderAccount.GetAddress(), chainB.SenderAccount.GetAddress())

	// Received on B.
	ownerB = queryGetOwner(t, chainB, chainBNft)
	require.Equal(t, chainB.SenderAccount.GetAddress().String(), ownerB)

	// Burned on C.
	err = chainC.SmartQuery(chainCNft, OwnerOfQuery{OwnerOf: OwnerOfQueryData{TokenID: "bad kid 1"}}, &OwnerOfResponse{})
	require.ErrorContains(t, err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>> not found")

	// B -> A
	path = getPath(1, 0)
	ics721Nft(t, chainB, path, coordinator, chainBNft, bridge, chainB.SenderAccount.GetAddress(), chainA.SenderAccount.GetAddress())

	// Received on chain A.
	ownerA = queryGetOwner(t, chainA, chainANft)
	require.Equal(t, chainA.SenderAccount.GetAddress().String(), ownerA)

	// Burned on chain B.
	err = chainB.SmartQuery(chainBNft, OwnerOfQuery{OwnerOf: OwnerOfQueryData{TokenID: "bad kid 1"}}, &OwnerOfResponse{})
	require.ErrorContains(t, err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>> not found")

	// Hooray! We have completed the journey between three
	// identical blockchains using our bridge contract.
}
