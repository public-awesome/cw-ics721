package e2e_test

import (
	"encoding/json"
	"testing"

	b64 "encoding/base64"
	"fmt"

	wasmd "github.com/CosmWasm/wasmd/app"
	wasmibctesting "github.com/CosmWasm/wasmd/x/wasm/ibctesting"
	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"
	sdk "github.com/cosmos/cosmos-sdk/types"
	channeltypes "github.com/cosmos/ibc-go/v4/modules/core/04-channel/types"
	ibctesting "github.com/cosmos/ibc-go/v4/testing"
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

	// Store the ICS721 contract.
	chainAStoreResp := suite.chainA.StoreCodeFile("../artifacts/ics721_base.wasm")
	require.Equal(suite.T(), uint64(1), chainAStoreResp.CodeID)
	chainBStoreResp := suite.chainB.StoreCodeFile("../artifacts/ics721_base.wasm")
	require.Equal(suite.T(), uint64(1), chainBStoreResp.CodeID)

	// Store the cw721 contract.
	chainAStoreResp = suite.chainA.StoreCodeFile("../external-wasms/cw721_base_v0.18.0.wasm")
	require.Equal(suite.T(), uint64(2), chainAStoreResp.CodeID)
	chainBStoreResp = suite.chainB.StoreCodeFile("../external-wasms/cw721_base_v0.18.0.wasm")
	require.Equal(suite.T(), uint64(2), chainBStoreResp.CodeID)

	instantiateICS721 := InstantiateICS721Bridge{
		2,
		// no pauser nor proxy by default.
		nil,
		nil,
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

	suite.T().Logf("chain a sender: %s", suite.chainA.SenderAccount.GetAddress().String())
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
	require.Equal(suite.T(), suite.chainABridge.String(), resp.Owner)

	chainBClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, cw721.String())

	// Check that the receiver on the receiving chain now owns the NFT.
	getOwnerQuery := OwnerQuery{
		Owner: OwnerQueryData{
			TokenID: "1",
			ClassID: chainBClassID,
		},
	}
	err = suite.chainB.SmartQuery(suite.chainBBridge.String(), getOwnerQuery, &resp)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), suite.chainB.SenderAccount.GetAddress().String(), resp.Owner)

	// Get the address of the instantiated cw721.
	getClassQuery := NftContractQuery{
		NftContractForClassId: NftContractQueryData{
			ClassID: chainBClassID,
		},
	}
	chainBCw721 := ""

	err = suite.chainB.SmartQuery(suite.chainBBridge.String(), getClassQuery, &chainBCw721)
	require.NoError(suite.T(), err)
	suite.T().Logf("Chain B cw721: %s", chainBCw721)

	// Check that the classID for the contract has been set properly.
	getClassIDQuery := ClassIdQuery{
		ClassIdForNFTContract: ClassIdQueryData{
			Contract: chainBCw721,
		},
	}
	var getClassIdResponse string
	err = suite.chainB.SmartQuery(suite.chainBBridge.String(), getClassIDQuery, &getClassIdResponse)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), fmt.Sprintf("%s/%s/%s", counterpartPortID, "channel-0", cw721), getClassIdResponse)

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

	// Send the NFT back!

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
	getClassQuery = NftContractQuery{
		NftContractForClassId: NftContractQueryData{
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

func TestIBC(t *testing.T) {
	suite.Run(t, new(TransferTestSuite))
}

// Instantiates a ICS721 contract on CHAIN. Returns the address of the
// instantiated contract.
func instantiateBridge(t *testing.T, chain *wasmibctesting.TestChain) sdk.AccAddress {
	// Store the contracts.
	bridgeresp := chain.StoreCodeFile("../artifacts/ics721_base.wasm")
	cw721resp := chain.StoreCodeFile("../external-wasms/cw721_base_v0.18.0.wasm")

	// Instantiate the ICS721 contract.
	instantiateICS721 := InstantiateICS721Bridge{
		cw721resp.CodeID,
		nil,
		nil,
	}
	instantiateICS721Raw, err := json.Marshal(instantiateICS721)
	require.NoError(t, err)
	return chain.InstantiateContract(bridgeresp.CodeID, instantiateICS721Raw)
}

func instantiateCw721(t *testing.T, chain *wasmibctesting.TestChain) sdk.AccAddress {
	cw721resp := chain.StoreCodeFile("../external-wasms/cw721_base_v0.18.0.wasm")
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

func transferNft(t *testing.T, chain *wasmibctesting.TestChain, nft, token_id string, sender, receiver sdk.AccAddress) {
	_, err := chain.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   sender.String(),
		Contract: nft,
		Msg:      []byte(fmt.Sprintf(`{"transfer_nft": { "recipient": "%s", "token_id": "%s" }}`, receiver, token_id)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(t, err)
}

func getCw721SendIbcAwayMessage(path *wasmibctesting.Path, coordinator *wasmibctesting.Coordinator, tokenId string, bridge, receiver sdk.AccAddress, timeout int64) string {
	ibcAway := fmt.Sprintf(`{ "receiver": "%s", "channel_id": "%s", "timeout": { "timestamp": "%d" } }`, receiver.String(), path.EndpointA.ChannelID, timeout)
	ibcAwayEncoded := b64.StdEncoding.EncodeToString([]byte(ibcAway))
	return fmt.Sprintf(`{ "send_nft": { "contract": "%s", "token_id": "%s", "msg": "%s" } }`, bridge, tokenId, ibcAwayEncoded)
}

func ics721Nft(t *testing.T, chain *wasmibctesting.TestChain, path *wasmibctesting.Path, coordinator *wasmibctesting.Coordinator, nft string, bridge, sender, receiver sdk.AccAddress) {
	// Send the NFT away.
	_, err := chain.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   sender.String(),
		Contract: nft,
		Msg:      []byte(getCw721SendIbcAwayMessage(path, coordinator, "bad kid 1", bridge, receiver, coordinator.CurrentTime.UnixNano()+1000000000000)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(t, err)
	coordinator.RelayAndAckPendingPackets(path)
}

func queryGetNftForClass(t *testing.T, chain *wasmibctesting.TestChain, bridge, classID string) string {
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

func queryGetOwnerOf(t *testing.T, chain *wasmibctesting.TestChain, nft string) string {
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

	// Chains are identical, so only one ICS721 contract address.
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
	chainBNft := queryGetNftForClass(t, chainB, bridge.String(), chainBClassID)
	t.Logf("chain B cw721: %s", chainBNft)

	ownerB := queryGetOwnerOf(t, chainB, chainBNft)
	require.Equal(t, chainB.SenderAccount.GetAddress().String(), ownerB)

	// Make sure chain A has the NFT in its ICS721 contract.
	ownerA := queryGetOwnerOf(t, chainA, chainANft)
	require.Equal(t, ownerA, bridge.String())

	// B -> C
	path = getPath(1, 2)
	ics721Nft(t, chainB, path, coordinator, chainBNft, bridge, chainB.SenderAccount.GetAddress(), chainC.SenderAccount.GetAddress())

	chainCClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, chainBClassID)
	chainCNft := queryGetNftForClass(t, chainC, bridge.String(), chainCClassID)
	t.Logf("chain C cw721: %s", chainCNft)

	ownerC := queryGetOwnerOf(t, chainC, chainCNft)
	require.Equal(t, chainC.SenderAccount.GetAddress().String(), ownerC)

	// Make sure the NFT is locked in the ICS721 contract on chain B.
	ownerB = queryGetOwnerOf(t, chainB, chainBNft)
	require.Equal(t, bridge.String(), ownerB)

	// C -> A
	path = getPath(2, 0)
	ics721Nft(t, chainC, path, coordinator, chainCNft, bridge, chainC.SenderAccount.GetAddress(), chainA.SenderAccount.GetAddress())
	chainAClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, chainCClassID)
	// This is a derivative and not actually the original chain A nft.
	chainANftDerivative := queryGetNftForClass(t, chainA, bridge.String(), chainAClassID)
	require.NotEqual(t, chainANft, chainANftDerivative)
	t.Logf("chain A cw721 derivative: %s", chainANftDerivative)

	ownerA = queryGetOwnerOf(t, chainA, chainANftDerivative)
	require.Equal(t, chainA.SenderAccount.GetAddress().String(), ownerA)

	// Make sure that the NFT is held in the ICS721 contract now.
	ownerC = queryGetOwnerOf(t, chainC, chainCNft)
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
	ownerC = queryGetOwnerOf(t, chainC, chainCNft)
	require.Equal(t, chainC.SenderAccount.GetAddress().String(), ownerC)

	// C -> B
	path = getPath(2, 1)
	ics721Nft(t, chainC, path, coordinator, chainCNft, bridge, chainC.SenderAccount.GetAddress(), chainB.SenderAccount.GetAddress())

	// Received on B.
	ownerB = queryGetOwnerOf(t, chainB, chainBNft)
	require.Equal(t, chainB.SenderAccount.GetAddress().String(), ownerB)

	// Burned on C.
	err = chainC.SmartQuery(chainCNft, OwnerOfQuery{OwnerOf: OwnerOfQueryData{TokenID: "bad kid 1"}}, &OwnerOfResponse{})
	require.ErrorContains(t, err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>> not found")

	// B -> A
	path = getPath(1, 0)
	ics721Nft(t, chainB, path, coordinator, chainBNft, bridge, chainB.SenderAccount.GetAddress(), chainA.SenderAccount.GetAddress())

	// Received on chain A.
	ownerA = queryGetOwnerOf(t, chainA, chainANft)
	require.Equal(t, chainA.SenderAccount.GetAddress().String(), ownerA)

	// Burned on chain B.
	err = chainB.SmartQuery(chainBNft, OwnerOfQuery{OwnerOf: OwnerOfQueryData{TokenID: "bad kid 1"}}, &OwnerOfResponse{})
	require.ErrorContains(t, err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>> not found")

	// Hooray! We have completed the journey between three
	// identical blockchains using our ICS721 contract.
}

func (suite *TransferTestSuite) TestMultipleAddressesInvolved() {
	var (
		sourcePortID      = suite.chainA.ContractInfo(suite.chainABridge).IBCPortID
		counterpartPortID = suite.chainB.ContractInfo(suite.chainBBridge).IBCPortID
	)
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

	chainANft := instantiateCw721(suite.T(), suite.chainA)
	mintNFT(suite.T(), suite.chainA, chainANft.String(), "bad kid 1", suite.chainA.SenderAccount.GetAddress())

	ics721Nft(suite.T(), suite.chainA, path, suite.coordinator, chainANft.String(), suite.chainABridge, suite.chainA.SenderAccount.GetAddress(), suite.chainB.SenderAccount.GetAddress())

	chainBClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, chainANft)
	chainBNft := queryGetNftForClass(suite.T(), suite.chainB, suite.chainBBridge.String(), chainBClassID)

	// Generate a new account and transfer the NFT to it.  For
	// reasons entirely beyond me, the first account we create
	// has an account number of ten. The second has 18.
	newAccount := CreateAndFundAccount(suite.T(), suite.chainB, 18)
	transferNft(suite.T(), suite.chainB, chainBNft, "bad kid 1", suite.chainB.SenderAccount.GetAddress(), newAccount.Address)

	// IBC away the transfered NFT.
	ibcAway := fmt.Sprintf(`{ "receiver": "%s", "channel_id": "%s", "timeout": { "timestamp": "%d" } }`, suite.chainA.SenderAccount.GetAddress().String(), path.EndpointB.ChannelID, suite.coordinator.CurrentTime.UnixNano()+1000000000000)
	ibcAwayEncoded := b64.StdEncoding.EncodeToString([]byte(ibcAway))

	// Send the NFT away.
	_, err := SendMsgsFromAccount(suite.T(), suite.chainB, newAccount, &wasmtypes.MsgExecuteContract{
		Sender:   newAccount.Address.String(),
		Contract: chainBNft,
		Msg:      []byte(fmt.Sprintf(`{ "send_nft": { "contract": "%s", "token_id": "bad kid 1", "msg": "%s" } }`, suite.chainBBridge.String(), ibcAwayEncoded)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.RelayAndAckPendingPackets(path.Invert())

	// Make sure the NFT was burned on chain B
	err = suite.chainB.SmartQuery(chainBNft, OwnerOfQuery{OwnerOf: OwnerOfQueryData{TokenID: "bad kid 1"}}, &OwnerOfResponse{})
	require.ErrorContains(suite.T(), err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>> not found")

	// Make another account on chain B and transfer to the new account.
	anotherAcount := CreateAndFundAccount(suite.T(), suite.chainB, 19)
	ics721Nft(suite.T(), suite.chainA, path, suite.coordinator, chainANft.String(), suite.chainABridge, suite.chainA.SenderAccount.GetAddress(), anotherAcount.Address)

	// Transfer it back to chain A using this new account.
	_, err = SendMsgsFromAccount(suite.T(), suite.chainB, anotherAcount, &wasmtypes.MsgExecuteContract{
		Sender:   anotherAcount.Address.String(),
		Contract: chainBNft,
		Msg:      []byte(fmt.Sprintf(`{ "send_nft": { "contract": "%s", "token_id": "bad kid 1", "msg": "%s" } }`, suite.chainBBridge.String(), ibcAwayEncoded)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.RelayAndAckPendingPackets(path.Invert())

	// Make sure it was burned on B.
	err = suite.chainB.SmartQuery(chainBNft, OwnerOfQuery{OwnerOf: OwnerOfQueryData{TokenID: "bad kid 1"}}, &OwnerOfResponse{})
	require.ErrorContains(suite.T(), err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>> not found")

	// Make sure it is owned by the correct address on A.
	resp := OwnerOfResponse{}
	err = suite.chainA.SmartQuery(chainANft.String(), OwnerOfQuery{OwnerOf: OwnerOfQueryData{TokenID: "bad kid 1"}}, &resp)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), resp.Owner, suite.chainA.SenderAccount.GetAddress().String())
}

func TestCloseRejected(t *testing.T) {
	coordinator := wasmibctesting.NewCoordinator(t, 2)
	chainA := coordinator.GetChain(wasmibctesting.GetChainID(0))
	chainB := coordinator.GetChain(wasmibctesting.GetChainID(1))

	// Store the ICS721 contract.
	chainAStoreResp := chainA.StoreCodeFile("../artifacts/ics721_base.wasm")
	require.Equal(t, uint64(1), chainAStoreResp.CodeID)
	chainBStoreResp := chainB.StoreCodeFile("../artifacts/ics721_base.wasm")
	require.Equal(t, uint64(1), chainBStoreResp.CodeID)

	// Store the cw721 contract.
	chainAStoreResp = chainA.StoreCodeFile("../external-wasms/cw721_base_v0.18.0.wasm")
	require.Equal(t, uint64(2), chainAStoreResp.CodeID)
	chainBStoreResp = chainB.StoreCodeFile("../external-wasms/cw721_base_v0.18.0.wasm")
	require.Equal(t, uint64(2), chainBStoreResp.CodeID)

	// Store the cw721_base contract.
	instantiateICS721 := InstantiateICS721Bridge{
		2,
		nil,
		nil,
	}
	instantiateICS721Raw, err := json.Marshal(instantiateICS721)
	require.NoError(t, err)
	chainABridge := chainA.InstantiateContract(1, instantiateICS721Raw)
	chainBBridge := chainB.InstantiateContract(1, instantiateICS721Raw)

	var (
		sourcePortID      = chainA.ContractInfo(chainABridge).IBCPortID
		counterpartPortID = chainB.ContractInfo(chainBBridge).IBCPortID
	)

	path := wasmibctesting.NewPath(chainA, chainB)

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

	// CloseInit should not be allowed.

	// CloseInit happens if someone on our side of the channel
	// attempts to close it. We should reject this and keep the
	// channel open.

	// For this version we are account number 17. Why not 10? Why
	// not 1? These are some of the world's great mysteries.
	newAccount := CreateAndFundAccount(t, chainA, 17)

	// Make sure ChanCloseInit is rejected.
	msg := channeltypes.NewMsgChannelCloseInit(path.EndpointA.ChannelConfig.PortID, path.EndpointA.ChannelID, newAccount.Address.String())
	chainA.Coordinator.UpdateTimeForChain(chainA)

	_, _, err = wasmd.SignAndDeliver(
		t,
		chainA.TxConfig,
		chainA.App.BaseApp,
		chainA.GetContext().BlockHeader(),
		[]sdk.Msg{msg},
		chainA.ChainID,
		[]uint64{newAccount.Acc.GetAccountNumber()},
		[]uint64{newAccount.Acc.GetSequence()},
		newAccount.PrivKey)
	require.Error(t, err)
	require.ErrorContains(t, err, "ICS 721 channels may not be closed")

	chainA.NextBlock()
	err = newAccount.Acc.SetSequence(newAccount.Acc.GetSequence() + 1)
	require.NoError(t, err)
	chainA.Coordinator.IncrementTime()

	// Check that we successfully stopped the channel from
	// closing.
	require.Equal(t, path.EndpointA.GetChannel().State, channeltypes.OPEN)

	// Force the other side of the channel to close so that proofs
	// work. The meer fact that we need to submit a proof that the
	// counterparty channel is in the CLOSED state in order to
	// call this strongly suggests that we are powerless to stop
	// channel's closing in CloseConfirm.
	err = path.EndpointB.SetChannelClosed()
	require.NoError(t, err)
	// Send the CloseConfirmMethod on our side. At this point the
	// contract should realize it is doomed and not error so as to
	// keep the state of the two chains consistent.
	//
	// If we get a `ChannelCloseConfirm` message, the other side
	// is already closed.
	//
	// <https://github.com/cosmos/ibc/blob/main/spec/core/ics-004-channel-and-packet-semantics/README.md#closing-handshake>
	path.EndpointA.ChanCloseConfirm()

	require.Equal(t, path.EndpointB.GetChannel().State, channeltypes.CLOSED)
	require.Equal(t, path.EndpointA.GetChannel().State, channeltypes.CLOSED)
}

func (suite *TransferTestSuite) TestPacketTimeoutCausesRefund() {
	var (
		sourcePortID      = suite.chainA.ContractInfo(suite.chainABridge).IBCPortID
		counterpartPortID = suite.chainB.ContractInfo(suite.chainBBridge).IBCPortID
	)

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

	cw721 := instantiateCw721(suite.T(), suite.chainA)
	mintNFT(suite.T(), suite.chainA, cw721.String(), "bad kid 1", suite.chainA.SenderAccount.GetAddress())

	// IBC away message that will expire in one second.
	ibcAway := fmt.Sprintf(`{ "receiver": "%s", "channel_id": "%s", "timeout": { "timestamp": "%d" } }`, suite.chainB.SenderAccount.GetAddress().String(), path.EndpointA.ChannelID, suite.coordinator.CurrentTime.UnixNano()+1000000)
	ibcAwayEncoded := b64.StdEncoding.EncodeToString([]byte(ibcAway))
	_, err := suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: cw721.String(),
		Msg:      []byte(fmt.Sprintf(`{ "send_nft": { "contract": "%s", "token_id": "bad kid 1", "msg": "%s" } }`, suite.chainABridge, ibcAwayEncoded)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)

	suite.coordinator.TimeoutPendingPackets(path)

	// NFTs should be returned to sender on packet timeout.
	owner := queryGetOwnerOf(suite.T(), suite.chainA, cw721.String())
	require.Equal(suite.T(), suite.chainA.SenderAccount.GetAddress().String(), owner)
}

// Tests that the NFT transfered to the ICS721 contract is returned to sender
// if the counterparty returns an ack fail while handling the
// transfer.
func (suite *TransferTestSuite) TestRefundOnAckFail() {
	var (
		sourcePortID      = suite.chainA.ContractInfo(suite.chainABridge).IBCPortID
		counterpartPortID = suite.chainB.ContractInfo(suite.chainBBridge).IBCPortID
	)
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

	chainANft := instantiateCw721(suite.T(), suite.chainA)
	mintNFT(suite.T(), suite.chainA, chainANft.String(), "bad kid 1", suite.chainA.SenderAccount.GetAddress())

	// Send the NFT, but use an invalid address in the receiver
	// field. This will cause processing to fail. The counterparty
	// should not commit any new state and should respond with an
	// ack fail which should cause the sent NFT to be returned to
	// the sender.
	ibcAway := fmt.Sprintf(`{ "receiver": "%s", "channel_id": "%s", "timeout": { "timestamp": "%d" } }`, "ekez", path.EndpointA.ChannelID, suite.coordinator.CurrentTime.UnixNano()+1000000000000)
	ibcAwayEncoded := b64.StdEncoding.EncodeToString([]byte(ibcAway))

	// Send the NFT away.
	_, err := suite.chainA.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   suite.chainA.SenderAccount.GetAddress().String(),
		Contract: chainANft.String(),
		Msg:      []byte(fmt.Sprintf(`{ "send_nft": { "contract": "%s", "token_id": "bad kid 1", "msg": "%s" } }`, suite.chainABridge.String(), ibcAwayEncoded)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.RelayAndAckPendingPackets(path)

	chainBClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, chainANft)
	chainBNft := queryGetNftForClass(suite.T(), suite.chainB, suite.chainBBridge.String(), chainBClassID)
	require.Equal(suite.T(), chainBNft, "")

	// Check that the NFT was returned to the sender due to the failure.
	ownerA := queryGetOwnerOf(suite.T(), suite.chainA, chainANft.String())
	require.Equal(suite.T(), suite.chainA.SenderAccount.GetAddress().String(), ownerA)
}
