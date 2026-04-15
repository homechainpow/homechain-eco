//! HomeChain V4 EVM Execution Engine
//!
//! This module bridges the `revm` EVM with HomeChain's SQLite-backed state.
//! It implements the `revm::Database` trait so that `revm` can read account
//! balances, nonces, bytecode, and storage slots from our database.
//!
//! The `execute_transaction` function is the single entry point called from
//! `apply_block_logic` (behind the V4 hardfork height gate).

use crate::storage::Storage;
use crate::evm_types::*;

use ethers_core::types::{Transaction as EthTransaction, U256 as EthU256};
use home_crypto::calculate_keccak256_hex;
use revm::{
    primitives::{
        AccountInfo, Address as RevmAddress, Bytecode, B256, TransactTo,
        TxEnv, BlockEnv, U256 as RevmU256, SpecId,
        ExecutionResult, Output, Account,
    },
    Database, EVM,
};
use std::collections::HashMap;
use std::sync::Arc;

// ═══════════════════════════════════════════════════════════════════════════
// HomeChain Database Bridge
// ═══════════════════════════════════════════════════════════════════════════

/// The bridge between HomeChain's SQLite storage and `revm`'s Database trait.
pub struct HomeChainDB<'a> {
    storage: &'a Storage,
    /// In-memory cache for the duration of one block's execution
    account_cache: HashMap<RevmAddress, Option<AccountInfo>>,
    #[allow(dead_code)]
    block_idx: u64,
}

impl<'a> HomeChainDB<'a> {
    pub fn new(storage: &'a Storage, block_idx: u64) -> Self {
        Self {
            storage,
            account_cache: HashMap::new(),
            block_idx,
        }
    }
}

impl<'a> Database for HomeChainDB<'a> {
    type Error = String;

    /// Fetch account info (nonce, balance, code_hash, bytecode)
    fn basic(&mut self, address: RevmAddress) -> Result<Option<AccountInfo>, Self::Error> {
        if let Some(cached) = self.account_cache.get(&address) {
            return Ok(cached.clone());
        }

        let addr_hex = revm_address_to_hex(&address);
        match self.storage.get_account(&addr_hex) {
            Ok(Some((nonce, balance_str, code_hash_str))) => {
                let balance = EthU256::from_dec_str(&balance_str)
                    .unwrap_or(EthU256::zero());

                let keccak_empty = "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
                let (code_hash, bytecode) = if code_hash_str != keccak_empty {
                    let ch_bytes = hex::decode(code_hash_str.trim_start_matches("0x"))
                        .unwrap_or_default();
                    let mut arr = [0u8; 32];
                    arr[..ch_bytes.len().min(32)].copy_from_slice(&ch_bytes[..ch_bytes.len().min(32)]);
                    let ch = B256::from(arr);

                    let code = match self.storage.get_contract_code(&code_hash_str) {
                        Ok(Some(bytes)) => Some(Bytecode::new_raw(bytes.into())),
                        _ => None,
                    };
                    (ch, code)
                } else {
                    (revm::primitives::KECCAK_EMPTY, None)
                };

                let info = AccountInfo {
                    balance: eth_u256_to_revm(&balance),
                    nonce,
                    code_hash,
                    code: bytecode,
                };
                self.account_cache.insert(address, Some(info.clone()));
                Ok(Some(info))
            }
            Ok(None) => {
                self.account_cache.insert(address, None);
                Ok(None)
            }
            Err(e) => Err(format!("DB error fetching account {}: {}", addr_hex, e)),
        }
    }

    /// Fetch bytecode by its keccak256 hash
    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        let hash_hex = format!("0x{}", hex::encode(code_hash.as_slice()));
        match self.storage.get_contract_code(&hash_hex) {
            Ok(Some(bytes)) => Ok(Bytecode::new_raw(bytes.into())),
            Ok(None) => Ok(Bytecode::new()),
            Err(e) => Err(format!("DB error fetching code {}: {}", hash_hex, e)),
        }
    }

    /// Fetch a contract storage slot
    fn storage(&mut self, address: RevmAddress, index: RevmU256) -> Result<RevmU256, Self::Error> {
        let addr_hex = revm_address_to_hex(&address);
        let slot_key = format!("0x{}", hex::encode(index.to_be_bytes::<32>()));

        match self.storage.get_storage_slot(&addr_hex, &slot_key) {
            Ok(Some(val_hex)) => {
                let clean = val_hex.trim_start_matches("0x");
                let bytes = hex::decode(clean).unwrap_or_default();
                let mut arr = [0u8; 32];
                let len = bytes.len().min(32);
                arr[32 - len..].copy_from_slice(&bytes[..len]);
                Ok(RevmU256::from_be_bytes(arr))
            }
            Ok(None) => Ok(RevmU256::ZERO),
            Err(e) => Err(format!("DB storage error: {}", e)),
        }
    }

    /// Fetch a block hash by number
    fn block_hash(&mut self, number: RevmU256) -> Result<B256, Self::Error> {
        let n: u64 = number.try_into().unwrap_or(0);
        let pseudo_hash = calculate_keccak256_hex(
            format!("homechain_block_{}", n).as_bytes()
        );
        let bytes = hex::decode(&pseudo_hash).unwrap_or_default();
        let mut arr = [0u8; 32];
        arr[..bytes.len().min(32)].copy_from_slice(&bytes[..bytes.len().min(32)]);
        Ok(B256::from(arr))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// EVM Transaction Execution Result
// ═══════════════════════════════════════════════════════════════════════════

pub struct EvmExecutionResult {
    pub success: bool,
    pub gas_used: u64,
    pub contract_address: Option<String>,
    pub logs: Vec<EvmLog>,
    pub revert_reason: Option<String>,
    pub state_diff: revm::primitives::HashMap<RevmAddress, Account>,
}

pub struct EvmLog {
    pub contract_address: String,
    pub topics: Vec<Option<String>>,
    pub data: Vec<u8>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Core Execution Function
// ═══════════════════════════════════════════════════════════════════════════

pub fn execute_transaction(
    storage: &Storage,
    tx: &EthTransaction,
    block_idx: u64,
    block_coinbase: &str,
    block_timestamp: u64,
    block_difficulty: EthU256,
    _tx_index: u64,
    cumulative_gas_used: &mut u64,
    sender_balance: &EthU256,
    sender_nonce: u64,
) -> Result<EvmExecutionResult, String> {

    let sender = match tx.recover_from() {
        Ok(s) => s,
        Err(e) => return Err(format!("Signature recovery failed: {:?}", e)),
    };
    let sender_hex = format!("0x{}", hex::encode(sender.as_bytes())).to_lowercase();

    let gas_limit = tx.gas.as_u64();
    let gas_price = tx.gas_price.unwrap_or(EthU256::from(INITIAL_BASE_FEE));
    let value = tx.value;

    let upfront_cost = gas_price
        .saturating_mul(EthU256::from(gas_limit))
        .saturating_add(value);

    if *sender_balance < upfront_cost {
        return Err(format!(
            "Insufficient balance. Has: {}, Needs: {}", sender_balance, upfront_cost
        ));
    }

    let mut db = HomeChainDB::new(storage, block_idx);

    let balance_str = sender_balance.to_string();
    let _ = storage.upsert_account(
        &sender_hex, sender_nonce, &balance_str,
        "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
    );

    let mut evm = EVM::new();

    evm.env.block = BlockEnv {
        number: RevmU256::from(block_idx),
        coinbase: hex_to_revm_address(block_coinbase),
        timestamp: RevmU256::from(block_timestamp),
        difficulty: eth_u256_to_revm(&block_difficulty),
        basefee: RevmU256::from(INITIAL_BASE_FEE),
        gas_limit: RevmU256::from(BLOCK_GAS_LIMIT),
        ..Default::default()
    };

    evm.env.cfg.chain_id = 4919u64;
    evm.env.cfg.spec_id = SpecId::SHANGHAI;

    let caller = hex_to_revm_address(&sender_hex);
    let transact_to = match tx.to {
        Some(to) => TransactTo::Call(hex_to_revm_address(
            &format!("0x{}", hex::encode(to.as_bytes()))
        )),
        None => TransactTo::Create(revm::primitives::CreateScheme::Create),
    };

    evm.env.tx = TxEnv {
        caller,
        gas_limit,
        gas_price: eth_u256_to_revm(&gas_price),
        gas_priority_fee: None,
        transact_to,
        value: eth_u256_to_revm(&value),
        data: tx.input.0.clone().into(),
        nonce: Some(tx.nonce.as_u64()),
        chain_id: Some(4919),
        access_list: vec![],
        ..Default::default()
    };

    evm.database(&mut db);

    let result_and_state = evm.transact()
        .map_err(|e| format!("EVM execution error: {:?}", e))?;

    let (success, gas_used, contract_address, logs_raw, revert_reason) = match result_and_state.result {
        ExecutionResult::Success { gas_used, output, logs, .. } => {
            let contract_addr = match output {
                Output::Create(_, Some(addr)) => {
                    Some(revm_address_to_hex(&addr))
                }
                _ => None,
            };
            (true, gas_used, contract_addr, logs, None)
        }
        ExecutionResult::Revert { gas_used, output } => {
            let reason = String::from_utf8_lossy(&output).to_string();
            (false, gas_used, None, vec![], Some(reason))
        }
        ExecutionResult::Halt { gas_used, reason } => {
            (false, gas_used, None, vec![], Some(format!("Halt: {:?}", reason)))
        }
    };

    *cumulative_gas_used += gas_used;

    let evm_logs: Vec<EvmLog> = logs_raw.iter().map(|log| {
        let contract_address = revm_address_to_hex(&log.address);
        let topics: Vec<Option<String>> = log.topics.iter().map(|t| {
            Some(format!("0x{}", hex::encode(t.as_slice())))
        }).collect();
        let data = log.data.to_vec();
        EvmLog { contract_address, topics, data }
    }).collect();

    // Commit ALL state changes from ResultAndState!
    let diff = result_and_state.state.clone();
    let _ = commit_evm_state_changes(
        storage, block_idx, result_and_state.state, &contract_address
    );

    Ok(EvmExecutionResult {
        success,
        gas_used,
        contract_address,
        logs: evm_logs,
        revert_reason,
        state_diff: diff,
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// State Commit Phase
// ═══════════════════════════════════════════════════════════════════════════

/// Commit the EVM's post-execution state diff back to SQLite.
/// Writes undo journal entries BEFORE each modification for Reorg safety.
fn commit_evm_state_changes(
    storage: &Storage,
    block_idx: u64,
    state_changes: impl IntoIterator<Item = (RevmAddress, Account)>,
    contract_address: &Option<String>,
) -> Result<(), String> {
    
    for (address, account) in state_changes {
        if !account.is_touched() {
            continue;
        }

        let addr_hex = revm_address_to_hex(&address);
        let info = account.info.clone();

        // Check if balance changed
        let new_balance = revm_u256_to_eth(&info.balance);
        let old_balance = match storage.get_account(&addr_hex) {
            Ok(Some((_, bal, _))) => EthU256::from_dec_str(&bal).unwrap_or(EthU256::zero()),
            _ => EthU256::zero(),
        };

        if new_balance != old_balance {
            let _ = storage.write_journal_entry(
                block_idx, "BALANCE", &addr_hex, None, &old_balance.to_string()
            );
        }

        // Check if nonce changed
        let old_nonce = match storage.get_account(&addr_hex) {
            Ok(Some((nonce, _, _))) => nonce,
            _ => 0,
        };
        
        if info.nonce != old_nonce {
            let _ = storage.write_journal_entry(
                block_idx, "NONCE", &addr_hex, None, &old_nonce.to_string()
            );
        }

        let code_hash_str = format!("0x{}", hex::encode(info.code_hash.as_slice()));
        let _ = storage.upsert_account(&addr_hex, info.nonce, &new_balance.to_string(), &code_hash_str);

        // Record new contract code if deployed here
        // revm 3 accounts have a `code` field if newly loaded or deployed, but `transact` usually modifies code.
        if let Some(bytecode) = info.code {
            if info.code_hash != revm::primitives::KECCAK_EMPTY {
                let _ = storage.write_journal_entry(block_idx, "CODE_CREATE", &addr_hex, None, &code_hash_str);
                let _ = storage.store_contract_code(&code_hash_str, &bytecode.bytecode);
            }
        }

        // Apply storage changes
        for (slot_key_u256, slot_val) in account.storage {
            let new_val = slot_val.present_value();
            let slot_key = format!("0x{}", hex::encode(slot_key_u256.to_be_bytes::<32>()));
            let new_val_hex = format!("0x{}", hex::encode(new_val.to_be_bytes::<32>()));

            let old_val = match storage.get_storage_slot(&addr_hex, &slot_key) {
                Ok(Some(v)) => v,
                _ => "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            };

            if old_val != new_val_hex {
                let _ = storage.write_journal_entry(block_idx, "STORAGE", &addr_hex, Some(&slot_key), &old_val);
                let _ = storage.set_storage_slot(&addr_hex, &slot_key, &new_val_hex);
            }
        }
    }

    if let Some(contract_addr) = contract_address {
        println!("[EVM] Contract deployed at {}", contract_addr);
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Read-Only Simulation (for eth_call / eth_estimateGas)
// ═══════════════════════════════════════════════════════════════════════════

pub fn simulate_call(
    storage: &Storage,
    from: Option<&str>,
    to: &str,
    data: &[u8],
    value: EthU256,
    gas_limit: u64,
    block_idx: u64,
) -> Result<(bool, Vec<u8>, u64), String> {
    let mut db = HomeChainDB::new(storage, block_idx);
    let mut evm = EVM::new();

    evm.env.block = BlockEnv {
        number: RevmU256::from(block_idx),
        gas_limit: RevmU256::from(BLOCK_GAS_LIMIT),
        basefee: RevmU256::from(INITIAL_BASE_FEE),
        ..Default::default()
    };
    
    evm.env.cfg.chain_id = 4919u64;
    evm.env.cfg.spec_id = SpecId::SHANGHAI;

    let caller = from
        .map(|f| hex_to_revm_address(f))
        .unwrap_or(RevmAddress::ZERO);

    evm.env.tx = TxEnv {
        caller,
        gas_limit,
        gas_price: RevmU256::ZERO,
        transact_to: TransactTo::Call(hex_to_revm_address(to)),
        value: eth_u256_to_revm(&value),
        data: data.to_vec().into(),
        nonce: None,
        chain_id: Some(4919),
        ..Default::default()
    };

    evm.database(&mut db);

    match evm.transact() {
        Ok(result_and_state) => {
            match result_and_state.result {
                ExecutionResult::Success { output, gas_used, .. } => {
                    let bytes = match output {
                        Output::Call(b) => b.to_vec(),
                        Output::Create(b, _) => b.to_vec(),
                    };
                    Ok((true, bytes, gas_used))
                }
                ExecutionResult::Revert { output, gas_used } => {
                    Ok((false, output.to_vec(), gas_used))
                }
                ExecutionResult::Halt { gas_used, .. } => {
                    Ok((false, vec![], gas_used))
                }
            }
        }
        Err(e) => Err(format!("Simulation error: {:?}", e)),
    }
}

pub fn estimate_gas(
    storage: &Storage,
    from: Option<&str>,
    to: Option<&str>,
    data: &[u8],
    value: EthU256,
    block_idx: u64,
) -> u64 {
    let mut low = 21_000u64;
    let mut high = BLOCK_GAS_LIMIT;
    let to_addr = to.unwrap_or("0x0000000000000000000000000000000000000000");

    while low < high {
        let mid = low.saturating_add(high) / 2;
        match simulate_call(
            storage, from, to_addr, data, value, mid, block_idx
        ) {
            Ok((true, _, _)) => high = mid,
            _ => low = mid.saturating_add(1),
        }
    }

    std::cmp::min(high.saturating_add(high / 10), BLOCK_GAS_LIMIT)
}
