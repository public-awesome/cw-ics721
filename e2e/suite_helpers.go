package e2e_test

import (
	"testing"

	wasmibctesting "github.com/CosmWasm/wasmd/x/wasm/ibctesting"
	"github.com/cosmos/cosmos-sdk/crypto/keys/secp256k1"
	sdk "github.com/cosmos/cosmos-sdk/types"

	wasmd "github.com/CosmWasm/wasmd/app"
	authtypes "github.com/cosmos/cosmos-sdk/x/auth/types"
	minttypes "github.com/cosmos/cosmos-sdk/x/mint/types"
	"github.com/stretchr/testify/require"
)

// Creates and funds a new account for CHAIN. ACCOUNT_NUMBER is the
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

// Same as SendMsgs on the chain type, but sends from a different
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
