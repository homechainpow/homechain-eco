use crate::SharedState;
use home_core::Block;
use reqwest::Client;

// Limit maximum reorg depth to 100 blocks
const MAX_REORG_DEPTH: u64 = 100;

pub async fn evaluate_fork(state: SharedState, new_block: Block, peer: &str) {
    let (last_idx, is_behind) = {
        let s = state.read().unwrap();
        let current_last = s.chain.last().map(|b| b.header.index).unwrap_or(0);
        
        // If the new block is exactly where we are but a different hash (Fork!)
        // or if it's slightly behind but claims a longer chain later?
        // For now, only evaluate if it's same index but different hash.
        if new_block.header.index == current_last {
            // Compare proof of work "weight".
            // Since we use DDA, lower target = higher difficulty. 
            // In a real system you'd sum difficulty. For simplicity, just check target.
            let local_target = &s.chain.last().unwrap().header.target;
            if new_block.header.target < *local_target {
                println!("[REORG] Peer {} sent a heavier block at #{}. Requires Reorg!", peer, new_block.header.index);
                (current_last, true)
            } else {
                (current_last, false)
            }
        } else {
            (current_last, false) // Ignore old blocks
        }
    };

    if is_behind {
        // Rollback 1 block and apply this new one
        rollback_blocks(state.clone(), 1).await;
        
        let mut s = state.write().unwrap();
        // Assume apply_block_logic correctly handles appending
        s.apply_block_logic(new_block, false, false);
    }
}

pub async fn rollback_blocks(state: SharedState, depth: u64) {
    if depth > MAX_REORG_DEPTH {
        println!("[REORG CAUTION] Prevented deep rollback of {} blocks. Max allowed is {}.", depth, MAX_REORG_DEPTH);
        return;
    }

    let mut s = state.write().unwrap();
    let current_len = s.chain.len();
    if (current_len as u64) < depth {
        return; 
    }

    // A correct rollback would reverse balances here using DB historical rewards
    // For V2 minimal, we just pop the blocks. True rollback requires DB manipulation.
    for _ in 0..depth {
        if let Some(popped) = s.chain.pop() {
            println!("[REORG] Dropped block #{}", popped.header.index);
            // Ideally: s.storage.delete_block(popped.header.index);
        }
    }
}
