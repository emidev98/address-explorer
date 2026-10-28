#![cfg_attr(not(feature = "server"), allow(dead_code))]

//! On-chain reconciliation scoring for exchange ledger rows.
//!
//! The algorithm mirrors UTXO Utils PR #7: exact hashes first, then a
//! weighted evidence score for amount direction, fee-adjusted amounts,
//! timestamp proximity, transaction shape, already-linked penalties, and
//! ambiguity between the best two candidates.

use std::collections::HashSet;

use crate::domain::civil_time;

use super::{
    ExchangeTransaction, ExchangeTxType, MatchState, OnChainLink, OnChainLinkScoringSettings,
    OnChainLinkSource,
};

const AMBIGUITY_MARGIN: f64 = 0.08;
const LINKABLE_TYPES: [ExchangeTxType; 2] = [ExchangeTxType::Withdrawal, ExchangeTxType::Deposit];

/// One input of a cached on-chain transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnChainInput {
    pub value: i64,
    pub address: Option<String>,
}

/// One output of a cached on-chain transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnChainOutput {
    pub value: i64,
    pub address: Option<String>,
    pub scriptpubkey_type: String,
}

/// Cached on-chain transaction prepared for scoring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnChainTransaction {
    pub txid: String,
    pub fee_sats: i64,
    pub block_time: i64,
    pub inputs: Vec<OnChainInput>,
    pub outputs: Vec<OnChainOutput>,
    pub local_addresses: Vec<String>,
}

/// One scored on-chain candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct OnChainLinkCandidate {
    pub txid: String,
    pub local_addresses: Vec<String>,
    pub source: OnChainLinkSource,
    pub likeliness: f64,
    pub base_likeliness: f64,
    pub hash_score: f64,
    pub amount_score: f64,
    pub payment_output_score: f64,
    pub fee_adjusted_amount_score: f64,
    pub timestamp_score: f64,
    pub direction_score: f64,
    pub tx_shape_score: f64,
    pub unmatched_score: f64,
    pub ambiguity_score: f64,
    pub amount_delta_sats: Option<i64>,
    pub timestamp_delta_seconds: i64,
    pub known_movement_sats: i64,
    pub is_already_linked: bool,
    pub fee_sats: i64,
    pub block_time: i64,
}

/// Result of matching one exchange row against the cached chain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult {
    pub match_state: MatchState,
    pub matched_tx_id: Option<String>,
    pub on_chain_link: Option<OnChainLink>,
}

impl MatchResult {
    fn unmatched() -> Self {
        Self {
            match_state: MatchState::Unmatched,
            matched_tx_id: None,
            on_chain_link: None,
        }
    }
}

/// Only withdrawals and deposits can represent a transfer between the
/// exchange and a locally tracked address.
pub fn is_on_chain_linkable(tx_type: ExchangeTxType) -> bool {
    LINKABLE_TYPES.contains(&tx_type)
}

#[derive(Debug, Clone)]
struct Flow {
    local_received_sats: i64,
    local_spent_sats: i64,
    external_output_sats: i64,
    local_output_values: Vec<i64>,
    local_input_values: Vec<i64>,
    external_output_values: Vec<i64>,
    known_movement_sats: i64,
}

#[derive(Debug, Clone, Copy, Default)]
struct Scores {
    hash: f64,
    amount: f64,
    payment_output: f64,
    fee_adjusted_amount: f64,
    timestamp: f64,
    direction: f64,
    tx_shape: f64,
    unmatched: f64,
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn round_score(value: f64) -> f64 {
    (clamp01(value) * 1000.0).round() / 1000.0
}

fn is_spendable(output: &OnChainOutput) -> bool {
    output.value > 0
        && output.scriptpubkey_type != "op_return"
        && output
            .address
            .as_deref()
            .is_some_and(|address| !address.trim().is_empty())
}

fn flow(tx: &OnChainTransaction) -> Flow {
    let local: HashSet<&str> = tx.local_addresses.iter().map(String::as_str).collect();
    let mut local_received_sats = 0;
    let mut local_spent_sats = 0;
    let mut external_output_sats = 0;
    let mut local_output_values = Vec::new();
    let mut local_input_values = Vec::new();
    let mut external_output_values = Vec::new();

    for output in &tx.outputs {
        if !is_spendable(output) {
            continue;
        }
        let address = output.address.as_deref().unwrap_or_default();
        if local.contains(address) {
            local_output_values.push(output.value);
            local_received_sats += output.value;
        } else {
            external_output_values.push(output.value);
            external_output_sats += output.value;
        }
    }

    for input in &tx.inputs {
        let Some(address) = input.address.as_deref() else {
            continue;
        };
        if local.contains(address) {
            local_input_values.push(input.value);
            local_spent_sats += input.value;
        }
    }

    Flow {
        local_received_sats,
        local_spent_sats,
        external_output_sats,
        local_output_values,
        local_input_values,
        external_output_values,
        known_movement_sats: local_received_sats - local_spent_sats,
    }
}

fn direction_values(tx_type: ExchangeTxType, flow: &Flow) -> Vec<i64> {
    let mut values = if tx_type == ExchangeTxType::Deposit {
        vec![flow.external_output_sats]
    } else {
        vec![flow.local_received_sats]
    };
    values.extend(if tx_type == ExchangeTxType::Deposit {
        flow.external_output_values.iter().copied()
    } else {
        flow.local_output_values.iter().copied()
    });
    values
}

fn comparable_amounts(tx_type: ExchangeTxType, flow: &Flow) -> Vec<i64> {
    let mut values = vec![flow.known_movement_sats.abs()];
    values.extend(flow.local_output_values.iter().copied());
    values.extend(flow.local_input_values.iter().copied());
    values.extend(direction_values(tx_type, flow));
    values.into_iter().filter(|value| *value >= 0).collect()
}

fn best_delta(exchange_amount: i64, values: &[i64]) -> Option<i64> {
    values
        .iter()
        .filter(|value| **value >= 0)
        .map(|value| (*value - exchange_amount).unsigned_abs() as i64)
        .min()
}

fn amount_score(delta: Option<i64>, tolerance: f64) -> f64 {
    delta.map_or(0.0, |delta| 1.0 - (delta as f64 / tolerance).min(1.0))
}

fn amount_scores(
    tx: &ExchangeTransaction,
    flow: &Flow,
    on_chain_fee_sats: i64,
    scoring: &OnChainLinkScoringSettings,
) -> (Option<i64>, f64, f64, f64) {
    if !super::csv::is_bitcoin_currency(&tx.currency) {
        return (None, 0.0, 0.0, 0.0);
    }
    let exchange_amount = tx.amount_sats.abs();
    let amount_delta = best_delta(exchange_amount, &comparable_amounts(tx.tx_type, flow));
    let payment_delta = best_delta(exchange_amount, &direction_values(tx.tx_type, flow));
    let mut fee_values = direction_values(tx.tx_type, flow);
    fee_values.push(flow.known_movement_sats.abs());
    let fee_values = fee_values
        .into_iter()
        .flat_map(|value| [value, value + on_chain_fee_sats, value - on_chain_fee_sats])
        .filter(|value| *value >= 0)
        .collect::<Vec<_>>();
    let fee_delta = best_delta(exchange_amount, &fee_values);
    let best = [amount_delta, payment_delta, fee_delta]
        .into_iter()
        .flatten()
        .min();
    (
        best,
        amount_score(amount_delta, scoring.amount_tolerance_sats),
        amount_score(payment_delta, scoring.amount_tolerance_sats),
        amount_score(fee_delta, scoring.amount_tolerance_sats),
    )
}

fn direction_score(tx_type: ExchangeTxType, flow: &Flow) -> f64 {
    match tx_type {
        ExchangeTxType::Withdrawal if flow.local_received_sats > 0 => {
            if flow.known_movement_sats > 0 {
                1.0
            } else {
                0.75
            }
        }
        ExchangeTxType::Deposit if flow.local_spent_sats > 0 => {
            if flow.known_movement_sats < 0 {
                1.0
            } else {
                0.75
            }
        }
        _ => 0.0,
    }
}

fn timestamp_scores(
    tx: &ExchangeTransaction,
    on_chain: &OnChainTransaction,
    scoring: &OnChainLinkScoringSettings,
) -> (i64, f64) {
    let delta = (on_chain.block_time - tx.timestamp).abs();
    let tolerance = scoring.timestamp_tolerance_hours * 60.0 * 60.0;
    if delta as f64 > tolerance * 3.0 {
        return (delta, 0.0);
    }
    let ratio = delta as f64 / tolerance;
    (delta, (-0.5 * ratio * ratio).exp())
}

fn tx_shape_score(on_chain: &OnChainTransaction) -> f64 {
    let outputs = on_chain
        .outputs
        .iter()
        .filter(|output| is_spendable(output))
        .collect::<Vec<_>>();
    let mut counts = std::collections::HashMap::new();
    for output in &outputs {
        *counts.entry(output.value).or_insert(0_usize) += 1;
    }
    let largest = counts.values().copied().max().unwrap_or(0);
    let coinjoin = on_chain.inputs.len() >= 5 && outputs.len() >= 5 && largest >= 3;
    let batch = outputs.len() >= 10;
    let mut score = 1.0;
    if coinjoin {
        score -= 0.45;
    }
    if batch {
        score -= 0.15;
    }
    clamp01(score)
}

fn weighted_score(
    tx: &ExchangeTransaction,
    scoring: &OnChainLinkScoringSettings,
    scores: Scores,
    ambiguity: Option<f64>,
) -> f64 {
    if scores.hash == 1.0 {
        return 1.0;
    }
    let mut total = 0.0;
    let mut weight_sum = 0.0;
    let mut add = |weight: f64, score: f64, applicable: bool| {
        if applicable && weight > 0.0 {
            total += weight * score;
            weight_sum += weight;
        }
    };
    add(scoring.hash_weight, scores.hash, tx.tx_hash.is_some());
    add(scoring.amount_weight, scores.amount, true);
    add(scoring.payment_output_weight, scores.payment_output, true);
    add(
        scoring.fee_adjusted_amount_weight,
        scores.fee_adjusted_amount,
        true,
    );
    add(scoring.timestamp_weight, scores.timestamp, true);
    add(scoring.direction_weight, scores.direction, true);
    add(scoring.tx_shape_weight, scores.tx_shape, true);
    add(scoring.unmatched_weight, scores.unmatched, true);
    add(
        scoring.ambiguity_weight,
        ambiguity.unwrap_or(0.0),
        ambiguity.is_some(),
    );
    if weight_sum > 0.0 {
        total / weight_sum
    } else {
        0.0
    }
}

fn source(hash_score: f64, amount_evidence: f64, timestamp_score: f64) -> OnChainLinkSource {
    if hash_score == 1.0 {
        OnChainLinkSource::Hash
    } else if amount_evidence > 0.0 && timestamp_score > 0.0 {
        OnChainLinkSource::AmountDate
    } else {
        OnChainLinkSource::Manual
    }
}

fn search_blob(candidate: &OnChainLinkCandidate) -> String {
    let date = civil_time::format_datetime(candidate.block_time);
    let addresses = candidate.local_addresses.join(" ");
    format!(
        "{} {} {} {} {} {} {} {}",
        candidate.txid,
        date,
        candidate.source.as_str(),
        candidate.likeliness,
        candidate.known_movement_sats,
        candidate.known_movement_sats.abs(),
        candidate.fee_sats,
        candidate.amount_delta_sats.unwrap_or_default(),
    ) + " "
        + &addresses
}

fn score_candidate(
    tx: &ExchangeTransaction,
    on_chain: &OnChainTransaction,
    scoring: &OnChainLinkScoringSettings,
    linked: &HashSet<String>,
) -> OnChainLinkCandidate {
    let flow = flow(on_chain);
    let hash_score = if tx.tx_hash.as_deref() == Some(&on_chain.txid) {
        1.0
    } else {
        0.0
    };
    let (amount_delta, amount, payment_output, fee_adjusted_amount) =
        amount_scores(tx, &flow, on_chain.fee_sats, scoring);
    let (timestamp_delta, timestamp) = timestamp_scores(tx, on_chain, scoring);
    let direction = direction_score(tx.tx_type, &flow);
    let shape = tx_shape_score(on_chain);
    let is_linked = linked.contains(&on_chain.txid);
    let scores = Scores {
        hash: hash_score,
        amount,
        payment_output,
        fee_adjusted_amount,
        timestamp,
        direction,
        tx_shape: shape,
        unmatched: if is_linked { 0.0 } else { 1.0 },
    };
    let base = round_score(weighted_score(tx, scoring, scores, None));
    let evidence = amount.max(payment_output).max(fee_adjusted_amount);
    let source = source(hash_score, evidence, timestamp);
    let likeliness = if source == OnChainLinkSource::Manual {
        base.min(0.999)
    } else {
        base
    };
    OnChainLinkCandidate {
        txid: on_chain.txid.clone(),
        local_addresses: on_chain.local_addresses.clone(),
        source,
        likeliness,
        base_likeliness: base,
        hash_score,
        amount_score: round_score(amount),
        payment_output_score: round_score(payment_output),
        fee_adjusted_amount_score: round_score(fee_adjusted_amount),
        timestamp_score: round_score(timestamp),
        direction_score: round_score(direction),
        tx_shape_score: round_score(shape),
        unmatched_score: if is_linked { 0.0 } else { 1.0 },
        ambiguity_score: 1.0,
        amount_delta_sats: amount_delta,
        timestamp_delta_seconds: timestamp_delta,
        known_movement_sats: flow.known_movement_sats,
        is_already_linked: is_linked,
        fee_sats: on_chain.fee_sats,
        block_time: on_chain.block_time,
    }
}

fn apply_ambiguity(
    tx: &ExchangeTransaction,
    candidates: &mut [OnChainLinkCandidate],
    scoring: &OnChainLinkScoringSettings,
) {
    let mut order = (0..candidates.len()).collect::<Vec<_>>();
    order.sort_by(|a, b| {
        candidates[*b]
            .base_likeliness
            .partial_cmp(&candidates[*a].base_likeliness)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let best = order.first().copied();
    let second = order.get(1).copied();
    let second_score = second
        .map(|index| candidates[index].base_likeliness)
        .unwrap_or(0.0);
    let candidate_count = candidates.len();
    for (index, candidate) in candidates.iter_mut().enumerate() {
        let ambiguity = if candidate.hash_score == 1.0 || candidate_count == 1 {
            1.0
        } else if Some(index) == best {
            clamp01((candidate.base_likeliness - second_score) / AMBIGUITY_MARGIN)
        } else {
            0.0
        };
        let scores = Scores {
            hash: candidate.hash_score,
            amount: candidate.amount_score,
            payment_output: candidate.payment_output_score,
            fee_adjusted_amount: candidate.fee_adjusted_amount_score,
            timestamp: candidate.timestamp_score,
            direction: candidate.direction_score,
            tx_shape: candidate.tx_shape_score,
            unmatched: candidate.unmatched_score,
        };
        let weighted = round_score(weighted_score(tx, scoring, scores, Some(ambiguity)));
        candidate.ambiguity_score = round_score(ambiguity);
        candidate.likeliness = if candidate.source == OnChainLinkSource::Manual {
            weighted.min(0.999)
        } else {
            weighted
        };
    }
}

fn sort_candidates(a: &OnChainLinkCandidate, b: &OnChainLinkCandidate) -> std::cmp::Ordering {
    b.likeliness
        .partial_cmp(&a.likeliness)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.is_already_linked.cmp(&b.is_already_linked))
        .then_with(|| {
            let rank = |source| match source {
                OnChainLinkSource::Hash => 0,
                OnChainLinkSource::AmountDate => 1,
                OnChainLinkSource::Manual => 2,
            };
            rank(a.source).cmp(&rank(b.source))
        })
        .then_with(|| a.timestamp_delta_seconds.cmp(&b.timestamp_delta_seconds))
        .then_with(|| {
            a.amount_delta_sats
                .unwrap_or(i64::MAX)
                .cmp(&b.amount_delta_sats.unwrap_or(i64::MAX))
        })
        .then_with(|| b.block_time.cmp(&a.block_time))
}

/// Score every cached on-chain transaction and return candidates ordered by
/// likelihood. The search string filters the same fields as UTXO Utils.
pub fn candidates(
    tx: &ExchangeTransaction,
    on_chain: &[OnChainTransaction],
    search: &str,
    scoring: &OnChainLinkScoringSettings,
    linked_txids: &HashSet<String>,
) -> Vec<OnChainLinkCandidate> {
    if !is_on_chain_linkable(tx.tx_type) {
        return Vec::new();
    }
    let mut values = on_chain
        .iter()
        .map(|candidate| score_candidate(tx, candidate, scoring, linked_txids))
        .collect::<Vec<_>>();
    apply_ambiguity(tx, &mut values, scoring);
    if !search.trim().is_empty() {
        let needle = search.trim().to_ascii_lowercase();
        values.retain(|candidate| {
            search_blob(candidate)
                .to_ascii_lowercase()
                .contains(&needle)
        });
    }
    values.sort_by(sort_candidates);
    values
}

fn link_from(
    candidate: &OnChainLinkCandidate,
    source: OnChainLinkSource,
    linked_at: i64,
) -> OnChainLink {
    OnChainLink {
        txid: candidate.txid.clone(),
        local_addresses: candidate.local_addresses.clone(),
        source,
        linked_at,
    }
}

/// Try hash matching first, then a unique amount/date candidate.
pub fn match_transaction(
    tx: &ExchangeTransaction,
    on_chain: &[OnChainTransaction],
    scoring: &OnChainLinkScoringSettings,
    linked_txids: &HashSet<String>,
    linked_at: i64,
) -> MatchResult {
    if !is_on_chain_linkable(tx.tx_type) {
        return MatchResult::unmatched();
    }
    let all = candidates(tx, on_chain, "", scoring, linked_txids);
    if let Some(tx_hash) = tx.tx_hash.as_deref() {
        if let Some(candidate) = all.iter().find(|candidate| candidate.txid == tx_hash) {
            return MatchResult {
                match_state: MatchState::HashMatch,
                matched_tx_id: Some(candidate.txid.clone()),
                on_chain_link: Some(link_from(candidate, OnChainLinkSource::Hash, linked_at)),
            };
        }
    }
    if !super::csv::is_bitcoin_currency(&tx.currency) {
        return MatchResult::unmatched();
    }
    let amount_date = all
        .into_iter()
        .filter(|candidate| candidate.source == OnChainLinkSource::AmountDate)
        .filter(|candidate| {
            candidate
                .amount_score
                .max(candidate.payment_output_score)
                .max(candidate.fee_adjusted_amount_score)
                > 0.0
        })
        .collect::<Vec<_>>();
    if amount_date.len() == 1 {
        let candidate = &amount_date[0];
        return MatchResult {
            match_state: MatchState::AmountDateMatch,
            matched_tx_id: Some(candidate.txid.clone()),
            on_chain_link: Some(link_from(
                candidate,
                OnChainLinkSource::AmountDate,
                linked_at,
            )),
        };
    }
    MatchResult::unmatched()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDRESS: &str = "bc1qtest";
    const BLOCK_TIME: i64 = 1_700_000_000;

    fn chain_tx(txid: &str, output_value: i64, input_value: i64) -> OnChainTransaction {
        OnChainTransaction {
            txid: txid.into(),
            fee_sats: 250,
            block_time: BLOCK_TIME,
            inputs: if input_value > 0 {
                vec![OnChainInput {
                    value: input_value,
                    address: Some("bc1qother".into()),
                }]
            } else {
                Vec::new()
            },
            outputs: vec![OnChainOutput {
                value: output_value,
                address: Some(ADDRESS.into()),
                scriptpubkey_type: "v0_p2wpkh".into(),
            }],
            local_addresses: vec![ADDRESS.into()],
        }
    }

    fn exchange_tx(tx_type: ExchangeTxType, amount_sats: i64) -> ExchangeTransaction {
        ExchangeTransaction {
            id: 1,
            exchange_id: 1,
            fingerprint: "fingerprint".into(),
            timestamp: BLOCK_TIME,
            tx_type,
            amount_sats,
            currency: "BTC".into(),
            fiat_amount: None,
            fiat_currency: None,
            converted_fiat_amount: None,
            converted_fiat_currency: None,
            fee: None,
            fee_currency: None,
            tx_hash: None,
            description: None,
            raw_data: Default::default(),
            match_state: MatchState::Unmatched,
            matched_tx_id: None,
            on_chain_link: None,
            is_manually_edited: false,
            is_intentional_duplicate: false,
        }
    }

    #[test]
    fn exact_hash_wins() {
        let chain = chain_tx("match", 100_000, 0);
        let mut tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        tx.tx_hash = Some("match".into());
        let result = match_transaction(
            &tx,
            &[chain],
            &OnChainLinkScoringSettings::default(),
            &HashSet::new(),
            7,
        );
        assert_eq!(result.match_state, MatchState::HashMatch);
        assert_eq!(
            result.on_chain_link.unwrap().source,
            OnChainLinkSource::Hash
        );
    }

    #[test]
    fn unique_amount_date_candidate_matches() {
        let chain = OnChainTransaction {
            txid: "amount".into(),
            fee_sats: 250,
            block_time: BLOCK_TIME,
            inputs: Vec::new(),
            outputs: vec![OnChainOutput {
                value: 104_500,
                address: Some(ADDRESS.into()),
                scriptpubkey_type: "v0_p2wpkh".into(),
            }],
            local_addresses: vec![ADDRESS.into()],
        };
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        let result = match_transaction(
            &tx,
            &[chain],
            &OnChainLinkScoringSettings::default(),
            &HashSet::new(),
            8,
        );
        assert_eq!(result.match_state, MatchState::AmountDateMatch);
    }

    #[test]
    fn ambiguous_amount_date_candidates_stay_unmatched() {
        let chain = vec![chain_tx("one", 100_000, 0), chain_tx("two", 100_000, 0)];
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        let result = match_transaction(
            &tx,
            &chain,
            &OnChainLinkScoringSettings::default(),
            &HashSet::new(),
            9,
        );
        assert_eq!(result.match_state, MatchState::Unmatched);
    }

    #[test]
    fn buy_rows_are_not_linkable() {
        let tx = exchange_tx(ExchangeTxType::Buy, 100_000);
        assert!(candidates(
            &tx,
            &[chain_tx("one", 100_000, 0)],
            "",
            &OnChainLinkScoringSettings::default(),
            &HashSet::new()
        )
        .is_empty());
    }
}
