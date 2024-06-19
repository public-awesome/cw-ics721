package test_suite

import (
	"encoding/hex"
	"fmt"

	b64 "encoding/base64"

	wasmibctesting "github.com/CosmWasm/wasmd/x/wasm/ibctesting"
	sdk "github.com/cosmos/cosmos-sdk/types"
)

func CreateCallbackMemo(srcCallback, srcReceiver, dstCallback, dstReceiver string) string {
	srcCallback = ParseOptional(srcCallback)
	srcReceiver = ParseOptional(srcReceiver)
	dstCallback = ParseOptional(dstCallback)
	dstReceiver = ParseOptional(dstReceiver)
	// if "receive_callback_addr" is not specified it will be the same as the ics721 contract address
	memo := fmt.Sprintf(`{ "callbacks": { "ack_callback_data": %s, "ack_callback_addr": %s, "receive_callback_data": %s, "receive_callback_addr": %s } }`, srcCallback, srcReceiver, dstCallback, dstReceiver)
	return b64.StdEncoding.EncodeToString([]byte(memo))
}

func NftCallbackSent() string {
	return b64.StdEncoding.EncodeToString([]byte(`{ "nft_sent": {}}`))
}

func NftCallbackReceived() string {
	return b64.StdEncoding.EncodeToString([]byte(`{ "nft_received": {}}`))
}

func NftCallbackFailed() string {
	return b64.StdEncoding.EncodeToString([]byte(`{ "fail_callback": {}}`))
}

func ParseOptional(memo string) string {
	r := ""
	if memo != "" {
		r = fmt.Sprintf("\"%s\"", memo)
	} else {
		r = "null"
	}
	return r
}

func GetCw721SendIbcAwayMessage(path *wasmibctesting.Path, coordinator *wasmibctesting.Coordinator, tokenId string, bridge, receiver sdk.AccAddress, timeout int64, memo string) string {
	memo = ParseOptional(memo)
	ibcAway := fmt.Sprintf(`{ "receiver": "%s", "channel_id": "%s", "timeout": { "timestamp": "%d" }, "memo": %s }`, receiver.String(), path.EndpointA.ChannelID, timeout, memo)
	ibcAwayEncoded := b64.StdEncoding.EncodeToString([]byte(ibcAway))
	return fmt.Sprintf(`{ "send_nft": { "contract": "%s", "token_id": "%s", "msg": "%s" } }`, bridge, tokenId, ibcAwayEncoded)
}

func AccAddressFromHex(address string) (addr sdk.AccAddress, err error) {
	bz, err := addressBytesFromHexString(address)
	return sdk.AccAddress(bz), err
}

func addressBytesFromHexString(address string) ([]byte, error) {
	return hex.DecodeString(address)
}
