// Package keyward is the Go SDK for embedding a Keyward Node in-process — the
// non-custodial BYOK protocol. You bind a listener the Owner's Client dials into,
// pair, then submit work intents and stream the provider's native response back.
// Your code decides the requests; the key stays on the Client and never reaches you.
//
// This is the in-process path. The zero-integration path needs no SDK at all: an
// unaware app just points its OpenAI base URL at a standalone `keyward node`.
//
// The wire/crypto formats here are byte-compatible with the Rust reference Client
// (Ed25519 SSH-CA chain: root signs `op_pubkey || not_after_le`, the operational
// key signs the sid).
package keyward

import (
	"context"
	"crypto/ed25519"
	"crypto/rand"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/http"
	"sync"
	"time"

	"github.com/coder/websocket"
)

// Usage is the provider-reported token usage on a terminal frame.
type Usage struct {
	InputTokens  uint64 `json:"input_tokens"`
	OutputTokens uint64 `json:"output_tokens"`
}

// EventKind discriminates an Event.
type EventKind int

const (
	// Chunk is a native streaming chunk (relay it to your user unchanged).
	Chunk EventKind = iota
	// Done is terminal success, with metered usage.
	Done
	// Error is terminal failure.
	Error
)

// Event is one streamed step of a provider response.
type Event struct {
	Kind  EventKind
	Delta json.RawMessage // Chunk: the native chunk
	Usage Usage           // Done
	Err   string          // Error
}

// Config is the node configuration.
type Config struct {
	Name         string // app name, sent to the Client
	ID           string // stable node id
	PairingToken string // one-time token the Owner pastes into their Client
	Root         ed25519.PrivateKey
	// AuthorizedClients, if non-nil, is an allow-list of Client identity
	// pubkeys (hex); nil accepts any Client that proves possession of its key.
	AuthorizedClients []string
}

// NewConfig builds a Config with a freshly generated root identity.
func NewConfig(name, id, pairingToken string) Config {
	_, priv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		panic(err)
	}
	return Config{Name: name, ID: id, PairingToken: pairingToken, Root: priv}
}

// RootFingerprint is this node's root fingerprint — show it to the Owner
// for out-of-band confirmation when pairing.
func (c Config) RootFingerprint() string {
	return fingerprint(c.Root.Public().(ed25519.PublicKey))
}

// Session is a paired connection with one Client.
type Session struct {
	conn    *websocket.Conn
	sid     string
	mu      sync.Mutex
	pending map[string]chan Event
	writeMu sync.Mutex
}

// ServeOne binds a WebSocket server at addr, accepts ONE Client dialing in,
// authenticates and pairs it, and returns the Session. (v0: one Client.)
func ServeOne(addr string, cfg Config) (*Session, error) {
	ln, err := net.Listen("tcp", addr)
	if err != nil {
		return nil, err
	}
	type result struct {
		s   *Session
		err error
	}
	ch := make(chan result, 1)
	var once sync.Once
	done := func(r result) { once.Do(func() { ch <- r }) }

	handler := func(w http.ResponseWriter, r *http.Request) {
		c, err := websocket.Accept(w, r, nil)
		if err != nil {
			done(result{nil, err})
			return
		}
		ctx := context.Background()
		hello, err := readFrame(ctx, c)
		if err != nil {
			done(result{nil, err})
			return
		}
		if err := authenticateClient(hello, cfg); err != nil {
			_ = writeFrame(ctx, c, map[string]any{"kw": "0", "mid": newMid(), "type": "error", "code": "bad_request", "message": err.Error()})
			c.Close(websocket.StatusPolicyViolation, "auth failed")
			done(result{nil, err})
			return
		}
		sid, paired := buildPaired(cfg)
		if err := writeFrame(ctx, c, paired); err != nil {
			done(result{nil, err})
			return
		}
		s := &Session{conn: c, sid: sid, pending: map[string]chan Event{}}
		done(result{s, nil})
		s.recvLoop(ctx) // blocks until the channel drops or Close()
		c.Close(websocket.StatusNormalClosure, "")
	}
	srv := &http.Server{Handler: http.HandlerFunc(handler)}
	go func() { _ = srv.Serve(ln) }()

	res := <-ch
	return res.s, res.err
}

// Submit sends a work intent — a provider name and the provider-native request body
// (minus any credential) — and returns a channel of native Events.
func (s *Session) Submit(provider string, request json.RawMessage) (<-chan Event, error) {
	mid := newMid()
	out := make(chan Event, 64)
	s.mu.Lock()
	s.pending[mid] = out
	s.mu.Unlock()
	frame := map[string]any{
		"kw": "0", "sid": s.sid, "mid": mid, "type": "work",
		"provider": provider, "request": request,
	}
	s.writeMu.Lock()
	err := writeFrame(context.Background(), s.conn, frame)
	s.writeMu.Unlock()
	if err != nil {
		s.mu.Lock()
		delete(s.pending, mid)
		s.mu.Unlock()
		return nil, err
	}
	return out, nil
}

// Close ends the session and disconnects the Client.
func (s *Session) Close() {
	s.conn.Close(websocket.StatusNormalClosure, "")
}

func (s *Session) recvLoop(ctx context.Context) {
	for {
		f, err := readFrame(ctx, s.conn)
		if err != nil {
			break
		}
		s.route(f)
	}
	s.mu.Lock()
	for _, ch := range s.pending {
		ch <- Event{Kind: Error, Err: "client disconnected"}
		close(ch)
	}
	s.pending = map[string]chan Event{}
	s.mu.Unlock()
}

func (s *Session) route(f frame) {
	var ev Event
	terminal := false
	switch f.Type {
	case "work_chunk":
		ev = Event{Kind: Chunk, Delta: f.Delta}
	case "work_done":
		u := Usage{}
		if f.Usage != nil {
			u = *f.Usage
		}
		ev = Event{Kind: Done, Usage: u}
		terminal = true
	case "work_error":
		ev = Event{Kind: Error, Err: f.Code + ": " + f.Message}
		terminal = true
	default:
		return
	}
	s.mu.Lock()
	ch := s.pending[f.Mid]
	if terminal {
		delete(s.pending, f.Mid)
	}
	s.mu.Unlock()
	if ch != nil {
		ch <- ev
		if terminal {
			close(ch)
		}
	}
}

// --- wire ---

// frame deserializes any incoming Keyward frame (unused fields stay zero).
type frame struct {
	Kw           string          `json:"kw"`
	Sid          string          `json:"sid"`
	Mid          string          `json:"mid"`
	Type         string          `json:"type"`
	PairingToken string          `json:"pairing_token"`
	Pubkey       string          `json:"pubkey"`
	Sig          string          `json:"sig"`
	Seq          uint64          `json:"seq"`
	Delta        json.RawMessage `json:"delta"`
	Usage        *Usage          `json:"usage"`
	Code         string          `json:"code"`
	Message      string          `json:"message"`
}

func readFrame(ctx context.Context, c *websocket.Conn) (frame, error) {
	_, data, err := c.Read(ctx)
	if err != nil {
		return frame{}, err
	}
	var f frame
	return f, json.Unmarshal(data, &f)
}

func writeFrame(ctx context.Context, c *websocket.Conn, v any) error {
	data, err := json.Marshal(v)
	if err != nil {
		return err
	}
	return c.Write(ctx, websocket.MessageText, data)
}

// --- crypto (byte-compatible with the Rust client's verifier) ---

func buildPaired(cfg Config) (string, map[string]any) {
	sid := "kw_sess_" + randHex(4)
	opPub, opPriv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		panic(err)
	}
	notAfter := time.Now().Unix() + 3600
	rootSig := ed25519.Sign(cfg.Root, certMsg(opPub, notAfter))
	sidSig := ed25519.Sign(opPriv, []byte(sid))
	rootPub := cfg.Root.Public().(ed25519.PublicKey)
	frame := map[string]any{
		"kw": "0", "sid": sid, "mid": newMid(), "type": "paired",
		"node": map[string]any{"name": cfg.Name, "id": cfg.ID},
		"root_pubkey":  hex.EncodeToString(rootPub),
		"op": map[string]any{
			"pubkey":    hex.EncodeToString(opPub),
			"not_after": notAfter,
			"root_sig":  hex.EncodeToString(rootSig),
		},
		"sig": hex.EncodeToString(sidSig),
	}
	return sid, frame
}

// certMsg is the canonical delegation message: op_pubkey(32) || not_after(i64 LE).
func certMsg(opPubkey ed25519.PublicKey, notAfter int64) []byte {
	b := make([]byte, 0, 40)
	b = append(b, opPubkey...)
	le := make([]byte, 8)
	binary.LittleEndian.PutUint64(le, uint64(notAfter))
	return append(b, le...)
}

func authenticateClient(f frame, cfg Config) error {
	if f.Type != "hello" {
		return errors.New("expected hello")
	}
	if f.PairingToken != cfg.PairingToken {
		return errors.New("pairing token rejected")
	}
	if f.Pubkey != "" && f.Sig != "" {
		pub, err := hex.DecodeString(f.Pubkey)
		if err != nil || len(pub) != ed25519.PublicKeySize {
			return errors.New("bad client pubkey")
		}
		sig, err := hex.DecodeString(f.Sig)
		if err != nil || len(sig) != ed25519.SignatureSize {
			return errors.New("bad client signature")
		}
		if !ed25519.Verify(ed25519.PublicKey(pub), []byte(cfg.PairingToken), sig) {
			return errors.New("client identity signature invalid")
		}
	} else if cfg.AuthorizedClients != nil {
		return errors.New("client identity required but not provided")
	}
	if cfg.AuthorizedClients != nil {
		ok := false
		for _, a := range cfg.AuthorizedClients {
			if a == f.Pubkey {
				ok = true
			}
		}
		if !ok {
			return errors.New("client not authorized")
		}
	}
	return nil
}

func fingerprint(pubkey []byte) string {
	h := hex.EncodeToString(pubkey)
	return fmt.Sprintf("%s-%s-%s-%s", h[0:4], h[4:8], h[8:12], h[12:16])
}

func newMid() string { return randHex(16) }

func randHex(n int) string {
	b := make([]byte, n)
	if _, err := rand.Read(b); err != nil {
		panic(err)
	}
	return hex.EncodeToString(b)
}
