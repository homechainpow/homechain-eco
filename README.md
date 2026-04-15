# HomeChain (HOME) - Layer 1 Ecosystem

![HomeChain Banner](https://img.shields.io/badge/Mainnet-Active-success?style=for-the-badge&logo=blockchaindotcom)
![License](https://img.shields.io/badge/License-MIT-blue?style=for-the-badge)
![Status](https://img.shields.io/badge/Parity-BSCScan%203.0-orange?style=for-the-badge)

Welcome to the official monorepo of the **HomeChain Layer 1**. This repository houses the complete infrastructure, from the core blockchain engine to the high-density block explorer.

## 🌌 Overview

HomeChain is a high-performance, EVM-compatible blockchain designed for elite stability and extreme decentralization. It utilizes a hybrid consensus model to ensure sub-second block times while maintaining the security of a distributed network.

### Key Features
- **🚀 Optimized Blocks**: Stable 15s heart-beat maintained by DDA.
- **🛡️ Enterprise Staking**: Native Validator support.
- **💎 Parity Explorer**: 1:1 functional and visual parity with industry leaders like BSCScan.
- **🔗 Cross-Chain Ready**: Built-in support for HRC-20 and HRC-721 token standards.

### 🌐 Network Connection (MetaMask)

Use these settings to add HomeChain to your wallet:

| Parameter | Value |
| :--- | :--- |
| **Network Name** | HomeChain |
| **RPC URL** | `https://rpc.homechain.online/rpc` |
| **Chain ID** | `4919` |
| **Currency Symbol**| `HOME` |
| **Block Explorer** | [https://explorer.homechain.online](https://explorer.homechain.online) |

### ⚙️ DDA Engine: The Heartbeat (15s Target)

HomeChain's **Dynamic Difficulty Adjustment (DDA)** engine ensures network stability by evaluating every single block:
- **Target Block Time**: 15 Seconds.
- **Adjustment Window**: Proportional scaling per block (Real-time).

### 💎 Economics & Halving Schedule

HomeChain utilize a **Geometric Scaling Halving** mechanism designed for long-term scarcity:

| Era | Block Range | Est. Duration | Reward (HOME) | Era Total Supply |
| :--- | :--- | :--- | :--- | :--- |
| **Era 1** | 1 - 57,600 | 10 Days | **2,500** | 144,000,000 |
| **Era 2** | 57,601 - 172,800 | 20 Days | **1,250** | 144,000,000 |
| **Era 3** | 172,801 - 403,200 | 40 Hari | **625** | 144,000,000 |
| **Era 4** | 403,201 - 864,000 | 80 Hari | **312.5** | 144,000,000 |
| **Era 5** | 864,001 - 1,785,600 | 160 Hari | **156.25** | 144,000,000 |

- **Total Supply Cap**: 21,000,000,000 $HOME.
- **Minimum Tx Fee**: 0.01 $HOME.
- **Precision**: 18 Decimals.

---

## ⛏️ Mining Quickstart

HomeChain is a CPU-friendly PoW blockchain:

1. **Clone & Build**:
   ```bash
   git clone https://github.com/homechainpow/homechain-eco.git
   cd homechain-eco
   cargo build --release
   ```
2. **Run Miner**:
   ```bash
   ./target/release/home-miner --rpc https://rpc.homechain.online/rpc --address YOUR_WALLET_ADDRESS
   ```

---

## 📁 Repository Map

| Component | Path | Description |
| :--- | :--- | :--- |
| **Blockchain Core** | [`/core`](./core) | Rust node and cryptographic primitives. |
| **Block Explorer**| [`/explorer`](./explorer) | Enterprise-grade frontend. |
| **Documentation** | [`/docs`](./docs) | Multi-lingual whitepapers. |

---

## 📜 Documentation

- 🇬🇧 [**Whitepaper (English)**](./docs/whitepaper_v2_en.md)
- 🇨🇳 [**Whitepaper (Chinese)**](./docs/whitepaper_v2_cn.md)
- 🇷🇺 [**Whitepaper (Russian)**](./docs/whitepaper_v2_ru.md)
- 🇪🇸 [**Whitepaper (Spanish)**](./docs/whitepaper_v2_es.md)
- 🇫🇷 [**Whitepaper (French)**](./docs/whitepaper_v2_fr.md)
- 🇮🇩 [**Whitepaper (Indonesian)**](./docs/whitepaper_v2_id.md)
- 🇻🇳 [**Whitepaper (Vietnamese)**](./docs/whitepaper_v2_vn.md)

---

## 🛡️ License

This project is licensed under the **MIT License**.

---
© 2026 HomeChain Foundation.
