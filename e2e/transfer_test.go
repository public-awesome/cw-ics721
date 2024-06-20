package e2e

import (
	"encoding/json"
	"testing"

	"github.com/public-awesome/ics721/e2e/test_suite"

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

func TestTransfer(t *testing.T) {
	suite.Run(t, new(TransferTestSuite))
}

func (suite *TransferTestSuite) SetupTest() {
	suite.coordinator = wasmibctesting.NewCoordinator(suite.T(), 2)
	suite.chainA = suite.coordinator.GetChain(wasmibctesting.GetChainID(0))
	suite.chainB = suite.coordinator.GetChain(wasmibctesting.GetChainID(1))
	// test_suite.coordinator.CommitBlock(test_suite.chainA, test_suite.chainB)

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

	instantiateICS721 := test_suite.InstantiateICS721Bridge{
		Cw721BaseCodeId: 2,
		// no pauser nor proxy by default.
		OutgoingProxy: nil,
		IncomingProxy: nil,
		Pauser:        nil,
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
	// test_suite.coordinator.CommitBlock(test_suite.chainA, test_suite.chainB)
	// test_suite.coordinator.UpdateTime()

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
	cw721Instantiate := test_suite.InstantiateCw721v18{
		Name:            "bad/kids",
		Symbol:          "bad/kids",
		Minter:          suite.chainA.SenderAccount.GetAddress().String(),
		WithdrawAddress: nil,
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
	resp := test_suite.OwnerOfResponse{}
	ownerOfQuery := test_suite.OwnerOfQuery{
		OwnerOf: test_suite.OwnerOfQueryData{
			TokenID: "1",
		},
	}
	err = suite.chainA.SmartQuery(cw721.String(), ownerOfQuery, &resp)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), suite.chainABridge.String(), resp.Owner)

	chainBClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, cw721.String())

	// Check that the receiver on the receiving chain now owns the NFT.
	getOwnerQuery := test_suite.OwnerQuery{
		Owner: test_suite.OwnerQueryData{
			TokenID: "1",
			ClassID: chainBClassID,
		},
	}
	err = suite.chainB.SmartQuery(suite.chainBBridge.String(), getOwnerQuery, &resp)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), suite.chainB.SenderAccount.GetAddress().String(), resp.Owner)

	// Get the address of the instantiated cw721.
	getClassQuery := test_suite.NftContractQuery{
		NftContractForClassId: test_suite.NftContractQueryData{
			ClassID: chainBClassID,
		},
	}
	chainBCw721 := ""

	err = suite.chainB.SmartQuery(suite.chainBBridge.String(), getClassQuery, &chainBCw721)
	require.NoError(suite.T(), err)
	suite.T().Logf("Chain B cw721: %s", chainBCw721)

	// Check that the classID for the contract has been set properly.
	getClassIDQuery := test_suite.ClassIdQuery{
		ClassIdForNFTContract: test_suite.ClassIdQueryData{
			Contract: chainBCw721,
		},
	}
	var getClassIdResponse string
	err = suite.chainB.SmartQuery(suite.chainBBridge.String(), getClassIDQuery, &getClassIdResponse)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), fmt.Sprintf("%s/%s/%s", counterpartPortID, "channel-0", cw721), getClassIdResponse)

	// Check that the contract info for the instantiated cw721 was
	// set correctly.
	contractInfo := test_suite.ContractInfoResponse{}
	contractInfoQuery := test_suite.ContractInfoQuery{
		ContractInfo: test_suite.ContractInfoQueryData{},
	}
	err = suite.chainB.SmartQuery(chainBCw721, contractInfoQuery, &contractInfo)
	require.NoError(suite.T(), err)
	require.Equal(suite.T(), test_suite.ContractInfoResponse{
		Name:   "bad/kids",
		Symbol: "bad/kids",
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
	getClassQuery = test_suite.NftContractQuery{
		NftContractForClassId: test_suite.NftContractQueryData{
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

func TestSendBetweenThreeIdenticalChainsV16(t *testing.T) {
	runTestSendBetweenThreeIdenticalChains(t, 16)
}

func TestSendBetweenThreeIdenticalChainsV17(t *testing.T) {
	runTestSendBetweenThreeIdenticalChains(t, 17)
}

func TestSendBetweenThreeIdenticalChainsV18(t *testing.T) {
	runTestSendBetweenThreeIdenticalChains(t, 18)
}

// Builds three identical chains A, B, and C then sends along the path
// A -> B -> C -> A -> C -> B -> A. If this works, likely most other
// things do too. :)
func runTestSendBetweenThreeIdenticalChains(t *testing.T, version int) {
	coordinator := wasmibctesting.NewCoordinator(t, 3)

	chainA := coordinator.GetChain(wasmibctesting.GetChainID(0))
	chainB := coordinator.GetChain(wasmibctesting.GetChainID(1))
	chainC := coordinator.GetChain(wasmibctesting.GetChainID(2))

	// Chains are identical, so only one ICS721 contract address.
	bridge := test_suite.InstantiateBridge(t, chainA)
	test_suite.InstantiateBridge(t, chainB)
	test_suite.InstantiateBridge(t, chainC)

	chainANft := test_suite.InstantiateCw721(t, chainA, version).String()
	test_suite.MintNFT(t, chainA, chainANft, "bad kid 1", chainA.SenderAccount.GetAddress())

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
	test_suite.Ics721TransferNft(t, chainA, path, coordinator, chainANft, "bad kid 1", bridge, chainA.SenderAccount.GetAddress(), chainB.SenderAccount.GetAddress(), "")

	// After Sending NFT via ICS721 "bridge" contract, check that the NFT is escrowed by ICS721 on chain A.
	chainANftOwner := test_suite.QueryGetOwnerOf(t, chainA, chainANft, "bad kid 1")
	require.Equal(t, chainANftOwner, bridge.String())

	// Check that chain B received the NFT.
	chainBClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, chainANft)
	chainBNft := test_suite.QueryGetNftForClass(t, chainB, bridge.String(), chainBClassID)
	t.Logf("chain B cw721: %s", chainBNft)
	// require.NotEqual(chainBNft, "", "NFT address on chain B is empty")

	ownerB := test_suite.QueryGetOwnerOf(t, chainB, chainBNft, "bad kid 1")
	require.Equal(t, chainB.SenderAccount.GetAddress().String(), ownerB)

	// Make sure chain A has the NFT in its ICS721 contract.
	ownerA := test_suite.QueryGetOwnerOf(t, chainA, chainANft, "bad kid 1")
	require.Equal(t, ownerA, bridge.String())

	// B -> C
	path = getPath(1, 2)
	test_suite.Ics721TransferNft(t, chainB, path, coordinator, chainBNft, "bad kid 1", bridge, chainB.SenderAccount.GetAddress(), chainC.SenderAccount.GetAddress(), "")

	chainCClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, chainBClassID)
	chainCNft := test_suite.QueryGetNftForClass(t, chainC, bridge.String(), chainCClassID)
	t.Logf("chain C cw721: %s", chainCNft)

	ownerC := test_suite.QueryGetOwnerOf(t, chainC, chainCNft, "bad kid 1")
	require.Equal(t, chainC.SenderAccount.GetAddress().String(), ownerC)

	// Make sure the NFT is locked in the ICS721 contract on chain B.
	ownerB = test_suite.QueryGetOwnerOf(t, chainB, chainBNft, "bad kid 1")
	require.Equal(t, bridge.String(), ownerB)

	// C -> A
	path = getPath(2, 0)
	test_suite.Ics721TransferNft(t, chainC, path, coordinator, chainCNft, "bad kid 1", bridge, chainC.SenderAccount.GetAddress(), chainA.SenderAccount.GetAddress(), "")
	chainAClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, chainCClassID)
	// This is a derivative and not actually the original chain A nft.
	chainANftDerivative := test_suite.QueryGetNftForClass(t, chainA, bridge.String(), chainAClassID)
	require.NotEqual(t, chainANft, chainANftDerivative)
	t.Logf("chain A cw721 derivative: %s", chainANftDerivative)

	ownerA = test_suite.QueryGetOwnerOf(t, chainA, chainANftDerivative, "bad kid 1")
	require.Equal(t, chainA.SenderAccount.GetAddress().String(), ownerA)

	// Make sure that the NFT is held in the ICS721 contract now.
	ownerC = test_suite.QueryGetOwnerOf(t, chainC, chainCNft, "bad kid 1")
	require.Equal(t, bridge.String(), ownerC)

	// Now, lets unwind the stack.

	// A -> C
	path = getPath(0, 2)
	test_suite.Ics721TransferNft(t, chainA, path, coordinator, chainANftDerivative, "bad kid 1", bridge, chainA.SenderAccount.GetAddress(), chainC.SenderAccount.GetAddress(), "")

	// NFT should now be burned on chain A. We can't ask the
	// contract "is this burned" so we just query and make sure it
	// now errors with a storage load failure.
	err := chainA.SmartQuery(chainANftDerivative, test_suite.OwnerOfQuery{OwnerOf: test_suite.OwnerOfQueryData{TokenID: "bad kid 1"}}, &test_suite.OwnerOfResponse{})
	require.ErrorContains(t, err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>>; key: [00, 06, 74, 6F, 6B, 65, 6E, 73, 62, 61, 64, 20, 6B, 69, 64, 20, 31] not found")

	// NFT should belong to chainC sender on chain C.
	ownerC = test_suite.QueryGetOwnerOf(t, chainC, chainCNft, "bad kid 1")
	require.Equal(t, chainC.SenderAccount.GetAddress().String(), ownerC)

	// C -> B
	path = getPath(2, 1)
	test_suite.Ics721TransferNft(t, chainC, path, coordinator, chainCNft, "bad kid 1", bridge, chainC.SenderAccount.GetAddress(), chainB.SenderAccount.GetAddress(), "")

	// Received on B.
	ownerB = test_suite.QueryGetOwnerOf(t, chainB, chainBNft, "bad kid 1")
	require.Equal(t, chainB.SenderAccount.GetAddress().String(), ownerB)

	// Burned on C.
	err = chainC.SmartQuery(chainCNft, test_suite.OwnerOfQuery{OwnerOf: test_suite.OwnerOfQueryData{TokenID: "bad kid 1"}}, &test_suite.OwnerOfResponse{})
	require.ErrorContains(t, err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>>; key: [00, 06, 74, 6F, 6B, 65, 6E, 73, 62, 61, 64, 20, 6B, 69, 64, 20, 31] not found")

	// B -> A
	path = getPath(1, 0)
	test_suite.Ics721TransferNft(t, chainB, path, coordinator, chainBNft, "bad kid 1", bridge, chainB.SenderAccount.GetAddress(), chainA.SenderAccount.GetAddress(), "")

	// Received on chain A.
	ownerA = test_suite.QueryGetOwnerOf(t, chainA, chainANft, "bad kid 1")
	require.Equal(t, chainA.SenderAccount.GetAddress().String(), ownerA)

	// Burned on chain B.
	err = chainB.SmartQuery(chainBNft, test_suite.OwnerOfQuery{OwnerOf: test_suite.OwnerOfQueryData{TokenID: "bad kid 1"}}, &test_suite.OwnerOfResponse{})
	require.ErrorContains(t, err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>>; key: [00, 06, 74, 6F, 6B, 65, 6E, 73, 62, 61, 64, 20, 6B, 69, 64, 20, 31] not found")

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

	chainANft := test_suite.InstantiateCw721(suite.T(), suite.chainA, 18)
	test_suite.MintNFT(suite.T(), suite.chainA, chainANft.String(), "bad kid 1", suite.chainA.SenderAccount.GetAddress())

	test_suite.Ics721TransferNft(suite.T(), suite.chainA, path, suite.coordinator, chainANft.String(), "bad kid 1", suite.chainABridge, suite.chainA.SenderAccount.GetAddress(), suite.chainB.SenderAccount.GetAddress(), "")

	chainBClassID := fmt.Sprintf(`%s/%s/%s`, path.EndpointB.ChannelConfig.PortID, path.EndpointB.ChannelID, chainANft)
	chainBNft := test_suite.QueryGetNftForClass(suite.T(), suite.chainB, suite.chainBBridge.String(), chainBClassID)

	// Generate a new account and transfer the NFT to it.  For
	// reasons entirely beyond me, the first account we create
	// has an account number of ten. The second has 18.
	newAccount := test_suite.CreateAndFundAccount(suite.T(), suite.chainB, 18)
	test_suite.TransferNft(suite.T(), suite.chainB, chainBNft, "bad kid 1", suite.chainB.SenderAccount.GetAddress(), newAccount.Address)

	// IBC away the transfered NFT.
	ibcAway := fmt.Sprintf(`{ "receiver": "%s", "channel_id": "%s", "timeout": { "timestamp": "%d" } }`, suite.chainA.SenderAccount.GetAddress().String(), path.EndpointB.ChannelID, suite.coordinator.CurrentTime.UnixNano()+1000000000000)
	ibcAwayEncoded := b64.StdEncoding.EncodeToString([]byte(ibcAway))

	// Send the NFT away.
	_, err := test_suite.SendMsgsFromAccount(suite.T(), suite.chainB, newAccount, &wasmtypes.MsgExecuteContract{
		Sender:   newAccount.Address.String(),
		Contract: chainBNft,
		Msg:      []byte(fmt.Sprintf(`{ "send_nft": { "contract": "%s", "token_id": "bad kid 1", "msg": "%s" } }`, suite.chainBBridge.String(), ibcAwayEncoded)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.RelayAndAckPendingPackets(path.Invert())

	// Make sure the NFT was burned on chain B
	err = suite.chainB.SmartQuery(chainBNft, test_suite.OwnerOfQuery{OwnerOf: test_suite.OwnerOfQueryData{TokenID: "bad kid 1"}}, &test_suite.OwnerOfResponse{})
	require.ErrorContains(suite.T(), err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>>; key: [00, 06, 74, 6F, 6B, 65, 6E, 73, 62, 61, 64, 20, 6B, 69, 64, 20, 31] not found")

	// Make another account on chain B and transfer to the new account.
	anotherAcount := test_suite.CreateAndFundAccount(suite.T(), suite.chainB, 19)
	test_suite.Ics721TransferNft(suite.T(), suite.chainA, path, suite.coordinator, chainANft.String(), "bad kid 1", suite.chainABridge, suite.chainA.SenderAccount.GetAddress(), anotherAcount.Address, "")

	// Transfer it back to chain A using this new account.
	_, err = test_suite.SendMsgsFromAccount(suite.T(), suite.chainB, anotherAcount, &wasmtypes.MsgExecuteContract{
		Sender:   anotherAcount.Address.String(),
		Contract: chainBNft,
		Msg:      []byte(fmt.Sprintf(`{ "send_nft": { "contract": "%s", "token_id": "bad kid 1", "msg": "%s" } }`, suite.chainBBridge.String(), ibcAwayEncoded)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(suite.T(), err)
	suite.coordinator.RelayAndAckPendingPackets(path.Invert())

	// Make sure it was burned on B.
	err = suite.chainB.SmartQuery(chainBNft, test_suite.OwnerOfQuery{OwnerOf: test_suite.OwnerOfQueryData{TokenID: "bad kid 1"}}, &test_suite.OwnerOfResponse{})
	require.ErrorContains(suite.T(), err, "cw721_base::state::TokenInfo<core::option::Option<cosmwasm_std::results::empty::Empty>>; key: [00, 06, 74, 6F, 6B, 65, 6E, 73, 62, 61, 64, 20, 6B, 69, 64, 20, 31] not found")

	// Make sure it is owned by the correct address on A.
	resp := test_suite.OwnerOfResponse{}
	err = suite.chainA.SmartQuery(chainANft.String(), test_suite.OwnerOfQuery{OwnerOf: test_suite.OwnerOfQueryData{TokenID: "bad kid 1"}}, &resp)
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
	instantiateICS721 := test_suite.InstantiateICS721Bridge{
		Cw721BaseCodeId: 2,
		OutgoingProxy:   nil,
		IncomingProxy:   nil,
		Pauser:          nil,
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
	newAccount := test_suite.CreateAndFundAccount(t, chainA, 17)

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

	cw721 := test_suite.InstantiateCw721(suite.T(), suite.chainA, 18)
	test_suite.MintNFT(suite.T(), suite.chainA, cw721.String(), "bad kid 1", suite.chainA.SenderAccount.GetAddress())

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
	owner := test_suite.QueryGetOwnerOf(suite.T(), suite.chainA, cw721.String(), "bad kid 1")
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

	chainANft := test_suite.InstantiateCw721(suite.T(), suite.chainA, 18)
	test_suite.MintNFT(suite.T(), suite.chainA, chainANft.String(), "bad kid 1", suite.chainA.SenderAccount.GetAddress())

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
	chainBNft := test_suite.QueryGetNftForClass(suite.T(), suite.chainB, suite.chainBBridge.String(), chainBClassID)
	require.Equal(suite.T(), chainBNft, "")

	// Check that the NFT was returned to the sender due to the failure.
	ownerA := test_suite.QueryGetOwnerOf(suite.T(), suite.chainA, chainANft.String(), "bad kid 1")
	require.Equal(suite.T(), suite.chainA.SenderAccount.GetAddress().String(), ownerA)
}
