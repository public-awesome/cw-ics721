package e2e_test

import (
	"strconv"
	"strings"
	"testing"

	wasmd "github.com/CosmWasm/wasmd/app"
	wasmibctesting "github.com/CosmWasm/wasmd/x/wasm/ibctesting"
	"github.com/cosmos/cosmos-sdk/crypto/keys/secp256k1"
	sdk "github.com/cosmos/cosmos-sdk/types"

	authtypes "github.com/cosmos/cosmos-sdk/x/auth/types"
	minttypes "github.com/cosmos/cosmos-sdk/x/mint/types"
	clienttypes "github.com/cosmos/ibc-go/v3/modules/core/02-client/types"
	channeltypes "github.com/cosmos/ibc-go/v3/modules/core/04-channel/types"
	"github.com/stretchr/testify/require"
	abci "github.com/tendermint/tendermint/abci/types"
)

// Creates and funds a new account for CHAIN. ACCOUNT_NUMBER is the
// number of accounts that have been previously created on CHAIN.
func CreateAndFundAccount(t *testing.T, chain *wasmibctesting.TestChain, accountNumber uint64) Account {
	privkey := secp256k1.GenPrivKey()
	pubkey := privkey.PubKey()
	addr := sdk.AccAddress(pubkey.Address())

	testSupport := chain.GetTestSupport()

	bondDenom := testSupport.StakingKeeper().BondDenom(chain.GetContext())
	coins := sdk.NewCoins(sdk.NewCoin(bondDenom, sdk.NewInt(1000000)))

	// Unclear to me exactly why we need to mint coins into this
	// "mint" module and then transfer. Why can't we just mint
	// directly to an address?
	err := testSupport.BankKeeper().MintCoins(chain.GetContext(), minttypes.ModuleName, coins)
	require.NoError(t, err)

	err = testSupport.BankKeeper().SendCoinsFromModuleToAccount(chain.GetContext(), minttypes.ModuleName, addr, coins)
	require.NoError(t, err)

	baseAcc := authtypes.NewBaseAccount(addr, pubkey, accountNumber, 0)

	return Account{PrivKey: privkey, PubKey: pubkey, Address: addr, Acc: baseAcc}
}

// Same as SendMsgs on the chain type, but sends from a different
// account than the sender account.
func SendMsgsFromAccount(t *testing.T, chain *wasmibctesting.TestChain, account Account, shouldPass bool, msgs ...sdk.Msg) (*sdk.Result, error) {
	chain.Coordinator.UpdateTimeForChain(chain)

	_, r, err := wasmd.SignAndDeliver(
		t,
		chain.TxConfig,
		chain.App.GetBaseApp(),
		chain.GetContext().BlockHeader(),
		msgs,
		chain.ChainID,
		[]uint64{account.Acc.GetAccountNumber()},
		[]uint64{account.Acc.GetSequence()},
		shouldPass, shouldPass, account.PrivKey,
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

	captureIBCEvents(chain, r)

	return r, nil
}

// -------------------------------------------------------------------
// All of this is copied from the wasmibctesting package as we need
// these methods to create the `SendMsgsFromAccount` method but they
// are non-public.
// -------------------------------------------------------------------

func captureIBCEvents(chain *wasmibctesting.TestChain, r *sdk.Result) {
	toSend := getSendPackets(r.Events)
	if len(toSend) > 0 {
		// Keep a queue on the chain that we can relay in tests
		chain.PendingSendPackets = append(chain.PendingSendPackets, toSend...)
	}
	toAck := getAckPackets(r.Events)
	if len(toAck) > 0 {
		// Keep a queue on the chain that we can relay in tests
		chain.PendingAckPackets = append(chain.PendingAckPackets, toAck...)
	}
}

func getSendPackets(evts []abci.Event) []channeltypes.Packet {
	var res []channeltypes.Packet
	for _, evt := range evts {
		if evt.Type == "send_packet" {
			packet := parsePacketFromEvent(evt)
			res = append(res, packet)
		}
	}
	return res
}

func getAckPackets(evts []abci.Event) []wasmibctesting.PacketAck {
	var res []wasmibctesting.PacketAck
	for _, evt := range evts {
		if evt.Type == "write_acknowledgement" {
			packet := parsePacketFromEvent(evt)
			ack := wasmibctesting.PacketAck{
				Packet: packet,
				Ack:    []byte(getField(evt, "packet_ack")),
			}
			res = append(res, ack)
		}
	}
	return res
}

func parsePacketFromEvent(evt abci.Event) channeltypes.Packet {
	return channeltypes.Packet{
		Sequence:           getUintField(evt, "packet_sequence"),
		SourcePort:         getField(evt, "packet_src_port"),
		SourceChannel:      getField(evt, "packet_src_channel"),
		DestinationPort:    getField(evt, "packet_dst_port"),
		DestinationChannel: getField(evt, "packet_dst_channel"),
		Data:               []byte(getField(evt, "packet_data")),
		TimeoutHeight:      parseTimeoutHeight(getField(evt, "packet_timeout_height")),
		TimeoutTimestamp:   getUintField(evt, "packet_timeout_timestamp"),
	}
}

// return the value for the attribute with the given name
func getField(evt abci.Event, key string) string {
	for _, attr := range evt.Attributes {
		if string(attr.Key) == key {
			return string(attr.Value)
		}
	}
	return ""
}

func getUintField(evt abci.Event, key string) uint64 {
	raw := getField(evt, key)
	return toUint64(raw)
}

func toUint64(raw string) uint64 {
	if raw == "" {
		return 0
	}
	i, err := strconv.ParseUint(raw, 10, 64)
	if err != nil {
		panic(err)
	}
	return i
}

func parseTimeoutHeight(raw string) clienttypes.Height {
	chunks := strings.Split(raw, "-")
	return clienttypes.Height{
		RevisionNumber: toUint64(chunks[0]),
		RevisionHeight: toUint64(chunks[1]),
	}
}
