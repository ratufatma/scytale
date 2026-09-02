package bridge

// IPCBridge handles message framing and exchange with the Rust core daemon.
type IPCBridge struct {
	SocketPath string
}
