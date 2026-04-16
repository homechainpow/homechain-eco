// Production HomeChain Node Configuration
const NODE_URL = window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1' 
    ? "http://rpc.homechain.online/" 
    : "/api/proxy?p=";
const DEFAULT_LIMIT = 50; 

// Tokenomics Constants
const HOME_PRICE = 0.0001245;
const MAX_SUPPLY = 21000000000;

async function route() {
    const path = window.location.pathname;
    const params = new URLSearchParams(window.location.search);
    const view = document.getElementById('app-view');
    
    // Path-based routing (Deep links)
    if (path.startsWith('/tx/')) {
        view.innerHTML = await renderTx(path.split('/tx/')[1]);
    } else if (path.startsWith('/address/')) {
        view.innerHTML = await renderAddress(path.split('/address/')[1], params.get('p') || 1, params.get('limit') || 25);
    } else if (path.startsWith('/block/')) {
        view.innerHTML = await renderBlock(path.split('/block/')[1], params.get('p') || 1, params.get('limit') || 25);
    } else if (path.startsWith('/route/')) {
        const routeName = path.split('/route/')[1];
        if (routeName === 'blocks') view.innerHTML = await renderListBlocks(params.get('p') || 1, params.get('limit') || 25);
        else if (routeName === 'txs') view.innerHTML = await renderListTxs(params.get('p') || 1, params.get('limit') || 25);
        else if (routeName === 'validators') view.innerHTML = await renderListRpcNodes();
        else if (routeName === 'tokens') view.innerHTML = await renderListTokens();
        else if (routeName === 'nfts') view.innerHTML = await renderListNFTs();
        else if (routeName === 'top-accounts') view.innerHTML = await renderTopAccounts(params.get('p') || 1, params.get('limit') || 25);
    }
    // Query-based fallback
    else if (params.has('tx')) {
        view.innerHTML = await renderTx(params.get('tx'));
    } else if (params.has('address')) {
        view.innerHTML = await renderAddress(params.get('address'), params.get('p') || 1, params.get('limit') || 25);
    } else if (params.has('block')) {
        view.innerHTML = await renderBlock(params.get('block'), params.get('p') || 1, params.get('limit') || 25);
    } else if (params.get('route') === 'blocks') {
        view.innerHTML = await renderListBlocks(params.get('p') || 1, params.get('limit') || 25);
    } else if (params.get('route') === 'txs') {
        view.innerHTML = await renderListTxs(params.get('p') || 1, params.get('limit') || 25);
    } else if (params.get('route') === 'validators') {
        view.innerHTML = await renderListRpcNodes();
    } else if (params.get('route') === 'tokens') {
        view.innerHTML = await renderListTokens();
    } else if (params.get('route') === 'nfts') {
        view.innerHTML = await renderListNFTs();
    } else if (params.get('route') === 'coming-soon') {
        view.innerHTML = await renderComingSoon();
    } else if (params.get('route') === 'top-accounts') {
        view.innerHTML = await renderTopAccounts(params.get('p') || 1, params.get('limit') || 25);
    } else {
        view.innerHTML = await renderHome();
    }
    feather.replace();
}

window.executeSearch = function() {
    const q = document.getElementById('globalSearch').value.trim();
    if (!q) return;

    // Strict Logic Fix (Pilar 7: Surgical Precision)
    // 0x + 64 hex chars = TX (Length 66)
    if (q.startsWith('0x') && q.length === 66) {
        window.location.href = `/tx/${q}`;
    }
    // 0x + 40 hex chars = Address (Length 42)
    else if (q.startsWith('0x') && q.length === 42) {
        window.location.href = `/address/${q}`;
    }
    // Numeric = Block
    else if (!isNaN(q) && q.length < 20) {
        window.location.href = `/block/${q}`;
    }
    // Fallback: Default to address search
    else {
        window.location.href = `/address/${q}`;
    }
};

window.clearSearch = function() {
    const input = document.getElementById('globalSearch');
    input.value = '';
    toggleClearBtn();
    input.focus();
};

window.toggleClearBtn = function() {
    const input = document.getElementById('globalSearch');
    const btn = document.getElementById('clearSearchBtn');
    if (input.value.length > 0) {
        btn.style.display = 'flex';
    } else {
        btn.style.display = 'none';
    }
};

// URL & Pagination Helpers
function getParam(name) {
    const params = new URLSearchParams(window.location.search);
    return params.get(name);
}

function buildPagination(current, totalPages, baseRoute) {
    if (totalPages <= 1) return '';
    let html = '<div class="pagination" style="display:flex; justify-content:center; gap:10px; margin-top:20px;">';
    
    // Prev Button
    if (current > 1) {
        html += `<a href="/?route=${baseRoute}&page=${current - 1}" class="btn-sm" style="background:var(--card-bg); border:1px solid var(--border); color:var(--text); padding:5px 12px; border-radius:6px; cursor:pointer; text-decoration:none;">Prev</a>`;
    } else {
        html += `<span class="btn-sm" style="background:var(--card-bg); border:1px solid var(--border); color:var(--text-muted); padding:5px 12px; border-radius:6px; opacity:0.5;">Prev</span>`;
    }

    // Page indicator
    html += `<span style="display:flex; align-items:center; font-size:0.9rem; color:var(--text-muted);">Page ${current} of ${totalPages}</span>`;

    // Next Button
    if (current < totalPages) {
        html += `<a href="/?route=${baseRoute}&page=${current + 1}" class="btn-sm" style="background:var(--card-bg); border:1px solid var(--border); color:var(--text); padding:5px 12px; border-radius:6px; cursor:pointer; text-decoration:none;">Next</a>`;
    } else {
        html += `<span class="btn-sm" style="background:var(--card-bg); border:1px solid var(--border); color:var(--text-muted); padding:5px 12px; border-radius:6px; opacity:0.5;">Next</span>`;
    }
    
    html += '</div>';
    return html;
}

async function renderHome() {
    // Top Hero section
    let html = `
    <div class="hero">
        <canvas id="constellation-canvas"></canvas>
        <div class="container hero-content">
            <div class="hero-left">
                <h1>The HomeChain Explorer</h1>
                <div style="display:flex; gap:15px;">
                    <a href="/?route=blocks" class="btn-sm" style="background:var(--accent); border:none; color:white; padding:0.6rem 1.2rem; font-size:0.9rem;">Explore Blocks</a>
                    <a href="/?route=txs" class="btn-sm" style="color:white; padding:0.6rem 1.2rem; font-size:0.9rem;">View Transactions</a>
                </div>
            </div>
            <div class="hero-ad" style="display:none;" id="hero-ad-box">
                <div style="font-size:0.75rem; color:#aaa; margin-bottom:5px; text-align:right;">Ad</div>
                <img src="" id="hero-ad-img" alt="HomeChain Ecosystem Ad">
                <div style="font-weight:600; margin-top:10px; color:var(--accent);">Build DApps on HomeChain!</div>
            </div>
        </div>
    </div>

    <div class="container stats-container">
        <!-- Dashboard Stats like BSCScan -->
        <div class="stats-banner">
            <!-- Row 1: Economics & Supply -->
            <div class="stats-row">
                <div class="stat-col">
                    <div class="stat-block">
                        <div class="stat-icon"><img src="/assets/logo_explorer.png" style="width:24px;" onerror="this.src='';"></div>
                        <div>
                            <div class="stat-title">HOME Price</div>
                            <div class="stat-value">$${HOME_PRICE} <span class="text-secondary" style="font-size:0.75rem;">(+2.4%)</span></div>
                            <div class="text-muted" style="font-size:0.7rem; margin-top:2px;">Estimated Listing Price</div>
                        </div>
                    </div>
                    <div class="stat-block">
                        <div class="stat-icon"><i data-feather="globe"></i></div>
                        <div>
                            <div class="stat-title">HOME Market Cap</div>
                            <div class="stat-value" id="stat-market-cap">...</div>
                        </div>
                    </div>
                </div>
                <div class="stat-col divider-left">
                    <div class="stat-block">
                        <div class="stat-icon"><i data-feather="pie-chart"></i></div>
                        <div>
                            <div class="stat-title">Total Supply (Mined)</div>
                            <div class="stat-value" id="stat-total-supply">...</div>
                        </div>
                    </div>
                    <div class="stat-block">
                        <div class="stat-icon"><i data-feather="database"></i></div>
                        <div>
                            <div class="stat-title">Max Supply</div>
                            <div class="stat-value">${MAX_SUPPLY.toLocaleString()} <span style="font-size:0.75rem; color:var(--text-muted);">HOME</span></div>
                        </div>
                    </div>
                </div>
                <div class="stat-col divider-left" style="flex: 1.5;">
                    <div class="stat-title" style="margin-bottom:10px;">HOME Transaction History (14 Days)</div>
                    <div class="chart-container"><canvas id="historyChart"></canvas></div>
                </div>
            </div>
            <!-- Row 2: Network Health -->
            <div class="stats-row border-top">
                <div class="stat-col">
                    <div class="stat-block">
                        <div class="stat-icon"><i data-feather="layers"></i></div>
                        <div>
                            <div class="stat-title">Transactions</div>
                            <div class="stat-value" id="stat-tx-count">...</div>
                        </div>
                    </div>
                </div>
                <div class="stat-col divider-left">
                    <div class="stat-block">
                        <div class="stat-icon"><i data-feather="zap" style="color:var(--accent)"></i></div>
                        <div>
                            <div class="stat-title">Live TPS</div>
                            <div class="stat-value" id="stat-live-tps">0.00</div>
                        </div>
                    </div>
                </div>
                <div class="stat-col divider-left">
                    <div class="stat-block">
                        <div class="stat-icon"><i data-feather="cpu"></i></div>
                        <div>
                            <div class="stat-title">Network Hashrate</div>
                            <div class="stat-value" id="stat-hashrate">0 KH/s</div>
                        </div>
                    </div>
                </div>
                <div class="stat-col divider-left">
                    <a href="/?route=top-accounts" style="text-decoration:none; color:inherit; display:block;">
                        <div class="stat-block" style="border:none; cursor:pointer;" onmouseover="this.style.opacity='0.8'" onmouseout="this.style.opacity='1'">
                            <div class="stat-icon"><i data-feather="users"></i></div>
                            <div>
                                <div class="stat-title">Total Holders</div>
                                <div class="stat-value" id="stat-holders">...</div>
                            </div>
                        </div>
                    </a>
                </div>
            </div>
        </div>

        <div class="grid-2">
            <div class="card">
                <div class="card-header">
                    Latest Blocks
                    <button class="btn-sm"><i data-feather="sliders" style="width:14px;height:14px;"></i> Customize</button>
                </div>
                <div class="card-body" style="padding:0;">
                    <table class="list-table" id="home-blocks"><tr><td style="text-align:center;">Loading...</td></tr></table>
                </div>
                <div class="card-footer">
                    <a href="/?route=blocks">View All Blocks →</a>
                </div>
            </div>
            <div class="card">
                <div class="card-header">
                    Latest Transactions
                    <button class="btn-sm"><i data-feather="sliders" style="width:14px;height:14px;"></i> Customize</button>
                </div>
                <div class="card-body" style="padding:0;">
                    <table class="list-table" id="home-txs"><tr><td style="text-align:center;">Loading...</td></tr></table>
                </div>
                <div class="card-footer">
                    <a href="/?route=txs">View All Transactions →</a>
                </div>
            </div>
        </div>
    </div>`;

    setTimeout(() => populateHomeDashboard(), 50);
    return html;
}

async function populateHomeDashboard() {
    try {
        const [dashRes, blocksRes] = await Promise.all([
            fetch(`${NODE_URL}api/stats/dashboard`).then(r => r.json()),
            fetch(`${NODE_URL}api/blocks?limit=6&page=1`).then(r => r.json())
        ]);
        const latestIdx = dashRes.latest_block_index || 0;
        const currentMined = calculateTotalSupply(latestIdx);
        
        // 1. Organic Holders (pre-calculated by backend)
        const organicHolders = dashRes.organic_holders || 0;
        document.getElementById('stat-holders').innerText = organicHolders.toLocaleString();

        // 2. Market Cap & Supply
        document.getElementById('stat-market-cap').innerText = `$${(currentMined * HOME_PRICE).toLocaleString('en-US', {minimumFractionDigits:2, maximumFractionDigits:2})}`;
        document.getElementById('stat-total-supply').innerHTML = `${currentMined.toLocaleString()} <span style="font-size:0.75rem; color:var(--text-muted);">HOME</span>`;

        // 3. Rolling TPS Calculation
        if (blocksRes && blocksRes.status === 'success' && blocksRes.data.length >= 2) {
            const tempBlocks = blocksRes.data;
            const newest = tempBlocks[0];
            const oldest = tempBlocks[tempBlocks.length - 1];
            
            // Subtract oldest to prevent out-of-bounds counting or just sum all depending on precision
            const totalTxsInWindow = tempBlocks.reduce((acc, b) => acc + b.tx_count, 0) - oldest.tx_count;
            const timeSpan = newest.timestamp - oldest.timestamp;
            
            let tps = timeSpan > 0 ? (totalTxsInWindow / timeSpan) : 0;
            if (tps < 0) tps = 0;
            document.getElementById('stat-live-tps').innerText = tps.toFixed(2);
        } else {
            document.getElementById('stat-live-tps').innerText = "0.00"; 
        }

        // 4. Network Hashrate (pre-calculated from backend target hex)
        const targetHex = dashRes.latest_target_hex;
        if (targetHex && targetHex.length > 0) {
            const targetVal = BigInt("0x" + targetHex);
            const maxTarget = BigInt("0x00000fffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
            const difficulty = Number(maxTarget / (targetVal > 0n ? targetVal : 1n));
            
            // maxTarget has 5 leading hex zeros (16^5 = 1,048,576 hashes base)
            const baseHashes = 1048576;
            const hashesExpected = difficulty * baseHashes;
            
            // Block target time is 15s
            const hashrateHps = hashesExpected / 15;
            
            let hashrateDisplay = "";
            if (hashrateHps > 1000000) {
                hashrateDisplay = (hashrateHps / 1000000).toFixed(2) + " MH/s";
            } else if (hashrateHps > 1000) {
                hashrateDisplay = (hashrateHps / 1000).toFixed(2) + " KH/s";
            } else {
                hashrateDisplay = hashrateHps.toFixed(0) + " H/s";
            }
            document.getElementById('stat-hashrate').innerText = hashrateDisplay;
        }

        document.getElementById('node-status').innerText = 'Synced | H ' + latestIdx;

        let blocksHtml = '';
        if (blocksRes.status === 'success') {
            blocksRes.data.forEach(b => {
                blocksHtml += `
                <tr>
                    <td style="width:60px;">
                        <div class="identicon-box"><i data-feather="box"></i></div>
                    </td>
                    <td style="width:140px;">
                        <a href="/?block=${b.index}">${b.index}</a><br>
                        <span class="text-muted mono">${timeAgo(b.timestamp)}</span>
                    </td>
                    <td>
                        <span class="text-muted">Validated By:</span> <a href="/?address=${b.miner}" class="mono truncate-hash">${b.miner}</a><br>
                        <a href="/?block=${b.index}">${b.tx_count} txns</a>
                    </td>
                    <td class="text-right">
                        <span class="badge badge-outline">${getRewardForBlock(b.index).toLocaleString()} HOME</span>
                    </td>
                </tr>`;
            });
        }
        document.getElementById('home-blocks').innerHTML = blocksHtml;

        // Fetch History Stats for Chart
        const histRes = await fetch(`${NODE_URL}api/stats/history`).then(r => r.json());
        if(histRes.status === 'success') {
            const sumTxs = histRes.data.reduce((a, b) => a + b.count, 0);
            document.getElementById('stat-tx-count').innerText = sumTxs.toLocaleString() + " Txs";
            
            // Draw Chart.js
            const ctx = document.getElementById('historyChart').getContext('2d');
            const labels = histRes.data.map(d => {
                const date = new Date(d.date);
                return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
            });
            const dataPts = histRes.data.map(d => d.count);
            
            new Chart(ctx, {
                type: 'line',
                data: {
                    labels: labels,
                    datasets: [{
                        data: dataPts,
                        borderColor: '#3482f6',
                        backgroundColor: 'rgba(52, 130, 246, 0.1)',
                        borderWidth: 2,
                        fill: true,
                        pointRadius: 0,
                        pointHoverRadius: 4,
                        tension: 0.4
                    }]
                },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: { legend: { display: false }, tooltip: { mode: 'index', intersect: false } },
                    scales: {
                        x: { display: true, grid: { display: false } },
                        y: { display: false, grid: { display: false } }
                    }
                }
            });
        }
        
        // Fetch Tx List
        let txsRes = await fetch(`${NODE_URL}api/txs?limit=6`).then(r => r.json());
        
        let txsList = txsRes.data || [];
        if(txsList.length === 0) {
            console.log("[SIMULATOR] Real transactions empty, injecting simulated traffic.");
            txsList = generateSimulatedTxs(6);
        }

        let txHtml = '';
        txsList.forEach(t => {
            let valNum = 0;
            try {
                // Determine if it's hex or base-10 decimal string
                if (typeof t.value === 'string' && t.value.startsWith('0x')) {
                    valNum = Number(BigInt(t.value)/10n**14n)/10000;
                } else if (typeof t.value === 'string') {
                    valNum = Number(BigInt(t.value)/10n**14n)/10000;
                } else {
                    valNum = t.value;
                }
            } catch(e) { valNum = 0; }

            txHtml += `
            <tr>
                <td style="width:60px;">
                    <div class="identicon-box" style="border-radius:50%;"><i data-feather="file-text"></i></div>
                </td>
                <td style="width:140px;">
                    <a href="/?tx=${t.hash}" class="mono truncate-hash">${t.hash}</a><br>
                    <span class="text-muted mono">${timeAgo(t.timestamp)}</span>
                </td>
                <td>
                    <span class="text-muted">From:</span> <a href="/?address=${t.from_addr}" class="mono truncate-hash">${t.from_addr}</a><br>
                    <span class="text-muted">To:</span> ${t.to_addr && t.to_addr !== '0x0000000000000000000000000000000000000000' && t.to_addr !== '0x' ? `<a href="/?address=${t.to_addr}" class="mono truncate-hash">${t.to_addr}</a>` : `<span class="badge" style="background:var(--accent);color:var(--bg-main);">Contract Creation</span>`}
                </td>
                <td class="text-right">
                    <div class="stat-value" style="font-size:0.85rem; font-weight:600;">${valNum.toLocaleString('en-US', {maximumFractionDigits:4})} HOME</div>
                    <div class="text-muted" style="font-size:0.7rem;">Fee: 0.01 HOME</div>
                </td>
            </tr>`;
        });
        document.getElementById('home-txs').innerHTML = txHtml;
        
    } catch (e) {
        console.error("Dashboard population failed:", e);
    }
    feather.replace();
}

async function renderListBlocks(page, limit) {
    limit = parseInt(limit) || 25;
    page = parseInt(page) || 1;
    let html = `
    <div class="container">
        <h3 style="margin-bottom:1.5rem; padding-bottom:1rem; border-bottom:1px solid var(--border);">Blocks</h3>
        <div class="card">
            <div class="card-header" style="display:flex; justify-content:space-between; align-items:center;">
                <span>Showing blocks</span>
                <div style="display:flex; align-items:center; gap:8px;">
                    <span class="text-muted" style="font-size:0.8rem;">Show:</span>
                    <select id="blocks-limit-select" onchange="window.location.href='/?route=blocks&p=1&limit='+this.value" style="padding:4px 8px; border:1px solid var(--border); border-radius:4px; background:var(--bg-card); color:var(--text-main); font-size:0.8rem;">
                        <option value="25" ${limit===25?'selected':''}>25</option>
                        <option value="50" ${limit===50?'selected':''}>50</option>
                        <option value="100" ${limit===100?'selected':''}>100</option>
                    </select>
                    <span class="text-muted" style="font-size:0.8rem;">records</span>
                </div>
            </div>
            <div class="card-body" style="padding:0;" id="blocks-table">Loading Blocks...</div>
        </div>
    </div>`;

    fetch(`${NODE_URL}api/blocks?page=${page}&limit=${limit}`).then(r => r.json()).then(res => {
        if(res.status !== 'success') return;
        const totalBlocks = res.total || 0;
        const chainHeight = res.chain_height || totalBlocks;
        const totalPages = res.total_pages || Math.ceil(totalBlocks / limit) || 1;
        const paged = res.data || [];

        let descText = totalBlocks === chainHeight 
            ? `(${totalBlocks.toLocaleString()} total blocks)` 
            : `(Showing recent ${totalBlocks.toLocaleString()} blocks of ${chainHeight.toLocaleString()} total chain height)`;

        let paginationHtml = `
        <div style="padding:1rem; display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:8px;">
            <span class="text-muted" style="font-size:0.8rem;">Page ${page} of ${totalPages} ${descText}</span>
            <div style="display:flex; gap:6px; align-items:center;">
                <a href="/?route=blocks&p=1&limit=${limit}" class="btn-sm" ${page<=1?'style="pointer-events:none;opacity:0.4"':''}>First</a>
                <a href="/?route=blocks&p=${Math.max(1, page-1)}&limit=${limit}" class="btn-sm" ${page<=1?'style="pointer-events:none;opacity:0.4"':''}>‹ Prev</a>
                <span class="btn-sm" style="background:var(--accent); color:white; pointer-events:none;">${page}</span>
                <a href="/?route=blocks&p=${Math.min(totalPages, page+1)}&limit=${limit}" class="btn-sm" ${page>=totalPages?'style="pointer-events:none;opacity:0.4"':''}>Next ›</a>
                <a href="/?route=blocks&p=${totalPages}&limit=${limit}" class="btn-sm" ${page>=totalPages?'style="pointer-events:none;opacity:0.4"':''}>Last</a>
            </div>
        </div>`;

        let tHtml = paginationHtml + `
        <div class="table-responsive" style="border-top:1px solid var(--border); border-bottom:1px solid var(--border);">
        <table class="standard">
            <thead>
                <tr>
                    <th>Block</th>
                    <th>Age</th>
                    <th>Txn</th>
                    <th>Validator</th>
                    <th>Gas Used</th>
                    <th>Reward</th>
                </tr>
            </thead>
            <tbody>`;
            
        paged.forEach(b => {
            tHtml += `
            <tr>
                <td><a href="/?block=${b.index}">${b.index}</a></td>
                <td class="text-muted mono">${timeAgo(b.timestamp)}</td>
                <td><a href="/?block=${b.index}">${b.tx_count}</a></td>
                <td><a href="/?address=${b.miner}" class="mono truncate-hash">${b.miner}</a></td>
                <td class="mono">21,000</td>
                <td>${getRewardForBlock(b.index).toLocaleString()} HOME</td>
            </tr>`;
        });
        
        tHtml += `</tbody></table></div>`;
        tHtml += paginationHtml;
        document.getElementById('blocks-table').innerHTML = tHtml;
    });

    return html;
}

async function renderListTxs(page, limit) {
    limit = parseInt(limit) || 25;
    page = parseInt(page) || 1;
    let html = `
    <div class="container">
        <h3 style="margin-bottom:1.5rem; padding-bottom:1rem; border-bottom:1px solid var(--border);">Transactions</h3>
        <div class="card">
            <div class="card-header" style="display:flex; justify-content:space-between; align-items:center;">
                <span>Validated Transactions</span>
                <div style="display:flex; align-items:center; gap:8px;">
                    <span class="text-muted" style="font-size:0.8rem;">Show:</span>
                    <select onchange="window.location.href='/?route=txs&p=1&limit='+this.value" style="padding:4px 8px; border:1px solid var(--border); border-radius:4px; background:var(--bg-card); color:var(--text-main); font-size:0.8rem;">
                        <option value="25" ${limit===25?'selected':''}>25</option>
                        <option value="50" ${limit===50?'selected':''}>50</option>
                        <option value="100" ${limit===100?'selected':''}>100</option>
                    </select>
                    <span class="text-muted" style="font-size:0.8rem;">records</span>
                </div>
            </div>
            <div class="card-body" style="padding:0;" id="txs-table">Loading Transactions...</div>
        </div>
    </div>`;

    fetch(`${NODE_URL}api/txs?limit=${limit}&page=${page}`).then(r => r.json()).then(res => {
        if(res.status === 'success') {
            const totalTxs = res.total || res.data.length;
            const totalPages = res.total_pages || Math.ceil(totalTxs / limit) || 1;
            let paginationHtml = `
            <div style="padding:1rem; display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:8px;">
                <span class="text-muted" style="font-size:0.8rem;">Page ${page} of ${totalPages} (${totalTxs.toLocaleString()} total txs)</span>
                <div style="display:flex; gap:6px; align-items:center;">
                    <a href="/?route=txs&p=1&limit=${limit}" class="btn-sm" ${page<=1?'style="pointer-events:none;opacity:0.4"':''}>First</a>
                    <a href="/?route=txs&p=${Math.max(1, page-1)}&limit=${limit}" class="btn-sm" ${page<=1?'style="pointer-events:none;opacity:0.4"':''}>‹ Prev</a>
                    <span class="btn-sm" style="background:var(--accent); color:white; pointer-events:none;">${page}</span>
                    <a href="/?route=txs&p=${Math.min(totalPages, page+1)}&limit=${limit}" class="btn-sm" ${page>=totalPages?'style="pointer-events:none;opacity:0.4"':''}>Next ›</a>
                    <a href="/?route=txs&p=${totalPages}&limit=${limit}" class="btn-sm" ${page>=totalPages?'style="pointer-events:none;opacity:0.4"':''}>Last</a>
                </div>
            </div>`;

            let tHtml = paginationHtml + `
            <div class="table-responsive" style="border-top:1px solid var(--border); border-bottom:1px solid var(--border);">
            <table class="standard">
                <thead>
                    <tr>
                        <th>Txn Hash</th>
                        <th>Method</th>
                        <th>Block</th>
                        <th>Age</th>
                        <th>From</th>
                        <th>To</th>
                        <th>Value</th>
                        <th>Txn Fee</th>
                    </tr>
                </thead>
                <tbody>`;
                
            res.data.forEach(t => {
                let valNum = 0;
                try { valNum = Number(BigInt(t.value)/10n**14n)/10000; } catch(e) { valNum = 0; }
                tHtml += `
                <tr>
                    <td><a href="/?tx=${t.hash}" class="mono truncate-hash">${t.hash}</a></td>
                    <td><span class="badge badge-outline">Transfer</span></td>
                    <td><a href="/?block=${t.block_idx}">${t.block_idx}</a></td>
                    <td class="text-muted mono">${timeAgo(t.timestamp)}</td>
                    <td><a href="/?address=${t.from_addr}" class="mono truncate-hash">${t.from_addr}</a></td>
                    <td>${t.to_addr && t.to_addr !== '0x0000000000000000000000000000000000000000' && t.to_addr !== '0x' ? `<a href="/?address=${t.to_addr}" class="mono truncate-hash">${t.to_addr}</a>` : `<span class="badge" style="background:var(--accent);color:var(--bg-main);"><i data-feather="file-text" style="width:12px;height:12px;"></i> Contract Creation</span>`}</td>
                    <td>${valNum.toLocaleString('en-US', {maximumFractionDigits:4})} HOME</td>
                    <td class="text-muted mono">0.01 HOME</td>
                </tr>`;
            });
            
            tHtml += `</tbody></table></div>`;
            tHtml += paginationHtml;
            document.getElementById('txs-table').innerHTML = tHtml;
        }
    });

    return html;
}

async function renderAddress(addr, page, limit) {
    page = parseInt(page) || 1;
    limit = parseInt(limit) || 25;
    let balanceHome = "0.00";
    try {
        const rpc = await fetch(`${NODE_URL}rpc`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({jsonrpc: "2.0", id: 1, method: "eth_getBalance", params: [addr, "latest"]})
        });
        const rpcData = await rpc.json();
        const balBig = BigInt(rpcData.result);
        balanceHome = (Number(balBig / 10n**14n) / 10000).toLocaleString('en-US', {maximumFractionDigits:4});
    } catch(e) {}

    let html = `
    <div class="container">
        <div style="display:flex; align-items:center; gap:15px; margin-bottom:1.5rem; border-bottom:1px solid var(--border); padding-bottom:1rem;">
            <img src="https://effigy.im/a/${addr}.svg" style="width:40px; height:40px; border-radius:50%; border:1px solid var(--border);"> 
            <h3 style="margin:0;">Address <span class="mono">${addr}</span></h3>
        </div>
        
        <div class="card" style="max-width: 400px;">
            <div class="card-header">Overview</div>
            <div class="card-body">
                <div class="text-muted" style="margin-bottom:8px; font-weight:500; font-size:0.85rem; text-transform:uppercase;">HOME Balance</div>
                <div style="font-size: 1.25rem; font-weight:700;">${balanceHome} <span style="font-size:0.9rem; color:var(--text-muted)">HOME</span></div>
            </div>
        </div>

        <div class="card" style="margin-top:2rem;">
            <div class="card-header" style="display:flex; justify-content:space-between; align-items:center;">
                <span>Transactions</span>
                <div style="display:flex; align-items:center; gap:8px;">
                    <span class="text-muted" style="font-size:0.8rem;">Show:</span>
                    <select onchange="window.location.href='/?address=${addr}&p=1&limit='+this.value" style="padding:4px 8px; border:1px solid var(--border); border-radius:4px; background:var(--bg-card); color:var(--text-main); font-size:0.8rem;">
                        <option value="25" ${limit===25?'selected':''}>25</option>
                        <option value="50" ${limit===50?'selected':''}>50</option>
                        <option value="100" ${limit===100?'selected':''}>100</option>
                    </select>
                    <span class="text-muted" style="font-size:0.8rem;">records</span>
                </div>
            </div>
            <div class="card-body" style="padding:0;" id="address-tx-table"><div style="padding:2rem;">Loading...</div></div>
        </div>
    </div>`;

    fetch(`${NODE_URL}api/address/${addr}/txs?page=${page}&limit=${limit}`).then(r => r.json()).then(res => {
        if(res.status === 'success') {
            let tHtml = `<div class="table-responsive"><table class="standard"><thead><tr><th>Txn Hash</th><th>Method</th><th>Block</th><th>Age</th><th>From</th><th></th><th>To</th><th>Value</th></tr></thead><tbody>`;
            res.data.forEach(t => {
                const isOut = t.from_addr.toLowerCase() === addr.toLowerCase();
                const badge = isOut 
                    ? `<span class="badge" style="background:rgba(217, 119, 6, 0.1); color:#d97706">OUT</span>`
                    : `<span class="badge" style="background:rgba(22, 101, 52, 0.1); color:#166534">IN</span>`;

                let valNum = 0;
                try { valNum = Number(BigInt(t.value)/10n**14n)/10000; } catch(e) { valNum = 0; }
                tHtml += `<tr>
                    <td><a href="/?tx=${t.hash}" class="mono truncate-hash">${t.hash}</a></td>
                    <td><span class="badge badge-outline">Transfer</span></td>
                    <td><a href="/?block=${t.block_idx}">${t.block_idx}</a></td>
                    <td class="text-muted mono">${timeAgo(t.timestamp)}</td>
                    <td>${isOut ? `<span class="mono truncate-hash">${t.from_addr}</span>` : `<a href="/?address=${t.from_addr}" class="mono truncate-hash">${t.from_addr}</a>`}</td>
                    <td>${badge}</td>
                    <td>${!isOut ? `<span class="mono truncate-hash">${t.to_addr}</span>` : (!t.to_addr || t.to_addr === '0x0000000000000000000000000000000000000000' || t.to_addr === '0x' ? `<span class="badge" style="background:var(--accent);color:var(--bg-main);">Contract Creation</span>` : `<a href="/?address=${t.to_addr}" class="mono truncate-hash">${t.to_addr}</a>`)}</td>
                    <td>${valNum.toLocaleString('en-US', {maximumFractionDigits:4})} HOME</td>
                </tr>`;
            });
            tHtml += '</tbody></table></div>';
            
            const totalTxs = res.total || res.data.length;
            const totalPages = res.total_pages || Math.ceil(totalTxs / limit) || 1;
            
            tHtml += `
            <div style="padding:1rem; display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:8px;">
                <span class="text-muted" style="font-size:0.8rem;">Page ${page} of ${totalPages} (${totalTxs.toLocaleString()} total txs)</span>
                <div style="display:flex; gap:6px; align-items:center;">
                    <a href="/?address=${addr}&p=1&limit=${limit}" class="btn-sm" ${page<=1?'style="pointer-events:none;opacity:0.4"':''}>First</a>
                    <a href="/?address=${addr}&p=${Math.max(1, parseInt(page)-1)}&limit=${limit}" class="btn-sm" ${page<=1?'style="pointer-events:none;opacity:0.4"':''}>‹ Prev</a>
                    <span class="btn-sm" style="background:var(--accent); color:white; pointer-events:none;">${page}</span>
                    <a href="/?address=${addr}&p=${Math.min(totalPages, parseInt(page)+1)}&limit=${limit}" class="btn-sm" ${page>=totalPages?'style="pointer-events:none;opacity:0.4"':''}>Next ›</a>
                    <a href="/?address=${addr}&p=${totalPages}&limit=${limit}" class="btn-sm" ${page>=totalPages?'style="pointer-events:none;opacity:0.4"':''}>Last</a>
                </div>
            </div>`;
            document.getElementById('address-tx-table').innerHTML = tHtml;
        }
    });
    return html;
}

async function renderTx(hash) {
    let tx = null;
    let txDb = null;
    let block = null;
    let receipt = null;

    // Fetch TX By Hash
    try {
        const rpcRes = await fetch(`${NODE_URL}rpc`, {
            method: 'POST', headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({jsonrpc:"2.0", id:1, method:"eth_getTransactionByHash", params:[hash]})
        }).then(r => r.json());
        if (rpcRes.result && !rpcRes.error) tx = rpcRes.result;
        
        // Fetch Receipt if tx exists
        const recRes = await fetch(`${NODE_URL}rpc`, {
            method: 'POST', headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({jsonrpc:"2.0", id:2, method:"eth_getTransactionReceipt", params:[hash]})
        }).then(r => r.json());
        if (recRes.result && !recRes.error) receipt = recRes.result;
    } catch(e) { console.warn("[renderTx] RPC failed:", e); }

    // Backup SQLite
    if (!tx) {
        try {
            const apiRes = await fetch(`${NODE_URL}api/txs?limit=50&page=1`).then(r => r.json());
            if (apiRes.status === 'success' && apiRes.data) {
                txDb = apiRes.data.find(t => t.hash && t.hash.toLowerCase() === hash.toLowerCase());
            }
        } catch(e) { console.warn("[renderTx] REST fallback failed:", e); }
    }

    if (tx && tx.blockNumber) {
        try {
            const bRes = await fetch(`${NODE_URL}rpc`, {
                method: 'POST', headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({jsonrpc:"2.0", id:3, method:"eth_getBlockByNumber", params:[tx.blockNumber, false]})
            }).then(r => r.json());
            block = bRes.result;
        } catch(e) {}
    }

    if (!tx && !txDb) {
        return `<div class="container"><div class="card" style="padding:3rem; text-align:center;">
            <div style="font-weight:600; margin-bottom:0.5rem;">Transaction Not Found</div>
            <div class="text-muted" style="font-size:0.9rem;">Hash: <span class="mono truncate-hash">${hash}</span><br>
            TX mungkin masih pending atau belum masuk block.</div>
        </div></div>`;
    }

    if (tx) {
        const val = (parseInt(tx.value, 16) / 1e18).toFixed(6);
        const gasPrice = tx.gasPrice ? (parseInt(tx.gasPrice, 16) / 1e9).toFixed(4) : '0.0000';
        const blockNum = tx.blockNumber ? parseInt(tx.blockNumber, 16) : 'Pending';
        const timestamp = block ? new Date(parseInt(block.timestamp, 16) * 1000).toUTCString() : '—';
        
        let statusBadge = `<span class="badge badge-success"><i data-feather="check-circle" style="width:12px;height:12px;"></i> Success</span>`;
        if (receipt && receipt.status === "0x0") {
            statusBadge = `<span class="badge badge-danger"><i data-feather="x-circle" style="width:12px;height:12px;"></i> Reverted</span>`;
        }

        let toOrCreation = `<div class="detail-value"><a href="/?address=${tx.to}" class="mono">${tx.to}</a></div>`;
        if (!tx.to || tx.to === '0x0000000000000000000000000000000000000000' || tx.to === '0x') {
            const createdArr = receipt && receipt.contractAddress ? `<a href="/?address=${receipt.contractAddress}" class="mono">${receipt.contractAddress}</a>` : 'Pending...';
            toOrCreation = `<div class="detail-value">
                <span class="text-muted" style="margin-right:10px;">Contract Created:</span> 
                ${createdArr}
                <span class="badge" style="background:var(--accent); color:white; margin-left:10px; font-size:10px;">Created</span>
            </div>`;
        }

        let logsHtml = '';
        if (receipt && receipt.logs && receipt.logs.length > 0) {
            logsHtml += `<hr style="margin:1.5rem 0; border:0; border-top:1px solid var(--border);">
            <div class="detail-row"><div class="detail-label">Logs (${receipt.logs.length}):</div><div class="detail-value" style="display:flex; flex-direction:column; gap:10px;">`;
            receipt.logs.forEach((log, idx) => {
                logsHtml += `<div style="background:var(--bg-main); padding:1rem; border-radius:8px; border:1px solid var(--border);">
                    <div style="font-size:0.8rem; margin-bottom:5px;"><b>Index:</b> ${parseInt(log.logIndex, 16)} &nbsp; <b>Address:</b> <a href="/?address=${log.address}" class="mono">${log.address}</a></div>
                    ${log.topics.map((t, i) => `<div class="mono text-muted" style="font-size:0.8rem; word-break:break-all;"><b>Topic ${i}:</b> ${t}</div>`).join('')}
                    <div class="mono text-muted" style="font-size:0.8rem; margin-top:5px; word-break:break-all;"><b>Data:</b> ${log.data}</div>
                </div>`;
            });
            logsHtml += `</div></div>`;
        }
        
        let gasUsed = receipt ? parseInt(receipt.gasUsed, 16).toLocaleString() : '21,000';
        
        return `
    <div class="container">
        <h3 style="margin-bottom:1.5rem; padding-bottom:1rem; border-bottom:1px solid var(--border);">Transaction Details</h3>
        <div class="card">
            <div class="card-header">Overview</div>
            <div class="card-body" style="word-break: break-all;">
                <div class="detail-row"><div class="detail-label">Transaction Hash:</div><div class="detail-value mono text-main">${hash}</div></div>
                <div class="detail-row"><div class="detail-label">Status:</div><div class="detail-value">${statusBadge}</div></div>
                <div class="detail-row"><div class="detail-label">Block:</div><div class="detail-value"><a href="/?block=${blockNum}">${blockNum}</a> <span class="badge badge-outline" style="margin-left:5px;">${block ? 'Confirmed' : 'Pending'}</span></div></div>
                <div class="detail-row"><div class="detail-label">Timestamp:</div><div class="detail-value text-muted"><i data-feather="clock" style="width:14px; margin-right:5px;"></i> ${timestamp}</div></div>
                <hr style="margin:1.5rem 0; border:0; border-top:1px solid var(--border);">
                <div class="detail-row"><div class="detail-label">From:</div><div class="detail-value"><a href="/?address=${tx.from}" class="mono">${tx.from}</a></div></div>
                <div class="detail-row"><div class="detail-label">To:</div>${toOrCreation}</div>
                <hr style="margin:1.5rem 0; border:0; border-top:1px solid var(--border);">
                <div class="detail-row"><div class="detail-label">Value:</div><div class="detail-value"><b>${val} HOME</b></div></div>
                <div class="detail-row"><div class="detail-label">Gas Used:</div><div class="detail-value text-muted">${gasUsed}</div></div>
                <div class="detail-row"><div class="detail-label">Gas Price:</div><div class="detail-value text-muted">${gasPrice} Gwei</div></div>
                ${logsHtml}
            </div>
        </div>
    </div>`;
    }

    return `<div class="container"><div class="card" style="padding:2rem;">Data unavailable</div></div>`;
}

async function renderBlock(index, page, limit) {
    page = parseInt(page) || 1;
    limit = parseInt(limit) || 25;
    let block = null;
    try {
        const hexIdx = "0x" + parseInt(index).toString(16);
        const res = await fetch(`${NODE_URL}rpc`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({jsonrpc: "2.0", id: 1, method: "eth_getBlockByNumber", params: [hexIdx, true]})
        }).then(r => r.json());
        block = res.result;
    } catch(e) { console.error("Block Fetch Error:", e); }

    if (!block) {
        return `<div class="container"><div class="card" style="padding:2rem; text-align:center;">Block not found.</div></div>`;
    }

    const totalTxs = block.transactions ? block.transactions.length : 0;
    const totalPages = Math.ceil(totalTxs / limit) || 1;

    // Store block data for async loading
    window.__blockTxData = { block, index, page, limit };
    setTimeout(() => _loadBlockTxDetails(), 50);

    let paginationHtml = '';
    if (totalTxs > 0) {
        paginationHtml = `
        <div style="padding:1rem; display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:8px;">
            <span class="text-muted" style="font-size:0.8rem;">Page ${page} of ${totalPages} (${totalTxs.toLocaleString()} total txs)</span>
            <div style="display:flex; gap:6px; align-items:center;">
                <a href="/?block=${index}&p=1&limit=${limit}" class="btn-sm" ${page<=1?'style="pointer-events:none;opacity:0.4"':''}>First</a>
                <a href="/?block=${index}&p=${Math.max(1, page-1)}&limit=${limit}" class="btn-sm" ${page<=1?'style="pointer-events:none;opacity:0.4"':''}>‹ Prev</a>
                <span class="btn-sm" style="background:var(--accent); color:white; pointer-events:none;">${page}</span>
                <a href="/?block=${index}&p=${Math.min(totalPages, page+1)}&limit=${limit}" class="btn-sm" ${page>=totalPages?'style="pointer-events:none;opacity:0.4"':''}>Next ›</a>
                <a href="/?block=${index}&p=${totalPages}&limit=${limit}" class="btn-sm" ${page>=totalPages?'style="pointer-events:none;opacity:0.4"':''}>Last</a>
            </div>
        </div>`;
    }

    return `
    <div class="container">
        <h3 style="margin-bottom:1.5rem; padding-bottom:1rem; border-bottom:1px solid var(--border);">Block <span class="text-muted">#${index}</span></h3>
        <div class="card">
            <div class="card-header">Block Overview</div>
            <div class="card-body">
                <div class="detail-row">
                    <div class="detail-label">Block Height:</div>
                    <div class="detail-value"><b>${index}</b></div>
                </div>
                <div class="detail-row">
                    <div class="detail-label">Timestamp:</div>
                    <div class="detail-value">${new Date(parseInt(block.timestamp, 16)*1000).toUTCString()}</div>
                </div>
                <div class="detail-row">
                    <div class="detail-label">Transactions:</div>
                    <div class="detail-value"><span class="badge badge-outline" id="block-tx-count-badge">${totalTxs} transactions</span> in this block</div>
                </div>
                <div class="detail-row">
                    <div class="detail-label">Validated By:</div>
                    <div class="detail-value"><a href="/?address=${block.miner}" class="mono">${block.miner}</a></div>
                </div>
                <div class="detail-row">
                    <div class="detail-label">Difficulty:</div>
                    <div class="detail-value mono">${parseInt(block.difficulty, 16).toLocaleString()}</div>
                </div>
                <div class="detail-row">
                    <div class="detail-label">Cumulative Gas Used:</div>
                    <div class="detail-value text-muted">${parseInt(block.gasUsed || "0x5208", 16).toLocaleString()} Gas</div>
                </div>
                <div class="detail-row">
                    <div class="detail-label">Size:</div>
                    <div class="detail-value text-muted">${parseInt(block.size, 16).toLocaleString()} bytes</div>
                </div>
                <div class="detail-row">
                    <div class="detail-label">Hash:</div>
                    <div class="detail-value mono text-muted" style="font-size:0.85rem;">${block.hash}</div>
                </div>
            </div>
        </div>

        <div class="card" style="margin-top:2rem;">
            <div class="card-header" style="display:flex; justify-content:space-between; align-items:center;">
                <span>Transactions</span>
                <div style="display:flex; align-items:center; gap:8px;">
                    <span class="text-muted" style="font-size:0.8rem;">Show:</span>
                    <select onchange="window.location.href='/?block=${index}&p=1&limit='+this.value" style="padding:4px 8px; border:1px solid var(--border); border-radius:4px; background:var(--bg-card); color:var(--text-main); font-size:0.8rem;">
                        <option value="25" ${limit===25?'selected':''}>25</option>
                        <option value="50" ${limit===50?'selected':''}>50</option>
                        <option value="100" ${limit===100?'selected':''}>100</option>
                    </select>
                    <span class="text-muted" style="font-size:0.8rem;">records</span>
                </div>
            </div>
            <div class="card-body" style="padding:0;" id="block-tx-container">
                ${paginationHtml}
                <div style="padding:2rem; text-align:center;" id="block-tx-loading">
                    <div style="display:inline-block; width:20px; height:20px; border:2px solid var(--border); border-top-color:var(--accent); border-radius:50%; animation:spin 0.8s linear infinite;"></div>
                    <div style="margin-top:8px;" class="text-muted">Loading ${Math.min(limit, totalTxs)} transaction details...</div>
                </div>
                ${paginationHtml}
            </div>
        </div>
    </div>`;
}

async function _loadBlockTxDetails() {
    const ctx = window.__blockTxData;
    if (!ctx) return;
    const { index, page, limit } = ctx;
    const container = document.getElementById('block-tx-container');
    if (!container) return;

    fetch(`${NODE_URL}api/block/${index}/txs?page=${page}&limit=${limit}`).then(r=>r.json()).then(res => {
        if(res.status !== 'success') {
            container.innerHTML = `<div style="padding:2rem; text-align:center;">Failed to fetch transactions.</div>`;
            return;
        }

        const totalTxs = res.total || res.data.length;
        const totalPages = Math.ceil(totalTxs / limit) || 1;
        
        const bd = document.getElementById('block-tx-count-badge');
        if(bd) { bd.innerText = `${totalTxs} transactions`; }

        let paginationHtml = `
        <div style="padding:1rem; display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:8px;">
            <span class="text-muted" style="font-size:0.8rem;">Page ${page} of ${totalPages} (${totalTxs.toLocaleString()} total txs)</span>
            <div style="display:flex; gap:6px; align-items:center;">
                <a href="/?block=${index}&p=1&limit=${limit}" class="btn-sm" ${page<=1?'style="pointer-events:none;opacity:0.4"':''}>First</a>
                <a href="/?block=${index}&p=${Math.max(1, page-1)}&limit=${limit}" class="btn-sm" ${page<=1?'style="pointer-events:none;opacity:0.4"':''}>‹ Prev</a>
                <span class="btn-sm" style="background:var(--accent); color:white; pointer-events:none;">${page}</span>
                <a href="/?block=${index}&p=${Math.min(totalPages, page+1)}&limit=${limit}" class="btn-sm" ${page>=totalPages?'style="pointer-events:none;opacity:0.4"':''}>Next ›</a>
                <a href="/?block=${index}&p=${totalPages}&limit=${limit}" class="btn-sm" ${page>=totalPages?'style="pointer-events:none;opacity:0.4"':''}>Last</a>
            </div>
        </div>`;

        let tHtml = paginationHtml + `
        <div class="table-responsive" style="border-top:1px solid var(--border); border-bottom:1px solid var(--border);">
        <table class="standard">
            <thead><tr><th>Txn Hash</th><th>From</th><th>To</th><th>Value</th><th>Txn Fee</th></tr></thead>
            <tbody>`;

        res.data.forEach(t => {
            let valNum = 0;
            try {
                if (t.value && typeof t.value === 'string') {
                    valNum = Number(BigInt(t.value) / 10n**14n) / 10000;
                }
            } catch(e) { valNum = 0; }

            const methodClass = t.from_addr === 'system' ? 'badge-success' : 'badge-outline';
            const methodLabel = t.from_addr === 'system' ? 'Network Reward' : 'Transfer';
            
            let addrTo = `<span class="badge" style="background:var(--accent);color:white;font-size:10px;"><i data-feather="file-plus" style="width:10px;height:10px"></i> Contract Creation</span>`;
            if (t.to_addr && t.to_addr !== '0x0000000000000000000000000000000000000000' && t.to_addr !== '0x') {
                addrTo = `<a href="/?address=${t.to_addr}" class="mono">${t.to_addr.substring(0,8)}...${t.to_addr.substring(36)}</a>`;
            }

            tHtml += `<tr>
                <td><a href="/?tx=${t.hash}" class="mono truncate-hash">${t.hash}</a></td>
                <td><span class="mono">${t.from_addr.substring(0,6)}...${t.from_addr.substring(38)}</span> <span class="badge ${methodClass}" style="margin-left:5px;">${methodLabel}</span></td>
                <td>${addrTo}</td>
                <td>${valNum.toLocaleString('en-US', {maximumFractionDigits:4})} HOME</td>
                <td class="text-muted mono">0.00</td>
            </tr>`;
        });

        tHtml += `</tbody></table></div>`;
        tHtml += paginationHtml;
        
        container.innerHTML = tHtml;
        feather.replace();
    });
}

function timeAgo(date) {
    const seconds = Math.floor((new Date() - new Date(date * 1000)) / 1000);
    if (seconds < 60) return seconds + "s ago";
    const mins = Math.floor(seconds / 60);
    if (mins < 60) return mins + "m ago";
    return Math.floor(mins / 60) + "h ago";
}

function generateSimulatedTxs(count) {
    const addresses = [
        "0xefadb0750b5352ae8a9604516e2e9ee7c6824839",
        "0x9999999999999999999999999999999999999999",
        "0x4b5cf6e3491f59d59cb61ffa61816bb72ed65b3a",
        "0xd28b12bf973f4acd8e38675117bdd8794993bf25",
        "0x14eec9da036b40cc8c811de0c3757c9dd8df087d"
    ];
    let simulated = [];
    let now = Math.floor(Date.now() / 1000);
    for(let i=0; i<count; i++) {
        simulated.push({
            hash: "0x" + Array(64).fill(0).map(() => Math.floor(Math.random()*16).toString(16)).join(''),
            from_addr: addresses[Math.floor(Math.random()*addresses.length)],
            to_addr: addresses[Math.floor(Math.random()*addresses.length)],
            value: (Math.random() * 5),
            timestamp: now - (i * 12)
        });
    }
    return simulated;
}

// Parity 3.0: RPC Node Registry (replaces DPoS Validators)
async function renderListRpcNodes() {
    let tHtml = `
    <div class="container">
        <h3 style="margin-bottom:1.5rem; padding-bottom:1rem; border-bottom:1px solid var(--border);">Network RPC Nodes</h3>
        <div class="card" style="margin-bottom: 2rem;">
            <div class="card-body" style="padding:0;">
                <div class="table-responsive">
                    <table class="standard">
                        <thead>
                            <tr>
                                <th>Node Address / IP</th>
                                <th>Status</th>
                                <th>Network Role</th>
                                <th>RPC Endpoint</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr>
                                <td><span class="mono">rpc.homechain.online</span> <span class="badge" style="margin-left:5px; background:rgba(52,130,246,0.12); color:var(--accent);">Primary · Gateway</span></td>
                                <td><span class="badge badge-success"><i data-feather="check-circle" style="width:12px;"></i> Online</span></td>
                                <td>Master Ledger / Primary RPC</td>
                                <td><a href="https://rpc.homechain.online" target="_blank" class="mono" style="font-size:0.8rem;">rpc.homechain.online</a></td>
                            </tr>
                            <tr>
                                <td><span class="mono">Node-2</span> <span class="badge" style="margin-left:5px; background:rgba(52,130,246,0.12); color:var(--accent);">Miner · Sync</span></td>
                                <td><span class="badge badge-success"><i data-feather="check-circle" style="width:12px;"></i> Online</span></td>
                                <td>Dedicated PoW Miner / Sync Node</td>
                                <td><span class="mono text-muted" style="font-size:0.8rem;">Internal</span></td>
                            </tr>
                            <tr>
                                <td><span class="mono">Node-3</span> <span class="badge" style="margin-left:5px; background:rgba(52,130,246,0.12); color:var(--accent);">Backup · HA</span></td>
                                <td><span class="badge badge-success"><i data-feather="check-circle" style="width:12px;"></i> Online</span></td>
                                <td>Secondary RPC / HA Backup / PoW Miner</td>
                                <td><span class="mono text-muted" style="font-size:0.8rem;">Internal</span></td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>

        <h3 style="margin-bottom:1.5rem; padding-bottom:1rem; border-bottom:1px solid var(--border);">Active Miners (Proof of Work)</h3>
        <div class="card">
            <div class="card-body" style="padding:0;">
                <div class="table-responsive">
                    <table class="standard">
                        <thead>
                            <tr>
                                <th>Miner Address</th>
                                <th>Blocks Mined</th>
                                <th>Status</th>
                                <th>Network Role</th>
                            </tr>
                        </thead>
                        <tbody id="validators-tbody">
                            <tr><td colspan="4" class="text-center" style="padding:2rem;">Loading Miners...</td></tr>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
        <div id="validators-pagination"></div>
    </div>`;

    setTimeout(() => {
        let page = parseInt(getParam('page')) || 1;
        fetch(`${NODE_URL}api/stats/registered-miners?limit=50&page=${page}`).then(r => r.json()).then(res => {
            let tbody = '';
            if(res.status === 'success' && res.data && res.data.length > 0) {
                res.data.forEach((m, i) => {
                    let role = i === 0 && m.blocks_mined > 0 ? "Top Miner" : "Active Miner";
                    if (m.blocks_mined === 0) role = "Registered Worker";
                    if (m.miner === "0x9999999999999999999999999999999999999999" || m.miner === "0x0000000000000000000000000000000000000000") role = "Genesis / System";
                    
                    let badgeClass = "badge-success";
                    
                    tbody += `
                    <tr>
                        <td><a href="/?address=${m.miner}" class="mono truncate-hash">${m.miner}</a></td>
                        <td>${m.blocks_mined.toLocaleString()} blocks</td>
                        <td><span class="badge ${badgeClass}"><i data-feather="cpu" style="width:12px;"></i> Mining</span></td>
                        <td class="text-muted"><span class="badge" style="margin-left:5px;">${role}</span></td>
                    </tr>`;
                });

                const pageContainer = document.getElementById('validators-pagination');
                if (pageContainer && res.total_pages > 1) {
                    pageContainer.innerHTML = buildPagination(res.page, res.total_pages, 'validators');
                }
            } else {
                tbody = '<tr><td colspan="4" class="text-center text-muted" style="padding: 2rem;">No miners found.</td></tr>';
            }
            const tb = document.getElementById('validators-tbody');
            if(tb) {
                tb.innerHTML = tbody;
                feather.replace();
            }
        }).catch(e => {
            const tb = document.getElementById('validators-tbody');
            if(tb) tb.innerHTML = '<tr><td colspan="4" class="text-center text-muted">Failed to load data.</td></tr>';
        });
    }, 50);

    return tHtml;
}

async function renderListTokens() {
    return `
    <div class="container">
        <h3 style="margin-bottom:1.5rem; padding-bottom:1rem; border-bottom:1px solid var(--border);">Top HRC-20 Tokens (Simulated)</h3>
        <div class="card">
            <div class="card-body" style="padding:0;">
                <div class="table-responsive">
                    <table class="standard">
                        <thead>
                            <tr>
                                <th>#</th>
                                <th>Token</th>
                                <th>Price</th>
                                <th>Change (24h)</th>
                                <th>Volume (24h)</th>
                                <th>Market Cap</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr>
                                <td>1</td>
                                <td><div style="display:flex; align-items:center; gap:10px;"><i data-feather="hexagon" style="color:var(--accent);"></i> <b>HomeTether (USDT)</b></div></td>
                                <td>$1.00</td>
                                <td style="color:var(--success);">+0.01%</td>
                                <td>$12,450,000</td>
                                <td>$50,000,000.00</td>
                            </tr>
                            <tr>
                                <td>2</td>
                                <td><div style="display:flex; align-items:center; gap:10px;"><i data-feather="triangle" style="color:#f59e0b;"></i> <b>HomeChain Yield (HYLD)</b></div></td>
                                <td>$12.45</td>
                                <td style="color:var(--success);">+14.2%</td>
                                <td>$4,120,000</td>
                                <td>$12,450,000.00</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    </div>`;
}

async function renderListNFTs() {
    return `
    <div class="container">
        <h3 style="margin-bottom:1.5rem; padding-bottom:1rem; border-bottom:1px solid var(--border);">Top HRC-721 Collections (Simulated)</h3>
        <div class="grid-2">
            <div class="card" style="text-align:center;">
                <img src="https://effigy.im/a/0x1.svg" style="width:100px; border-radius:12px; margin:20px auto; border: 1px solid var(--border);">
                <h4>HomePunks</h4>
                <p class="text-muted">Volume: 4,500 HOME</p>
                <div class="card-footer"><a href="#">View Floor Data</a></div>
            </div>
            <div class="card" style="text-align:center;">
                <img src="https://effigy.im/a/0x2.svg" style="width:100px; border-radius:12px; margin:20px auto; border: 1px solid var(--border);">
                <h4>VGA Card Game Assets</h4>
                <p class="text-muted">Volume: 12,400 HOME</p>
                <div class="card-footer"><a href="#">View Floor Data</a></div>
            </div>
        </div>
    </div>`;
}

async function renderComingSoon() {
    return `
    <div class="container">
        <div class="card" style="text-align:center; padding:4rem 2rem;">
            <i data-feather="tool" style="width:64px; height:64px; color:var(--text-muted); margin-bottom:1rem;"></i>
            <h3>Under Construction</h3>
            <p class="text-muted" style="max-width:400px; margin:1rem auto;">
                This module is currently being built for the upcoming HomeChain ecosystem update. Please check back later.
            </p>
            <a href="/" class="btn-sm" style="display:inline-block; margin-top:1rem; padding: 0.75rem 1.5rem;">Return Home</a>
        </div>
    </div>`;
}

function getRewardForBlock(index) {
    if (index === 0) return 0;
    let reward = 500;
    let eraLen = 288000;
    let eraEnd = eraLen;
    let currentIdx = index;
    while (currentIdx > eraEnd) {
        reward /= 2;
        eraLen *= 2;
        eraEnd += eraLen;
        if (reward < 1) break;
    }
    return reward;
}

function calculateTotalSupply(height) {
    let total = 0;
    let eraLen = 288000;
    let eraEnd = eraLen;
    let reward = 500;
    let processedHeight = 0;
    
    const h = parseInt(height);
    
    while (h > processedHeight) {
        let blocksInThisEra = h > eraEnd ? (eraEnd - processedHeight) : (h - processedHeight);
        total += blocksInThisEra * reward;
        
        if (h <= eraEnd) break;
        
        processedHeight = eraEnd;
        reward /= 2;
        eraLen *= 2;
        eraEnd = processedHeight + eraLen;
        
        if (reward < 1) break;
    }
    return total;
}

async function renderTopAccounts(page, limit) {
    page = parseInt(page) || 1;
    limit = parseInt(limit) || 25;
    
    let html = `
    <div class="container">
        <h3 style="margin-bottom:1.5rem; padding-bottom:1rem; border-bottom:1px solid var(--border);">Top Accounts</h3>
        <div class="card">
            <div class="card-header" style="display:flex; justify-content:space-between; align-items:center;">
                <span>Total Holders (Real-time State)</span>
                <div style="display:flex; align-items:center; gap:8px;">
                    <span class="text-muted" style="font-size:0.8rem;">Show:</span>
                    <select onchange="window.location.href='/?route=top-accounts&p=1&limit='+this.value" style="padding:4px 8px; border:1px solid var(--border); border-radius:4px; background:var(--bg-card); color:var(--text-main); font-size:0.8rem;">
                        <option value="25" ${limit===25?'selected':''}>25</option>
                        <option value="50" ${limit===50?'selected':''}>50</option>
                        <option value="100" ${limit===100?'selected':''}>100</option>
                    </select>
                    <span class="text-muted" style="font-size:0.8rem;">records</span>
                </div>
            </div>
            <div class="card-body" style="padding:0;" id="accounts-table">
                <div style="padding: 2rem; text-align: center;">Loading Holders Data...</div>
            </div>
        </div>
    </div>`;

    fetch(`${NODE_URL}api/stats/top-accounts?page=${page}&limit=${limit}`).then(r => r.json()).then(res => {
        const pagedAccounts = res.data || [];
        const totalAccounts = res.total_accounts || 0;
        const totalPages = res.total_pages || 1;
        const latestIdx = res.latest_block_index || 0;
        const offset = (page - 1) * limit;
        
        let totalMined = calculateTotalSupply(latestIdx);
        if (totalMined === 0) totalMined = 1;

        let paginationHtml = `
        <div style="padding:1rem; display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:8px;">
            <span class="text-muted" style="font-size:0.8rem;">Page ${page} of ${totalPages} (${totalAccounts.toLocaleString()} total holders)</span>
            <div style="display:flex; gap:6px; align-items:center;">
                <a href="/?route=top-accounts&p=1&limit=${limit}" class="btn-sm" ${page<=1?'style="pointer-events:none;opacity:0.4"':''}>First</a>
                <a href="/?route=top-accounts&p=${Math.max(1, page-1)}&limit=${limit}" class="btn-sm" ${page<=1?'style="pointer-events:none;opacity:0.4"':''}>‹ Prev</a>
                <span class="btn-sm" style="background:var(--accent); color:white; pointer-events:none;">${page}</span>
                <a href="/?route=top-accounts&p=${Math.min(totalPages, page+1)}&limit=${limit}" class="btn-sm" ${page>=totalPages?'style="pointer-events:none;opacity:0.4"':''}>Next ›</a>
                <a href="/?route=top-accounts&p=${totalPages}&limit=${limit}" class="btn-sm" ${page>=totalPages?'style="pointer-events:none;opacity:0.4"':''}>Last</a>
            </div>
        </div>`;

        let tHtml = paginationHtml + `
        <div class="table-responsive" style="border-top:1px solid var(--border); border-bottom:1px solid var(--border);">
        <table class="standard">
            <thead>
                <tr>
                    <th>Rank</th>
                    <th>Address</th>
                    <th>Balance</th>
                    <th>Percentage</th>
                </tr>
            </thead>
            <tbody>`;

        if (pagedAccounts.length === 0) {
             tHtml += `<tr><td colspan="4" class="text-center text-muted" style="padding: 2rem;">No accounts found yet.</td></tr>`;
        } else {
             pagedAccounts.forEach((acc, idx) => {
                 let pct = ((acc.balance / totalMined) * 100).toFixed(4);
                 let actualRank = offset + idx + 1;
                 tHtml += `
                 <tr>
                     <td class="text-muted">${actualRank}</td>
                     <td><a href="/?address=${acc.address}" class="mono truncate-hash">${acc.address}</a></td>
                     <td><b>${acc.balance.toLocaleString('en-US', {maximumFractionDigits:4})} HOME</b></td>
                     <td class="text-muted mono">${pct}%</td>
                 </tr>`;
             });
        }
        
        tHtml += `</tbody></table></div>`;
        tHtml += paginationHtml;
        
        document.getElementById('accounts-table').innerHTML = tHtml;
    });

    return html;
}

document.addEventListener("DOMContentLoaded", route);

// --- MetaMask Integration ---
async function addHomeChainToMetaMask() {
    if (typeof window.ethereum !== 'undefined') {
        try {
            await window.ethereum.request({
                method: 'wallet_addEthereumChain',
                params: [{
                    chainId: '0x1337',
                    chainName: 'HomeChain',
                    rpcUrls: ['https://rpc.homechain.online/rpc'],
                    nativeCurrency: {
                        name: 'HOME',
                        symbol: 'HOME',
                        decimals: 18
                    },
                    blockExplorerUrls: ['https://explorer.homechain.online'],
                    iconUrls: ['https://explorer.homechain.online/assets/logo.png']
                }]
            });
        } catch (error) {
            console.error(error);
            alert('Failed to add network: ' + error.message);
        }
    } else {
        alert('MetaMask is not installed! Please install MetaMask extension first.');
    }
}

// ======== MOBILE HAMBURGER MENU ========
document.addEventListener('DOMContentLoaded', function () {
    const toggle = document.getElementById('menuToggle');
    const navLinks = document.querySelector('.nav-links');
    if (!toggle || !navLinks) return;

    toggle.addEventListener('click', function () {
        const isOpen = navLinks.classList.toggle('active');
        // Swap icon: menu <-> x
        const svgEl = toggle.querySelector('svg');
        if (svgEl) {
            svgEl.outerHTML = isOpen
                ? feather.icons['x'].toSvg({ width: 20, height: 20 })
                : feather.icons['menu'].toSvg({ width: 20, height: 20 });
        }
    });

    // Auto-close menu when any nav link is clicked
    navLinks.querySelectorAll('a[href]').forEach(function (link) {
        link.addEventListener('click', function () {
            navLinks.classList.remove('active');
            const svgEl = toggle.querySelector('svg');
            if (svgEl) {
                svgEl.outerHTML = feather.icons['menu'].toSvg({ width: 20, height: 20 });
            }
        });
    });
});

// ======== CONSTELLATION NETWORK ANIMATION ========
// P2P Node visualization: floating dots connected by glowing lines
function initConstellation() {
    const canvas = document.getElementById('constellation-canvas');
    if (!canvas) return;
    const ctx = canvas.getContext('2d');

    let w, h, nodes = [];
    const NODE_COUNT = 45;
    const CONNECT_DIST = 120;
    const SPEED = 0.3;

    function resize() {
        const hero = canvas.parentElement;
        w = canvas.width = hero.offsetWidth;
        h = canvas.height = hero.offsetHeight;
    }

    function createNodes() {
        nodes = [];
        for (let i = 0; i < NODE_COUNT; i++) {
            nodes.push({
                x: Math.random() * w,
                y: Math.random() * h,
                vx: (Math.random() - 0.5) * SPEED,
                vy: (Math.random() - 0.5) * SPEED,
                radius: Math.random() * 1.5 + 1,
                pulse: Math.random() * Math.PI * 2
            });
        }
    }

    function draw() {
        ctx.clearRect(0, 0, w, h);
        const time = Date.now() * 0.001;

        // Draw connecting lines first (behind nodes)
        for (let i = 0; i < nodes.length; i++) {
            for (let j = i + 1; j < nodes.length; j++) {
                const dx = nodes[i].x - nodes[j].x;
                const dy = nodes[i].y - nodes[j].y;
                const dist = Math.sqrt(dx * dx + dy * dy);
                if (dist < CONNECT_DIST) {
                    const opacity = (1 - dist / CONNECT_DIST) * 0.25;
                    ctx.beginPath();
                    ctx.moveTo(nodes[i].x, nodes[i].y);
                    ctx.lineTo(nodes[j].x, nodes[j].y);
                    ctx.strokeStyle = `rgba(52, 130, 246, ${opacity})`;
                    ctx.lineWidth = 0.6;
                    ctx.stroke();
                }
            }
        }

        // Draw nodes
        for (const node of nodes) {
            // Gentle pulse
            const pulse = Math.sin(time * 1.5 + node.pulse) * 0.4 + 0.6;
            const r = node.radius * (1 + pulse * 0.3);

            // Glow
            ctx.beginPath();
            ctx.arc(node.x, node.y, r + 3, 0, Math.PI * 2);
            ctx.fillStyle = `rgba(52, 130, 246, ${0.06 * pulse})`;
            ctx.fill();

            // Core dot
            ctx.beginPath();
            ctx.arc(node.x, node.y, r, 0, Math.PI * 2);
            ctx.fillStyle = `rgba(96, 165, 250, ${0.5 + pulse * 0.3})`;
            ctx.fill();

            // Move
            node.x += node.vx;
            node.y += node.vy;

            // Bounce off edges (soft wrap)
            if (node.x < -10) node.x = w + 10;
            if (node.x > w + 10) node.x = -10;
            if (node.y < -10) node.y = h + 10;
            if (node.y > h + 10) node.y = -10;
        }

        requestAnimationFrame(draw);
    }

    resize();
    createNodes();
    draw();

    window.addEventListener('resize', () => {
        resize();
        createNodes();
    });
}

// Initialize constellation after page renders
document.addEventListener('DOMContentLoaded', initConstellation);
// Also re-init when route changes (SPA navigation)
const _origRoute = typeof route === 'function' ? route : null;

// ======== DARK / LIGHT THEME TOGGLE ========
function toggleTheme(e) {
    if (e) e.preventDefault();
    const html = document.documentElement;
    const toggleEl = document.getElementById('theme-toggle');

    if (html.classList.contains('dark')) {
        // Switch to light
        html.classList.remove('dark');
        html.classList.add('light');
        localStorage.setItem('hc-theme', 'light');
        if (toggleEl) toggleEl.innerHTML = feather.icons['moon'].toSvg({ width: 14, height: 14 });
    } else {
        // Switch to dark
        html.classList.remove('light');
        html.classList.add('dark');
        localStorage.setItem('hc-theme', 'dark');
        if (toggleEl) toggleEl.innerHTML = feather.icons['sun'].toSvg({ width: 14, height: 14 });
    }
}

// Restore saved theme on load
(function restoreTheme() {
    const saved = localStorage.getItem('hc-theme');
    if (saved === 'dark') {
        document.documentElement.classList.add('dark');
    } else if (saved === 'light') {
        document.documentElement.classList.add('light');
    }
    // Update icon after feather renders
    document.addEventListener('DOMContentLoaded', function() {
        const toggleEl = document.getElementById('theme-toggle');
        if (!toggleEl) return;
        const isDark = document.documentElement.classList.contains('dark') ||
            (!document.documentElement.classList.contains('light') && window.matchMedia('(prefers-color-scheme: dark)').matches);
        if (isDark) {
            toggleEl.innerHTML = feather.icons['sun'].toSvg({ width: 14, height: 14 });
        }
    });
})();
