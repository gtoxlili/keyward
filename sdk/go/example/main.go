// Minimal Orchestrator using the Go SDK. Run it, then dial a Rust Executor in:
//
//	go run ./example
//	# then, in another shell:
//	KEYWARD_ORCH_URL=ws://127.0.0.1:8800 KEYWARD_PAIRING_TOKEN=pt_go keyward executor
//
// Set PROVIDER=openai (and build the executor with --features openai + a key) for a
// real call.
package main

import (
	"encoding/json"
	"fmt"
	"os"

	keyward "github.com/gtoxlili/keyward/sdk/go"
)

func main() {
	cfg := keyward.NewConfig("sdk-go-example", "orch_go", "pt_go")
	fmt.Printf("orchestrator on ws://127.0.0.1:8800  (root fingerprint %s)\n", cfg.RootFingerprint())
	fmt.Println("dial an executor:  KEYWARD_ORCH_URL=ws://127.0.0.1:8800 KEYWARD_PAIRING_TOKEN=pt_go keyward executor")

	session, err := keyward.ServeOne("127.0.0.1:8800", cfg)
	if err != nil {
		panic(err)
	}
	fmt.Println("executor paired — sending a work intent…")

	provider := os.Getenv("PROVIDER")
	if provider == "" {
		provider = "mock"
	}
	req := json.RawMessage(`{"model":"gpt-4o","messages":[{"role":"user","content":"Say hello to the Keyward Go SDK."}],"stream":true}`)
	events, err := session.Submit(provider, req)
	if err != nil {
		panic(err)
	}

	var text string
	for ev := range events {
		switch ev.Kind {
		case keyward.Chunk:
			var c struct {
				Choices []struct {
					Delta struct {
						Content string `json:"content"`
					} `json:"delta"`
				} `json:"choices"`
			}
			if json.Unmarshal(ev.Delta, &c) == nil && len(c.Choices) > 0 {
				text += c.Choices[0].Delta.Content
			}
		case keyward.Done:
			fmt.Printf("\nassembled: %q\nusage in=%d out=%d\n", text, ev.Usage.InputTokens, ev.Usage.OutputTokens)
		case keyward.Error:
			fmt.Println("error:", ev.Err)
		}
	}
}
