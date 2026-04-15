# 🛡️ HomeChain V2: The EVM-Native Rust Genesis (Parity 2.0)

---

### 🚀 Production Status
- **Mainnet Explorer**: [https://homechain-explorer.vercel.app](https://homechain-explorer.vercel.app)
- **Primary Node RPC**: `http://rpc.homechain.online`
- **Network Sync**: **Healthy | Operational**
- **Dashboard UI**: **BSCScan Parity 2.0 (High Density Enabled)**

---

HomeChain V2 is a high-performance Layer 1 blockchain architecture rebuilt from the ground up in Rust. It combines the classic **Proof-of-Work (PoW)** consensus with full **EVM (Ethereum Virtual Machine)** compatibility, allowing seamless integration with tools like Metamask and the broader Web3 ecosystem.

## 🚀 Key Features

- **EVM-Native State**: Real-time balance and nonce management compliant with Ecrecover and EIP-1559 address derivation.
- **Hybrid Consensus**: Uses the robust Keccak256 algorithm for Proof-of-Work, ensuring a fair and decentralized genesis.
- **Zero-Allocation Miner**: A highly optimized Rust miner designed to saturate CPU pipelines with literal zero-allocation hashing loops.
- **Metamask RPC Gateway**: Built-in JSON-RPC demultiplexer (Port 5005) supporting standard `eth_` methods.

## 🏗️ Project Structure (Crate-based Architecture)

The workspace is divided into four specialized crates:

1.  **`home-core`**: The consensus backbone. Defines the Block, Header, and EVM Transaction structures. Implements the Merkle Root logic for header-isolation.
2.  **`home-crypto`**: Cryptographic primitives. Handles Keccak256, SHA256 (for PoW), and EVM address derivation/checksums.
3.  **`home-node`**: The stateful validator. Manages the SQLite ledger, account balances, and the Axum-powered RPC gateway.
4.  **`home-miner`**: The performance engine. A dedicated block-producer that interacts with the Node to secure the chain.

## 🛠️ Getting Started

### Prerequisites
- **Rust Toolchain**: 1.94+ (Stable)
- **Local Dependencies**: `build-essential`, `pkg-config`, `libssl-dev`.

### Building from Source
```bash
# Clone and build the entire workspace in release mode
cargo build --release
```

### Running the Infrastructure
```bash
# 1. Start the Node (on Terminal A)
./target/release/home-node

# 2. Start the Miner (on Terminal B)
./target/release/home-miner
```

## 🛰️ Network Configuration (Web3)

To connect Metamask to your local or remote HomeChain node:

- **RPC URL**: `http://<YOUR_IP>:5005/rpc`
- **Chain ID**: `4919` (0x1337)
- **Currency Symbol**: `HOME`

## ⚖️ License
This project is part of the HomeChain ecosystem. All rights reserved.
