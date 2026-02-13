# VEIL Android UI

Flutter-based social feed application designed as a thin client for the 
[VEIL Android Node](../README.md).

## Features

- **Social Feed**: Chronological display of reconstructed posts with media support.
- **Identity Management**: Automated key generation and profile publishing.
- **Multi-Lane Connectivity**: Visual indicators for QUIC, WebSocket, and Tor lane health.
- **Web-of-Trust**: Local trust tiering (Trusted, Known, Unknown, Muted, Blocked).
- **Secure Messaging**: E2E encrypted direct and group messaging.

## Getting Started

This UI communicates with the local node process via localhost HTTP and WebSocket. 
It **does not** use the VEIL SDK directly for protocol logic; it relies on the 
node's RPC interface.

### Running

To run the UI in development mode against a connected device or emulator:

```bash
cd apps/android-node/src/ui
flutter run
```

*Note: Ensure the background node service is running, or start the node binary 
manually if testing UI components in isolation.*

## Architecture

- **Logic Layer**: `lib/logic` contains controllers for social state, messaging, 
  and node service communication.
- **UI Layer**: `lib/ui` contains screens and widgets following the VEIL design 
  system (Glassmorphism, high-contrast dark theme).
- **Native Integration**: `android/` contains the Kotlin service wrapper that 
  orchestrates the native Rust node.
