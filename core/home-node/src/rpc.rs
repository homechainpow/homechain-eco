use crate::{SharedState, rpc_schema::*};
use axum::{extract::State, Json};
use ethers_core::types::Transaction as EthTransaction;
use ethers_core::utils::rlp::Decodable;
use home_core::U256;
use serde_json::{json, Value};

pub async fn rpc_handler(
    State(state): State<SharedState>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    if let Some(arr) = payload.as_array() {
        // Handle Batch Request
        let mut responses = Vec::new();
        for req in arr {
            if let Ok(rpc_req) = serde_json::from_value::<RpcRequest>(req.clone()) {
                responses.push(process_request(&state, rpc_req).await);
            }
        }
        Json(json!(responses))
    } else if let Ok(rpc_req) = serde_json::from_value::<RpcRequest>(payload) {
        // Handle Single Request
        let res = process_request(&state, rpc_req).await;
        Json(serde_json::to_value(res).unwrap())
    } else {
        Json(json!({"jsonrpc": "2.0", "error": {"code": -32600, "message": "Invalid Request"}}))
    }
}

async fn process_request(state: &SharedState, req: RpcRequest) -> RpcResponse {
    let method = req.method.as_str();
    
    match method {
        "eth_chainId" => {
            // Return hex Chain ID 0x1337 (4919)
            RpcResponse::success(req.id, json!("0x1337"))
        }
        "eth_blockNumber" => {
            let s = state.read().unwrap();
            let len = s.chain.last().map(|b| b.header.index).unwrap_or(0);
            RpcResponse::success(req.id, json!(format!("0x{:x}", len)))
        }
        "eth_getBalance" => {
            let params = req.params.as_array().unwrap();
            let address = params.get(0).unwrap().as_str().unwrap().to_lowercase();
            let s = state.read().unwrap();
            let eth_balance = s.balances.get(&address).cloned().unwrap_or(U256::zero());
            
            // Directly return native U256 balance (already 18 decimals)
            RpcResponse::success(req.id, json!(format!("0x{:x}", eth_balance)))
        }
        "eth_getTransactionCount" => {
            let params = req.params.as_array().unwrap();
            let address = params.get(0).unwrap().as_str().unwrap().to_lowercase();
            let tag = params.get(1).and_then(|v| v.as_str()).unwrap_or("latest");
            let s = state.read().unwrap();
            let confirmed_nonce = s.account_nonces.get(&address).cloned().unwrap_or(0);
            let nonce = if tag == "pending" {
                // Return pending nonce if higher than confirmed
                s.pending_nonces.get(&address).cloned().unwrap_or(confirmed_nonce)
            } else {
                confirmed_nonce
            };
            RpcResponse::success(req.id, json!(format!("0x{:x}", nonce)))
        }
        "eth_sendRawTransaction" => {
            let params = req.params.as_array().unwrap();
            let raw_tx_hex = params.get(0).unwrap().as_str().unwrap();
            let raw_tx_bytes = hex::decode(raw_tx_hex.trim_start_matches("0x")).unwrap();
            
            // Decodes Type 0, 1, and 2 safely
            match ethers_core::utils::rlp::decode::<EthTransaction>(&raw_tx_bytes) {
                Ok(tx) => {
                    let mut s = state.write().unwrap();
                    let hash = tx.hash();
                    let tx_hash = format!("0x{}", hex::encode(hash.as_bytes()));
                    
                    // Track pending nonce for this sender (ECDSA recovery)
                    if let Ok(sender) = tx.recover_from() {
                        let sender_addr = format!("0x{}", hex::encode(sender.as_bytes())).to_lowercase();
                        let next_nonce = tx.nonce.as_u64() + 1;
                        let current_pending = s.pending_nonces.get(&sender_addr).cloned().unwrap_or(0);
                        if next_nonce > current_pending {
                            s.pending_nonces.insert(sender_addr, next_nonce);
                        }
                    }
                    
                    s.pending_transactions.push(tx);
                    RpcResponse::success(req.id, json!(tx_hash))
                }
                Err(e) => {
                    RpcResponse::error(req.id, -32000, &format!("RLP Decode error: {:?}", e))
                }
            }
        }
        "eth_estimateGas" => {
            let params = req.params.as_array().unwrap();
            let call_obj = params.get(0).unwrap().as_object().unwrap();
            let to = call_obj.get("to").and_then(|t| t.as_str());
            let from = call_obj.get("from").and_then(|f| f.as_str());
            let data_str = call_obj.get("data").and_then(|d| d.as_str()).unwrap_or("0x");
            let data = hex::decode(data_str.trim_start_matches("0x")).unwrap_or_default();
            let value_str = call_obj.get("value").and_then(|v| v.as_str()).unwrap_or("0x0");
            let value = U256::from_str_radix(value_str.trim_start_matches("0x"), 16).unwrap_or_default();
            
            let s = state.read().unwrap();
            let block_idx = s.chain.last().map(|b| b.header.index).unwrap_or(0);
            
            let gas = crate::evm::estimate_gas(&s.storage, from, to, &data, value, block_idx);
            RpcResponse::success(req.id, json!(format!("0x{:x}", gas)))
        }
        "eth_gasPrice" => {
            // 1 Wei Minimum
            RpcResponse::success(req.id, json!("0x1"))
        }
        "eth_getTransactionByHash" => {
            let params = req.params.as_array().unwrap();
            let hash_str = params.get(0).unwrap().as_str().unwrap().to_lowercase();
            let s = state.read().unwrap();
            // First: check confirmed transactions in SQLite
            match s.storage.get_transaction_by_hash(&hash_str) {
                Ok(Some(tx_data)) => RpcResponse::success(req.id, tx_data),
                _ => {
                    // Fallback: check pending transactions in mempool
                    let pending_match = s.pending_transactions.iter().find(|tx| {
                        let h = format!("0x{}", hex::encode(tx.hash().as_bytes()));
                        h == hash_str
                    });
                    match pending_match {
                        Some(tx) => {
                            let from_addr = match tx.recover_from() {
                                Ok(s) => format!("0x{}", hex::encode(s.as_bytes())).to_lowercase(),
                                Err(_) => "0x0000000000000000000000000000000000000000".to_string(),
                            };
                            let to_addr = match tx.to {
                                Some(to) => format!("0x{}", hex::encode(to.as_bytes())).to_lowercase(),
                                None => "0x0000000000000000000000000000000000000000".to_string(),
                            };
                            let pending_tx = json!({
                                "hash": hash_str,
                                "blockHash": null,
                                "blockNumber": null,
                                "from": from_addr,
                                "to": to_addr,
                                "value": format!("0x{:x}", tx.value),
                                "gasPrice": format!("0x{:x}", tx.gas_price.unwrap_or(U256::from(1))),
                                "gas": format!("0x{:x}", tx.gas),
                                "nonce": format!("0x{:x}", tx.nonce),
                                "input": "0x",
                                "transactionIndex": null,
                                "type": "0x0"
                            });
                            RpcResponse::success(req.id, pending_tx)
                        }
                        None => RpcResponse::success(req.id, Value::Null),
                    }
                }
            }
        }
        "eth_getTransactionReceipt" => {
            let params = req.params.as_array().unwrap();
            let hash_str = params.get(0).unwrap().as_str().unwrap().to_lowercase();
            let s = state.read().unwrap();
            match s.storage.get_transaction_by_hash(&hash_str) {
                Ok(Some(tx_data)) => {
                    let receipt = json!({
                        "transactionHash": hash_str,
                        "blockHash": tx_data["blockHash"],
                        "blockNumber": tx_data["blockNumber"],
                        "contractAddress": null,
                        "cumulativeGasUsed": "0x5208", // 21000
                        "from": tx_data["from"],
                        "to": tx_data["to"],
                        "gasUsed": "0x5208",
                        "logs": [],
                        "logsBloom": "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                        "status": "0x1",
                        "transactionIndex": "0x0",
                        "type": "0x0"
                    });
                    let receipt = if let Ok(Some(evm_rec)) = s.storage.get_receipt(&hash_str) {
                        let mut r = receipt;
                        let status_str = evm_rec["status"].as_str().unwrap_or("0x0");
                        r["status"] = json!(if status_str == "0x1" { "0x1" } else { "0x0" });
                        r["gasUsed"] = evm_rec["gasUsed"].clone();
                        r["cumulativeGasUsed"] = evm_rec["cumulativeGasUsed"].clone();
                        r["contractAddress"] = evm_rec["contractAddress"].clone();
                        if let Some(bloom) = evm_rec.get("logsBloom") {
                            r["logsBloom"] = bloom.clone();
                        }
                        
                        if let Ok(logs) = s.storage.get_event_logs(0, u64::MAX, None, None) {
                            let mut formatted_logs = Vec::new();
                            for log in logs {
                                if log["transactionHash"].as_str().unwrap() == hash_str {
                                    let mut l_clone = log.clone();
                                    l_clone["blockNumber"] = r["blockNumber"].clone();
                                    l_clone["transactionIndex"] = r["transactionIndex"].clone();
                                    l_clone["blockHash"] = r["blockHash"].clone();
                                    formatted_logs.push(l_clone);
                                }
                            }
                            r["logs"] = json!(formatted_logs);
                        }
                        r
                    } else { receipt };
                    RpcResponse::success(req.id, receipt)
                }
                _ => RpcResponse::success(req.id, Value::Null),
            }
        }
        "eth_getBlockByNumber" => {
            let params = req.params.as_array().unwrap();
            let block_param = params.get(0).unwrap().as_str().unwrap();
            let s = state.read().unwrap();
            
            let block = if block_param == "latest" {
                s.chain.last()
            } else {
                let idx = u64::from_str_radix(block_param.trim_start_matches("0x"), 16).unwrap_or(0);
                s.chain.iter().find(|b| b.header.index == idx)
            };

            match block {
                Some(b) => {
                    let evm_block = EvmBlock::from_home_block(b);
                    RpcResponse::success(req.id, serde_json::to_value(evm_block).unwrap())
                }
                None => RpcResponse::success(req.id, Value::Null)
            }
        }
        "eth_getBlockByHash" => {
            let params = req.params.as_array().unwrap();
            let hash_str = params.get(0).unwrap().as_str().unwrap().to_lowercase();
            let s = state.read().unwrap();
            // Internal block hashes are stored without "0x" prefix
            let clean_hash = hash_str.trim_start_matches("0x");
            let block = s.chain.iter().find(|b| b.hash == clean_hash);
            match block {
                Some(b) => {
                    let evm_block = EvmBlock::from_home_block(b);
                    RpcResponse::success(req.id, serde_json::to_value(evm_block).unwrap())
                }
                None => RpcResponse::success(req.id, Value::Null)
            }
        }
        "eth_getCode" => {
            let params = req.params.as_array().unwrap();
            let addr = params.get(0).unwrap().as_str().unwrap().to_lowercase();
            let s = state.read().unwrap();
            
            match s.storage.get_account(&addr) {
                Ok(Some((_, _, code_hash))) => {
                    let keccak_empty = "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
                    if code_hash != keccak_empty {
                        if let Ok(Some(code)) = s.storage.get_contract_code(&code_hash) {
                            return RpcResponse::success(req.id, json!(format!("0x{}", hex::encode(code))));
                        }
                    }
                    RpcResponse::success(req.id, json!("0x"))
                }
                _ => RpcResponse::success(req.id, json!("0x")),
            }
        }
        "eth_call" => {
            let params = req.params.as_array().unwrap();
            let call_obj = params.get(0).unwrap().as_object().unwrap();
            let to = call_obj.get("to").and_then(|t| t.as_str()).unwrap_or("0x0000000000000000000000000000000000000000");
            let from = call_obj.get("from").and_then(|f| f.as_str());
            let data_str = call_obj.get("data").and_then(|d| d.as_str()).unwrap_or("0x");
            let data = hex::decode(data_str.trim_start_matches("0x")).unwrap_or_default();
            
            let s = state.read().unwrap();
            let block_idx = s.chain.last().map(|b| b.header.index).unwrap_or(0);
            
            match crate::evm::simulate_call(&s.storage, from, to, &data, U256::zero(), 15_000_000, block_idx) {
                Ok((_, out_data, _)) => RpcResponse::success(req.id, json!(format!("0x{}", hex::encode(out_data)))),
                Err(e) => RpcResponse::error(req.id, -32000, &e),
            }
        }
        "eth_getLogs" => {
            let params = req.params.as_array().unwrap();
            let filter = params.get(0).and_then(|v| v.as_object()).unwrap();
            
            let s = state.read().unwrap();
            let latest = s.chain.last().map(|b| b.header.index).unwrap_or(0);

            let from_block = match filter.get("fromBlock") {
                Some(v) => {
                    if let Some(s) = v.as_str() {
                        if s == "latest" { latest }
                        else { u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0) }
                    } else { v.as_u64().unwrap_or(0) }
                }
                None => latest
            };
            let to_block = match filter.get("toBlock") {
                Some(v) => {
                    if let Some(s) = v.as_str() {
                        if s == "latest" { latest }
                        else { u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(latest) }
                    } else { v.as_u64().unwrap_or(latest) }
                }
                None => latest
            };

            let address = filter.get("address").and_then(|v| v.as_str());
            let topic0 = filter.get("topics").and_then(|v| v.as_array()).and_then(|a| a.get(0)).and_then(|v| v.as_str());

            match s.storage.get_event_logs(from_block, to_block, address, topic0) {
                Ok(logs) => RpcResponse::success(req.id, json!(logs)),
                Err(e) => RpcResponse::error(req.id, -32000, &e.to_string()),
            }
        }
        "net_version" => {
            RpcResponse::success(req.id, json!("4919"))
        }
        "web3_clientVersion" => {
            RpcResponse::success(req.id, json!("HomeChain/v4.0.0/rust-revm"))
        }
        "eth_accounts" => {
            // Non-custodial node, no local accounts
            RpcResponse::success(req.id, json!([]))
        }
        "eth_syncing" => {
            // Always return false (fully synced)
            RpcResponse::success(req.id, json!(false))
        }
        "eth_getStorageAt" => {
            let params = req.params.as_array().unwrap();
            let addr = params.get(0).unwrap().as_str().unwrap().to_lowercase();
            let slot_hex = params.get(1).unwrap().as_str().unwrap();
            let s = state.read().unwrap();
            
            // Normalize slot key to 32-byte hex
            let clean_slot = slot_hex.trim_start_matches("0x");
            let padded_slot = format!("0x{:0>64}", clean_slot);
            
            match s.storage.get_storage_slot(&addr, &padded_slot) {
                Ok(Some(val)) => RpcResponse::success(req.id, json!(val)),
                _ => RpcResponse::success(req.id, json!("0x0000000000000000000000000000000000000000000000000000000000000000")),
            }
        }
        "eth_feeHistory" => {
            // Simplified EIP-1559 fee history — fixed base fee for V4 MVP
            let base_fee = format!("0x{:x}", crate::evm_types::INITIAL_BASE_FEE);
            let s = state.read().unwrap();
            let latest = s.chain.last().map(|b| b.header.index).unwrap_or(0);
            RpcResponse::success(req.id, json!({
                "baseFeePerGas": [&base_fee, &base_fee],
                "gasUsedRatio": [0.0],
                "oldestBlock": format!("0x{:x}", latest),
                "reward": [[format!("0x{:x}", 0u64)]]
            }))
        }
        "eth_maxPriorityFeePerGas" => {
            // Fixed tip for V4 MVP (0 priority fee — base fee only)
            RpcResponse::success(req.id, json!("0x0"))
        }
        _ => RpcResponse::error(req.id, -32601, &format!("Method not found: {}", method)),
    }
}
