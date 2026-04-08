// explorer.rs - Sentrix Chain — Block Explorer Web UI

use axum::{
    extract::{State, Path},
    response::Html,
};
use crate::api::routes::SharedState;

const CSS: &str = r#"
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: 'Segoe UI', system-ui, sans-serif; background: #0a0e17; color: #e1e5ee; }
.container { max-width: 1200px; margin: 0 auto; padding: 20px; }
header { background: linear-gradient(135deg, #1a1f35, #0d1225); padding: 20px 0; border-bottom: 1px solid #2a3050; }
header h1 { font-size: 24px; color: #7c8aff; }
header span { color: #5a6380; font-size: 14px; }
.stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px; margin: 24px 0; }
.stat-card { background: #111827; border: 1px solid #1f2937; border-radius: 12px; padding: 20px; }
.stat-card .label { color: #6b7280; font-size: 13px; text-transform: uppercase; }
.stat-card .value { color: #f9fafb; font-size: 22px; font-weight: 600; margin-top: 4px; }
table { width: 100%; border-collapse: collapse; margin-top: 16px; }
th { background: #111827; color: #9ca3af; font-size: 12px; text-transform: uppercase; padding: 12px 16px; text-align: left; }
td { padding: 12px 16px; border-bottom: 1px solid #1f2937; font-size: 14px; }
tr:hover td { background: #111827; }
a { color: #7c8aff; text-decoration: none; }
a:hover { text-decoration: underline; }
.hash { font-family: 'Consolas', monospace; font-size: 13px; color: #9ca3af; }
.badge { display: inline-block; padding: 2px 8px; border-radius: 4px; font-size: 12px; }
.badge-green { background: #064e3b; color: #34d399; }
.badge-blue { background: #1e3a5f; color: #60a5fa; }
.tabs { display: flex; gap: 8px; margin: 20px 0; }
.tab { padding: 8px 16px; border-radius: 8px; background: #111827; color: #9ca3af; border: 1px solid #1f2937; }
.tab.active { background: #1e3a5f; color: #60a5fa; border-color: #3b82f6; }
.mono { font-family: 'Consolas', monospace; }
"#;

fn page(title: &str, body: &str) -> Html<String> {
    Html(format!(r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>{title} — Sentrix Explorer</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>{CSS}</style></head><body>
<header><div class="container">
<h1>Sentrix Explorer</h1>
<span>Chain ID: 7119 | PoA Blockchain</span>
</div></header>
<div class="container">{body}</div>
</body></html>"#))
}

// ── Explorer home ────────────────────────────────────────
pub async fn explorer_home(State(state): State<SharedState>) -> Html<String> {
    let bc = state.read().await;
    let stats = bc.chain_stats();
    let height = bc.height();

    let mut blocks_html = String::new();
    let start = if height > 20 { height - 20 } else { 0 };
    for i in (start..=height).rev() {
        if let Some(block) = bc.get_block(i) {
            blocks_html.push_str(&format!(
                r#"<tr>
                <td><a href="/explorer/block/{}">{}</a></td>
                <td class="hash">{}</td>
                <td>{}</td>
                <td>{}</td>
                <td class="hash">{}</td>
                </tr>"#,
                block.index, block.index,
                &block.hash[..16],
                block.tx_count(),
                block.timestamp,
                &block.validator[..block.validator.len().min(20)],
            ));
        }
    }

    let body = format!(r#"
    <div class="stats">
        <div class="stat-card"><div class="label">Height</div><div class="value">{}</div></div>
        <div class="stat-card"><div class="label">Total Minted</div><div class="value">{} SRX</div></div>
        <div class="stat-card"><div class="label">Total Burned</div><div class="value">{} SRX</div></div>
        <div class="stat-card"><div class="label">Validators</div><div class="value">{}</div></div>
        <div class="stat-card"><div class="label">Tokens</div><div class="value">{}</div></div>
        <div class="stat-card"><div class="label">Mempool</div><div class="value">{}</div></div>
    </div>
    <div class="tabs">
        <a class="tab active" href="/explorer">Blocks</a>
        <a class="tab" href="/explorer/validators">Validators</a>
        <a class="tab" href="/explorer/tokens">Tokens</a>
    </div>
    <table>
    <tr><th>Block</th><th>Hash</th><th>Txs</th><th>Timestamp</th><th>Validator</th></tr>
    {}
    </table>"#,
        stats["height"],
        stats["total_minted_srx"],
        stats["total_burned_srx"],
        stats["active_validators"],
        stats["deployed_tokens"],
        stats["mempool_size"],
        blocks_html,
    );

    page("Home", &body)
}

// ── Block detail ─────────────────────────────────────────
pub async fn explorer_block(
    State(state): State<SharedState>,
    Path(index): Path<u64>,
) -> Html<String> {
    let bc = state.read().await;
    match bc.get_block(index) {
        Some(block) => {
            let mut txs_html = String::new();
            for tx in &block.transactions {
                let badge = if tx.is_coinbase() {
                    r#"<span class="badge badge-green">COINBASE</span>"#
                } else {
                    r#"<span class="badge badge-blue">TX</span>"#
                };
                txs_html.push_str(&format!(
                    r#"<tr>
                    <td>{}</td>
                    <td class="hash"><a href="/explorer/tx/{}">{}</a></td>
                    <td class="mono">{}</td>
                    <td class="mono">{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    </tr>"#,
                    badge,
                    tx.txid, &tx.txid[..16],
                    &tx.from_address,
                    &tx.to_address,
                    tx.amount,
                    tx.fee,
                ));
            }

            let body = format!(r#"
            <h2 style="margin:20px 0">Block #{}</h2>
            <table>
            <tr><td style="color:#6b7280;width:150px">Hash</td><td class="hash">{}</td></tr>
            <tr><td style="color:#6b7280">Previous Hash</td><td class="hash">{}</td></tr>
            <tr><td style="color:#6b7280">Merkle Root</td><td class="hash">{}</td></tr>
            <tr><td style="color:#6b7280">Timestamp</td><td>{}</td></tr>
            <tr><td style="color:#6b7280">Validator</td><td class="mono"><a href="/explorer/address/{}">{}</a></td></tr>
            <tr><td style="color:#6b7280">Transactions</td><td>{}</td></tr>
            </table>
            <h3 style="margin:24px 0 12px">Transactions</h3>
            <table>
            <tr><th>Type</th><th>TxID</th><th>From</th><th>To</th><th>Amount</th><th>Fee</th></tr>
            {}
            </table>"#,
                block.index,
                block.hash,
                block.previous_hash,
                block.merkle_root,
                block.timestamp,
                block.validator, block.validator,
                block.tx_count(),
                txs_html,
            );
            page(&format!("Block #{}", block.index), &body)
        }
        None => page("Not Found", "<h2>Block not found</h2>"),
    }
}

// ── Address detail ───────────────────────────────────────
pub async fn explorer_address(
    State(state): State<SharedState>,
    Path(address): Path<String>,
) -> Html<String> {
    let bc = state.read().await;
    let balance = bc.accounts.get_balance(&address);
    let nonce = bc.accounts.get_nonce(&address);
    let history = bc.get_address_history(&address);

    let mut txs_html = String::new();
    for tx in history.iter().rev().take(50) {
        let dir = tx["direction"].as_str().unwrap_or("?");
        let badge = match dir {
            "in" => r#"<span class="badge badge-green">IN</span>"#,
            "out" => r#"<span class="badge badge-blue">OUT</span>"#,
            "reward" => r#"<span class="badge badge-green">REWARD</span>"#,
            _ => "",
        };
        txs_html.push_str(&format!(
            r#"<tr>
            <td>{}</td>
            <td class="hash"><a href="/explorer/tx/{}">{}</a></td>
            <td>{}</td>
            <td>{}</td>
            <td><a href="/explorer/block/{}">#{}</a></td>
            </tr>"#,
            badge,
            tx["txid"].as_str().unwrap_or(""),
            &tx["txid"].as_str().unwrap_or("")[..16.min(tx["txid"].as_str().unwrap_or("").len())],
            tx["amount"],
            tx["fee"],
            tx["block_index"], tx["block_index"],
        ));
    }

    let body = format!(r#"
    <h2 style="margin:20px 0">Address</h2>
    <table>
    <tr><td style="color:#6b7280;width:150px">Address</td><td class="mono">{}</td></tr>
    <tr><td style="color:#6b7280">Balance</td><td>{} sentri ({} SRX)</td></tr>
    <tr><td style="color:#6b7280">Nonce</td><td>{}</td></tr>
    <tr><td style="color:#6b7280">Transactions</td><td>{}</td></tr>
    </table>
    <h3 style="margin:24px 0 12px">Transaction History</h3>
    <table>
    <tr><th>Dir</th><th>TxID</th><th>Amount</th><th>Fee</th><th>Block</th></tr>
    {}
    </table>"#,
        address,
        balance, balance as f64 / 100_000_000.0,
        nonce,
        history.len(),
        txs_html,
    );

    page(&format!("Address {}", &address[..10]), &body)
}

// ── Transaction detail ───────────────────────────────────
pub async fn explorer_tx(
    State(state): State<SharedState>,
    Path(txid): Path<String>,
) -> Html<String> {
    let bc = state.read().await;
    match bc.get_transaction(&txid) {
        Some(tx_data) => {
            let tx = &tx_data["transaction"];
            let body = format!(r#"
            <h2 style="margin:20px 0">Transaction</h2>
            <table>
            <tr><td style="color:#6b7280;width:150px">TxID</td><td class="hash">{}</td></tr>
            <tr><td style="color:#6b7280">From</td><td class="mono"><a href="/explorer/address/{}">{}</a></td></tr>
            <tr><td style="color:#6b7280">To</td><td class="mono"><a href="/explorer/address/{}">{}</a></td></tr>
            <tr><td style="color:#6b7280">Amount</td><td>{} sentri</td></tr>
            <tr><td style="color:#6b7280">Fee</td><td>{} sentri</td></tr>
            <tr><td style="color:#6b7280">Nonce</td><td>{}</td></tr>
            <tr><td style="color:#6b7280">Block</td><td><a href="/explorer/block/{}">#{}</a></td></tr>
            <tr><td style="color:#6b7280">Timestamp</td><td>{}</td></tr>
            </table>"#,
                tx["txid"].as_str().unwrap_or(""),
                tx["from_address"].as_str().unwrap_or(""), tx["from_address"].as_str().unwrap_or(""),
                tx["to_address"].as_str().unwrap_or(""), tx["to_address"].as_str().unwrap_or(""),
                tx["amount"],
                tx["fee"],
                tx["nonce"],
                tx_data["block_index"], tx_data["block_index"],
                tx["timestamp"],
            );
            page("Transaction", &body)
        }
        None => page("Not Found", "<h2>Transaction not found</h2>"),
    }
}

// ── Validators page ──────────────────────────────────────
pub async fn explorer_validators(State(state): State<SharedState>) -> Html<String> {
    let bc = state.read().await;

    let mut rows = String::new();
    for v in bc.authority.validators.values() {
        let status = if v.is_active {
            r#"<span class="badge badge-green">ACTIVE</span>"#
        } else {
            r#"<span class="badge" style="background:#4a1c1c;color:#f87171">INACTIVE</span>"#
        };
        rows.push_str(&format!(
            r#"<tr>
            <td>{}</td>
            <td class="mono"><a href="/explorer/address/{}">{}</a></td>
            <td>{}</td>
            <td>{}</td>
            </tr>"#,
            v.name,
            v.address, &v.address,
            status,
            v.blocks_produced,
        ));
    }

    let body = format!(r#"
    <div class="tabs">
        <a class="tab" href="/explorer">Blocks</a>
        <a class="tab active" href="/explorer/validators">Validators</a>
        <a class="tab" href="/explorer/tokens">Tokens</a>
    </div>
    <table>
    <tr><th>Name</th><th>Address</th><th>Status</th><th>Blocks Produced</th></tr>
    {}
    </table>"#, rows);

    page("Validators", &body)
}

// ── Tokens page ──────────────────────────────────────────
pub async fn explorer_tokens(State(state): State<SharedState>) -> Html<String> {
    let bc = state.read().await;
    let tokens = bc.list_tokens();

    let mut rows = String::new();
    for t in &tokens {
        rows.push_str(&format!(
            r#"<tr>
            <td><strong>{}</strong></td>
            <td>{}</td>
            <td class="hash">{}</td>
            <td>{}</td>
            <td>{}</td>
            <td class="mono">{}</td>
            </tr>"#,
            t["symbol"].as_str().unwrap_or(""),
            t["name"].as_str().unwrap_or(""),
            t["contract_address"].as_str().unwrap_or(""),
            t["total_supply"],
            t["holders"],
            t["owner"].as_str().unwrap_or(""),
        ));
    }

    let body = format!(r#"
    <div class="tabs">
        <a class="tab" href="/explorer">Blocks</a>
        <a class="tab" href="/explorer/validators">Validators</a>
        <a class="tab active" href="/explorer/tokens">Tokens</a>
    </div>
    <table>
    <tr><th>Symbol</th><th>Name</th><th>Contract</th><th>Supply</th><th>Holders</th><th>Owner</th></tr>
    {}
    </table>"#, rows);

    page("Tokens", &body)
}
