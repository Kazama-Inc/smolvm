// whpprobe prints, as JSON, whether this host can run local machines, the way
// kusa-connect will ask: the Go SDK's CheckLocal over the runtime bundle in
// SMOLMACHINES_RUNTIME_DIR. On Windows that is the WHP probe (WinHvPlatform.dll,
// WHvGetCapability, a one-vCPU partition) with Cause set when it fails. It
// never starts a machine and exits 0 either way.
package main

import (
	"encoding/json"
	"os"

	smol "github.com/smol-machines/smol/sdk/go"
)

func main() {
	_ = json.NewEncoder(os.Stdout).Encode(smol.CheckLocal())
}
