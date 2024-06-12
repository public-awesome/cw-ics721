package e2e

import (
	wasmd "github.com/CosmWasm/wasmd/app"
)

func init() {
	// override default gas
	wasmd.DefaultGas = 3_000_000
}
