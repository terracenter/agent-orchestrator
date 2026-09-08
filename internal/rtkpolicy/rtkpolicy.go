// Package rtkpolicy is the single source of truth for the list of binaries
// that require the rtk wrapper (issue #180). The list is embedded at compile
// time from rtk_required.json, the same physical file that orq-agent (Rust)
// embeds via include_str! in orq-agent/src/compliance.rs — adding a binary
// there updates both auditors after a rebuild, without touching either
// language's source.
package rtkpolicy

import (
	_ "embed"
	"encoding/json"
	"fmt"
	"sort"
)

//go:embed rtk_required.json
var rawJSON []byte

const supportedSchemaVersion = 1

type rtkRequiredConfig struct {
	SchemaVersion int      `json:"schema_version"`
	Binaries      []string `json:"binaries"`
}

var binaries []string

func init() {
	var cfg rtkRequiredConfig
	if err := json.Unmarshal(rawJSON, &cfg); err != nil {
		panic(fmt.Sprintf("rtkpolicy: parsing embedded rtk_required.json: %v", err))
	}
	if cfg.SchemaVersion != supportedSchemaVersion {
		panic(fmt.Sprintf("rtkpolicy: unsupported schema_version %d in rtk_required.json; expected %d", cfg.SchemaVersion, supportedSchemaVersion))
	}
	if len(cfg.Binaries) == 0 {
		panic("rtkpolicy: embedded rtk_required.json defines no binaries")
	}

	sorted := make([]string, len(cfg.Binaries))
	copy(sorted, cfg.Binaries)
	sort.Strings(sorted)
	binaries = sorted
}

// Binaries returns the canonical, sorted list of binaries that require the
// rtk wrapper. It returns a defensive copy so callers cannot mutate the
// shared, embedded list.
func Binaries() []string {
	out := make([]string, len(binaries))
	copy(out, binaries)
	return out
}
