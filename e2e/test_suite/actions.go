package test_suite

import (
	"encoding/json"
	"fmt"
	"testing"

	wasmtypes "github.com/CosmWasm/wasmd/x/wasm/types"

	"github.com/stretchr/testify/require"

	wasmibctesting "github.com/CosmWasm/wasmd/x/wasm/ibctesting"
	"github.com/cosmos/cosmos-sdk/crypto/keys/secp256k1"
	sdk "github.com/cosmos/cosmos-sdk/types"

	wasmd "github.com/CosmWasm/wasmd/app"
	authtypes "github.com/cosmos/cosmos-sdk/x/auth/types"
	minttypes "github.com/cosmos/cosmos-sdk/x/mint/types"
)

func StoreCodes(t *testing.T, chain *wasmibctesting.TestChain, bridge *sdk.AccAddress) {
	resp := chain.StoreCodeFile("../artifacts/ics721_base.wasm")
	require.Equal(t, uint64(1), resp.CodeID)

	resp = chain.StoreCodeFile("../external-wasms/cw721_base_v0.18.0.wasm")
	require.Equal(t, uint64(2), resp.CodeID)

	resp = chain.StoreCodeFile("../artifacts/ics721_base_tester.wasm")
	require.Equal(t, uint64(3), resp.CodeID)

	instantiateBridge := InstantiateICS721Bridge{
		CW721CodeID:   2,
		OutgoingProxy: nil,
		IncomingProxy: nil,
		Pauser:        nil,
	}
	instantiateBridgeRaw, err := json.Marshal(instantiateBridge)
	require.NoError(t, err)

	*bridge = chain.InstantiateContract(1, instantiateBridgeRaw)
}

// InstantiateBridge Instantiates a ICS721 contract on CHAIN. Returns the address of the
// instantiated contract.
func InstantiateBridge(t *testing.T, chain *wasmibctesting.TestChain) sdk.AccAddress {
	// Store the contracts.
	bridgeresp := chain.StoreCodeFile("../artifacts/ics721_base.wasm")
	cw721resp := chain.StoreCodeFile("../external-wasms/cw721_base_v0.18.0.wasm")

	// Instantiate the ICS721 contract.
	instantiateICS721 := InstantiateICS721Bridge{
		CW721CodeID:   cw721resp.CodeID,
		OutgoingProxy: nil,
		IncomingProxy: nil,
		Pauser:        nil,
	}
	instantiateICS721Raw, err := json.Marshal(instantiateICS721)
	require.NoError(t, err)
	return chain.InstantiateContract(bridgeresp.CodeID, instantiateICS721Raw)
}

func InstantiateCw721(t *testing.T, chain *wasmibctesting.TestChain, version int) sdk.AccAddress {
	versionStr := fmt.Sprintf("v0.%d.0", version)
	cw721resp := chain.StoreCodeFile(fmt.Sprintf("../external-wasms/cw721_base_%s.wasm", versionStr))

	var instantiateRaw []byte
	var err error

	// Since v0.18.0, the cw721 contract has an additional field in the instantiate message.
	if version > 17 {
		cw721InstantiateV18 := InstantiateCw721v18{
			Name:   "bad/kids",
			Symbol: "bad/kids",
			Minter: chain.SenderAccount.GetAddress().String(), // withdraw_address
		}
		instantiateRaw, err = json.Marshal(cw721InstantiateV18)
	} else {
		cw721InstantiateV16 := InstantiateCw721v16{
			Name:   "bad/kids",
			Symbol: "bad/kids",
			Minter: chain.SenderAccount.GetAddress().String(),
		}
		instantiateRaw, err = json.Marshal(cw721InstantiateV16)
	}

	require.NoError(t, err)
	return chain.InstantiateContract(cw721resp.CodeID, instantiateRaw)
}

// MigrateWIthUpdate Migrates the ICS721 contract on CHAIN to use a different CW721 contract code ID
func MigrateWIthUpdate(t *testing.T, chain *wasmibctesting.TestChain, ics721 string, codeId, cw721BaseCodeId uint64) {
	_, err := chain.SendMsgs(&wasmtypes.MsgMigrateContract{
		// Sender account is the minter in our test universe.
		Sender:   chain.SenderAccount.GetAddress().String(),
		Contract: ics721,
		CodeID:   codeId,
		Msg:      []byte(fmt.Sprintf(`{ "with_update": { "cw721_base_code_id": %d } }`, cw721BaseCodeId)),
	})
	require.NoError(t, err)
}

func MintNFT(t *testing.T, chain *wasmibctesting.TestChain, cw721 string, id string, receiver sdk.AccAddress) {
	_, err := chain.SendMsgs(&wasmtypes.MsgExecuteContract{
		// Sender account is the minter in our test universe.
		Sender:   chain.SenderAccount.GetAddress().String(),
		Contract: cw721,
		Msg:      []byte(fmt.Sprintf(`{ "mint": { "token_id": "%s", "owner": "%s" } }`, id, receiver.String())),
		Funds:    []sdk.Coin{},
	})
	require.NoError(t, err)
}

func TransferNft(t *testing.T, chain *wasmibctesting.TestChain, nftContract string, tokenId string, sender, receiver sdk.AccAddress) {
	_, err := chain.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   sender.String(),
		Contract: nftContract,
		Msg:      []byte(fmt.Sprintf(`{"transfer_nft": { "recipient": "%s", "token_id": "%s" }}`, receiver, tokenId)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(t, err)
}

func Ics721TransferNft(t *testing.T, chain *wasmibctesting.TestChain, path *wasmibctesting.Path, coordinator *wasmibctesting.Coordinator, nftContract string, tokenId string, bridge, sender, receiver sdk.AccAddress, memo string) *sdk.Result { // Send the NFT away.
	res, err := chain.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   sender.String(),
		Contract: nftContract,
		Msg:      []byte(GetCw721SendIbcAwayMessage(path, coordinator, tokenId, bridge, receiver, coordinator.CurrentTime.UnixNano()+1000000000000, memo)),
		Funds:    []sdk.Coin{},
	})
	require.NoError(t, err)
	coordinator.RelayAndAckPendingPackets(path)
	return res
}

func SendIcsFromChainToChain(t *testing.T, coordinator *wasmibctesting.Coordinator, sourceChain *wasmibctesting.TestChain, sourceBridge sdk.AccAddress, sourceTester sdk.AccAddress, destinationTester sdk.AccAddress, path *wasmibctesting.Path, endpoint *wasmibctesting.Endpoint, nftContract string, tokenId string, memo string, relay bool) {
	msg := fmt.Sprintf(`{ "send_nft": {"cw721": "%s", "ics721": "%s", "token_id": "%s", "recipient":"%s", "channel_id":"%s", "memo":"%s"}}`, nftContract, sourceBridge.String(), tokenId, destinationTester.String(), endpoint.ChannelID, memo)
	_, err := sourceChain.SendMsgs(&wasmtypes.MsgExecuteContract{
		Sender:   sourceChain.SenderAccount.GetAddress().String(),
		Contract: sourceTester.String(),
		Msg:      []byte(msg),
		Funds:    []sdk.Coin{},
	})
	require.NoError(t, err)

	if relay {
		coordinator.UpdateTime()
		coordinator.RelayAndAckPendingPackets(path)
		coordinator.UpdateTime()
	}
}

// CreateAndFundAccount Creates and funds a new account for CHAIN. ACCOUNT_NUMBER is the
// number of accounts that have been previously created on CHAIN.
func CreateAndFundAccount(t *testing.T, chain *wasmibctesting.TestChain, accountNumber uint64) Account {
	privkey := secp256k1.GenPrivKey()
	pubkey := privkey.PubKey()
	addr := sdk.AccAddress(pubkey.Address())

	bondDenom := chain.App.StakingKeeper.BondDenom(chain.GetContext())
	coins := sdk.NewCoins(sdk.NewCoin(bondDenom, sdk.NewInt(1000000)))

	// Unclear to me exactly why we need to mint coins into this
	// "mint" module and then transfer. Why can't we just mint
	// directly to an address?
	err := chain.App.BankKeeper.MintCoins(chain.GetContext(), minttypes.ModuleName, coins)
	require.NoError(t, err)

	err = chain.App.BankKeeper.SendCoinsFromModuleToAccount(chain.GetContext(), minttypes.ModuleName, addr, coins)
	require.NoError(t, err)

	baseAcc := authtypes.NewBaseAccount(addr, pubkey, accountNumber, 0)

	return Account{PrivKey: privkey, PubKey: pubkey, Address: addr, Acc: baseAcc}
}

// SendMsgsFromAccount Same as SendMsgs on the chain type, but sends from a different
// account than the sender account.
func SendMsgsFromAccount(t *testing.T, chain *wasmibctesting.TestChain, account Account, msgs ...sdk.Msg) (*sdk.Result, error) {
	chain.Coordinator.UpdateTimeForChain(chain)

	_, r, err := wasmd.SignAndDeliver(
		t,
		chain.TxConfig,
		chain.App.BaseApp,
		chain.GetContext().BlockHeader(),
		msgs,
		chain.ChainID,
		[]uint64{account.Acc.GetAccountNumber()},
		[]uint64{account.Acc.GetSequence()},
		account.PrivKey,
	)
	if err != nil {
		return nil, err
	}

	// SignAndDeliver calls app.Commit()
	chain.NextBlock()

	// increment sequence for successful transaction execution
	err = account.Acc.SetSequence(account.Acc.GetSequence() + 1)
	if err != nil {
		return nil, err
	}

	chain.Coordinator.IncrementTime()
	chain.CaptureIBCEvents(r)

	return r, nil
}
