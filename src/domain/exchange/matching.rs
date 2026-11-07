#![cfg_attr(not(feature = "server"), allow(dead_code))]

//! On-chain reconciliation scoring for exchange ledger rows.
//!
//! The module is pure: callers provide one exchange ledger row and the cached
//! confirmed on-chain transactions, and receive either scored candidates or
//! the result of an automatic match. No database, clock, or network access
//! happens here.
//!
//! # Scoring
//!
//! Each candidate receives a score in `0..=1` for:
//!
//! - an exact hash match;
//! - the broad local balance movement;
//! - the most direction-specific output;
//! - an amount adjusted by the on-chain fee;
//! - timestamp proximity;
//! - the flow direction expected for a withdrawal or deposit;
//! - the transaction shape (CoinJoin and batch transactions are penalized);
//! - whether the candidate is already linked to another ledger row;
//! - ambiguity when the best candidate is close to the second best.
//!
//! An exact hash is definitive and returns a score of `1.0`. All other
//! evidence is combined with the weights in [`OnChainLinkScoringSettings`].
//!
//! # Automatic matching
//!
//! A row is matched automatically in two cases:
//!
//! 1. Its txid exactly matches a cached on-chain txid. Exact hashes may be
//!    shared by several ledger rows because duplicate exports can legitimately
//!    name the same transaction.
//! 2. Exactly one unclaimed candidate has strong amount/date evidence: the
//!    direction must match the exchange transaction type, the best amount
//!    score must reach `MIN_AMOUNT_EVIDENCE`, and the timestamp score must
//!    reach `MIN_TIMESTAMP_EVIDENCE`.
//!
//! Amount/date matching is one-to-one. A transaction claimed by another row is
//! skipped unless either row is marked as an intentional duplicate. A valid
//! txid that is not cached stays unmatched instead of guessing by amount/date.

use std::collections::{HashMap, HashSet};

use crate::domain::civil_time;
use crate::domain::flow::is_spendable_output;

use super::{
    ExchangeTransaction, ExchangeTxType, MatchState, OnChainLink, OnChainLinkScoringSettings,
    OnChainLinkSource,
};

/// Base-score gap that counts as a fully separated best candidate.
const AMBIGUITY_MARGIN: f64 = 0.08;
const SECONDS_PER_HOUR: f64 = 60.0 * 60.0;
/// Candidates beyond this many timestamp tolerances cannot be an amount/date
/// match, so they do not need to be scored.
const TIMESTAMP_CUTOFF_MULTIPLIER: f64 = 3.0;
/// Minimum best-amount score for automatic amount/date matching. The three
/// amount scores are correlated; the best one represents the evidence.
const MIN_AMOUNT_EVIDENCE: f64 = 0.5;
/// Minimum timestamp score for automatic amount/date matching.
const MIN_TIMESTAMP_EVIDENCE: f64 = 0.25;
const DIRECTION_SCORE_FULL: f64 = 1.0;
const DIRECTION_SCORE_PARTIAL: f64 = 0.75;
const COINJOIN_PENALTY: f64 = 0.45;
const COINJOIN_MIN_INPUTS: usize = 5;
const COINJOIN_MIN_OUTPUTS: usize = 5;
const COINJOIN_MIN_EQUAL_OUTPUTS: usize = 3;
const BATCH_MIN_OUTPUTS: usize = 10;
const BATCH_PENALTY: f64 = 0.15;
/// Manual/weak candidates never reach a perfect score.
const MAX_WEAK_LIKELINESS: f64 = 0.999;
const SCORE_SCALE: f64 = 1_000.0;

/// One input of a cached on-chain transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnChainInput {
    /// Value in satoshis. Zero means the previous output value is unknown and
    /// the input is ignored by flow scoring.
    pub value: i64,
    /// Address that owns the spent previous output, when known.
    pub address: Option<String>,
}

/// One output of a cached on-chain transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnChainOutput {
    /// Value in satoshis.
    pub value: i64,
    /// Address that owns the output, when known.
    pub address: Option<String>,
    /// Mempool script type such as `v0_p2wpkh` or `op_return`.
    pub scriptpubkey_type: String,
}

/// Cached on-chain transaction prepared for scoring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnChainTransaction {
    pub txid: String,
    /// Network fee in satoshis.
    pub fee_sats: i64,
    /// Confirmation time as a Unix timestamp in seconds.
    pub block_time: i64,
    pub inputs: Vec<OnChainInput>,
    pub outputs: Vec<OnChainOutput>,
    /// Locally tracked addresses that appear in this transaction.
    pub local_addresses: Vec<String>,
}

/// One scored on-chain candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct OnChainLinkCandidate {
    pub txid: String,
    pub local_addresses: Vec<String>,
    /// Evidence class of the candidate. [`OnChainLinkSource::Manual`] here
    /// means the evidence is too weak for automatic matching; it does not mean
    /// the row was linked manually.
    pub source: OnChainLinkSource,
    /// Rounded confidence in `0..=1`.
    pub likeliness: f64,
    /// Smallest amount difference between the ledger amount and any amount
    /// signal, in satoshis.
    pub amount_delta_sats: Option<i64>,
    /// Absolute distance between the ledger timestamp and the block time.
    pub timestamp_delta_seconds: i64,
    /// Net local movement: received minus spent, in satoshis.
    pub known_movement_sats: i64,
    /// Whether another ledger row already claims this transaction.
    pub is_already_linked: bool,
    /// On-chain network fee in satoshis.
    pub fee_sats: i64,
    /// Confirmation time as a Unix timestamp in seconds.
    pub block_time: i64,
    scores: Scores,
    base_likeliness: f64,
    ambiguity: f64,
}

/// Transactions already claimed by other ledger rows.
///
/// Amount/date auto-matching is one-to-one. A claimed transaction is skipped
/// unless the current row or the row that claimed it is marked as an
/// intentional duplicate. Exact hash matches ignore claims because identical
/// hashes can legitimately describe the same on-chain transaction.
#[derive(Debug, Clone, Default)]
pub struct OnChainClaims {
    claimed: HashSet<String>,
    shareable: HashSet<String>,
}

impl OnChainClaims {
    /// Create an empty claim set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build claims from a plain txid set. The set cannot carry the duplicate
    /// flag, so every claim is treated as non-shareable.
    pub fn from_linked_txids(linked_txids: &HashSet<String>) -> Self {
        Self {
            claimed: linked_txids.clone(),
            shareable: HashSet::new(),
        }
    }

    /// Record that `txid` is linked to another ledger row. `shareable` is
    /// true when the claiming row is an intentional duplicate.
    pub fn claim(&mut self, txid: impl Into<String>, shareable: bool) {
        let txid = txid.into();
        if shareable {
            self.shareable.insert(txid.clone());
        }
        self.claimed.insert(txid);
    }

    fn blocks(&self, txid: &str, allow_shared: bool) -> bool {
        self.claimed.contains(txid) && !allow_shared && !self.shareable.contains(txid)
    }
}

/// Result of matching one exchange row against the cached chain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult {
    /// Reconciliation state to persist.
    pub match_state: MatchState,
    /// Cached on-chain txid selected by the match.
    pub matched_tx_id: Option<String>,
    /// Durable link to persist with the row.
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

    fn hash_match(on_chain: &OnChainTransaction, linked_at: i64) -> Self {
        Self {
            match_state: MatchState::HashMatch,
            matched_tx_id: Some(on_chain.txid.clone()),
            on_chain_link: Some(link_from_transaction(
                on_chain,
                OnChainLinkSource::Hash,
                linked_at,
            )),
        }
    }

    fn amount_date_match(candidate: &OnChainLinkCandidate, linked_at: i64) -> Self {
        Self {
            match_state: MatchState::AmountDateMatch,
            matched_tx_id: Some(candidate.txid.clone()),
            on_chain_link: Some(link_from_candidate(
                candidate,
                OnChainLinkSource::AmountDate,
                linked_at,
            )),
        }
    }
}

/// Only withdrawals and deposits can represent a transfer between the
/// exchange and a locally tracked address.
pub fn is_on_chain_linkable(tx_type: ExchangeTxType) -> bool {
    matches!(
        tx_type,
        ExchangeTxType::Withdrawal | ExchangeTxType::Deposit
    )
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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
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

impl Scores {
    fn amount_evidence(self) -> f64 {
        self.amount
            .max(self.payment_output)
            .max(self.fee_adjusted_amount)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct AmountScores {
    best_delta: Option<i64>,
    amount: f64,
    payment_output: f64,
    fee_adjusted: f64,
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn round_score(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    (clamp01(value) * SCORE_SCALE).round() / SCORE_SCALE
}

fn absolute_sats(value: i64) -> i64 {
    i64::try_from(value.unsigned_abs()).unwrap_or(i64::MAX)
}

fn normalized_txid(tx: &ExchangeTransaction) -> Option<String> {
    tx.tx_hash
        .as_deref()
        .map(str::trim)
        .filter(|hash| !hash.is_empty())
        .map(str::to_ascii_lowercase)
}

/// Bitcoin txids are 64 hexadecimal characters. A non-txid value is treated
/// as an internal identifier and may still be matched by amount/date.
fn looks_like_txid(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn txid_matches(hash: &str, txid: &str) -> bool {
    txid.eq_ignore_ascii_case(hash)
}

fn timestamp_delta_seconds(tx: &ExchangeTransaction, on_chain: &OnChainTransaction) -> i64 {
    let delta = i128::from(on_chain.block_time) - i128::from(tx.timestamp);
    i64::try_from(delta.unsigned_abs()).unwrap_or(i64::MAX)
}

fn timestamp_tolerance_seconds(scoring: &OnChainLinkScoringSettings) -> f64 {
    scoring.timestamp_tolerance_hours * SECONDS_PER_HOUR
}

fn timestamp_cutoff_seconds(scoring: &OnChainLinkScoringSettings) -> f64 {
    timestamp_tolerance_seconds(scoring) * TIMESTAMP_CUTOFF_MULTIPLIER
}

fn flow(tx: &OnChainTransaction) -> Flow {
    let local: HashSet<&str> = tx.local_addresses.iter().map(String::as_str).collect();
    let mut local_received_sats: i64 = 0;
    let mut local_spent_sats: i64 = 0;
    let mut external_output_sats: i64 = 0;
    let mut local_output_values = Vec::new();
    let mut local_input_values = Vec::new();
    let mut external_output_values = Vec::new();

    for output in &tx.outputs {
        if !is_spendable_output(
            output.value,
            &output.scriptpubkey_type,
            output.address.as_deref(),
        ) {
            continue;
        }
        let address = output.address.as_deref().unwrap_or_default();
        if local.contains(address) {
            local_output_values.push(output.value);
            local_received_sats = local_received_sats.saturating_add(output.value);
        } else {
            external_output_values.push(output.value);
            external_output_sats = external_output_sats.saturating_add(output.value);
        }
    }

    for input in &tx.inputs {
        let Some(address) = input.address.as_deref() else {
            continue;
        };
        if input.value <= 0 {
            continue;
        }
        if local.contains(address) {
            local_input_values.push(input.value);
            local_spent_sats = local_spent_sats.saturating_add(input.value);
        }
    }

    Flow {
        local_received_sats,
        local_spent_sats,
        external_output_sats,
        local_output_values,
        local_input_values,
        external_output_values,
        known_movement_sats: local_received_sats.saturating_sub(local_spent_sats),
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
    let mut values = vec![absolute_sats(flow.known_movement_sats)];
    values.extend(flow.local_output_values.iter().copied());
    values.extend(flow.local_input_values.iter().copied());
    values.extend(direction_values(tx_type, flow));
    values.into_iter().filter(|value| *value >= 0).collect()
}

fn best_delta(exchange_amount: i64, values: &[i64]) -> Option<i64> {
    values
        .iter()
        .filter(|value| **value >= 0)
        .map(|value| {
            let delta = (i128::from(*value) - i128::from(exchange_amount)).unsigned_abs();
            i64::try_from(delta).unwrap_or(i64::MAX)
        })
        .min()
}

fn amount_score(delta: Option<i64>, tolerance: f64) -> f64 {
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return 0.0;
    }
    delta.map_or(0.0, |delta| 1.0 - (delta as f64 / tolerance).min(1.0))
}

fn amount_scores(
    tx: &ExchangeTransaction,
    flow: &Flow,
    on_chain_fee_sats: i64,
    scoring: &OnChainLinkScoringSettings,
) -> AmountScores {
    if !super::csv::is_bitcoin_currency(&tx.currency) {
        return AmountScores::default();
    }
    let exchange_amount = absolute_sats(tx.amount_sats);
    if exchange_amount == 0 {
        return AmountScores::default();
    }
    let amount_delta = best_delta(exchange_amount, &comparable_amounts(tx.tx_type, flow));
    let payment_delta = best_delta(exchange_amount, &direction_values(tx.tx_type, flow));
    let mut fee_values = direction_values(tx.tx_type, flow);
    fee_values.push(absolute_sats(flow.known_movement_sats));
    let fee_values = fee_values
        .into_iter()
        .flat_map(|value| {
            [
                value,
                value.saturating_add(on_chain_fee_sats),
                value.saturating_sub(on_chain_fee_sats),
            ]
        })
        .filter(|value| *value >= 0)
        .collect::<Vec<_>>();
    let fee_delta = best_delta(exchange_amount, &fee_values);
    let best_delta = [amount_delta, payment_delta, fee_delta]
        .into_iter()
        .flatten()
        .min();
    AmountScores {
        best_delta,
        amount: amount_score(amount_delta, scoring.amount_tolerance_sats),
        payment_output: amount_score(payment_delta, scoring.amount_tolerance_sats),
        fee_adjusted: amount_score(fee_delta, scoring.amount_tolerance_sats),
    }
}

fn direction_score(tx_type: ExchangeTxType, flow: &Flow) -> f64 {
    match tx_type {
        ExchangeTxType::Withdrawal if flow.local_received_sats > 0 => {
            if flow.known_movement_sats > 0 {
                DIRECTION_SCORE_FULL
            } else {
                DIRECTION_SCORE_PARTIAL
            }
        }
        ExchangeTxType::Deposit if flow.local_spent_sats > 0 => {
            if flow.known_movement_sats < 0 {
                DIRECTION_SCORE_FULL
            } else {
                DIRECTION_SCORE_PARTIAL
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
    let delta = timestamp_delta_seconds(tx, on_chain);
    let tolerance = timestamp_tolerance_seconds(scoring);
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return (delta, 0.0);
    }
    if delta as f64 > tolerance * TIMESTAMP_CUTOFF_MULTIPLIER {
        return (delta, 0.0);
    }
    let ratio = delta as f64 / tolerance;
    (delta, (-0.5 * ratio * ratio).exp())
}

fn tx_shape_score(on_chain: &OnChainTransaction) -> f64 {
    let outputs = on_chain
        .outputs
        .iter()
        .filter(|output| {
            is_spendable_output(
                output.value,
                &output.scriptpubkey_type,
                output.address.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    let mut counts = HashMap::new();
    for output in &outputs {
        *counts.entry(output.value).or_insert(0_usize) += 1;
    }
    let largest = counts.values().copied().max().unwrap_or(0);
    let coinjoin = on_chain.inputs.len() >= COINJOIN_MIN_INPUTS
        && outputs.len() >= COINJOIN_MIN_OUTPUTS
        && largest >= COINJOIN_MIN_EQUAL_OUTPUTS;
    let batch = outputs.len() >= BATCH_MIN_OUTPUTS;
    let mut score = 1.0;
    if coinjoin {
        score -= COINJOIN_PENALTY;
    }
    if batch {
        score -= BATCH_PENALTY;
    }
    clamp01(score)
}

fn weighted_score(
    has_tx_hash: bool,
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
        if applicable && weight.is_finite() && weight > 0.0 {
            let score = if score.is_finite() {
                score.clamp(0.0, 1.0)
            } else {
                0.0
            };
            total += weight * score;
            weight_sum += weight;
        }
    };
    add(scoring.hash_weight, scores.hash, has_tx_hash);
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
    if weight_sum.is_finite() && weight_sum > 0.0 {
        total / weight_sum
    } else {
        0.0
    }
}

fn source(scores: Scores) -> OnChainLinkSource {
    if scores.hash == 1.0 {
        OnChainLinkSource::Hash
    } else if scores.direction > 0.0
        && scores.amount_evidence() >= MIN_AMOUNT_EVIDENCE
        && scores.timestamp >= MIN_TIMESTAMP_EVIDENCE
    {
        OnChainLinkSource::AmountDate
    } else {
        OnChainLinkSource::Manual
    }
}

fn final_likeliness(source: OnChainLinkSource, weighted: f64) -> f64 {
    let rounded = round_score(weighted);
    if source == OnChainLinkSource::Manual {
        rounded.min(MAX_WEAK_LIKELINESS)
    } else {
        rounded
    }
}

fn search_blob(candidate: &OnChainLinkCandidate) -> String {
    let date = civil_time::format_datetime(candidate.block_time);
    format!(
        "{} {} {} {} {} {} {} {} {}",
        candidate.txid,
        date,
        candidate.source.as_str(),
        candidate.likeliness,
        candidate.known_movement_sats,
        candidate.known_movement_sats.unsigned_abs(),
        candidate.fee_sats,
        candidate.amount_delta_sats.unwrap_or_default(),
        candidate.local_addresses.join(" "),
    )
}

fn score_candidate(
    tx: &ExchangeTransaction,
    tx_hash: Option<&str>,
    on_chain: &OnChainTransaction,
    scoring: &OnChainLinkScoringSettings,
    is_linked: bool,
) -> OnChainLinkCandidate {
    let flow = flow(on_chain);
    let hash_score = if tx_hash.is_some_and(|hash| txid_matches(hash, &on_chain.txid)) {
        1.0
    } else {
        0.0
    };
    let amounts = amount_scores(tx, &flow, on_chain.fee_sats, scoring);
    let (timestamp_delta, timestamp) = timestamp_scores(tx, on_chain, scoring);
    let direction = direction_score(tx.tx_type, &flow);
    let shape = tx_shape_score(on_chain);
    let scores = Scores {
        hash: hash_score,
        amount: amounts.amount,
        payment_output: amounts.payment_output,
        fee_adjusted_amount: amounts.fee_adjusted,
        timestamp,
        direction,
        tx_shape: shape,
        unmatched: if is_linked { 0.0 } else { 1.0 },
    };
    let base_likeliness = weighted_score(tx_hash.is_some(), scoring, scores, None);
    let source = source(scores);
    let likeliness = final_likeliness(
        source,
        weighted_score(tx_hash.is_some(), scoring, scores, Some(1.0)),
    );
    OnChainLinkCandidate {
        txid: on_chain.txid.clone(),
        local_addresses: on_chain.local_addresses.clone(),
        source,
        likeliness,
        amount_delta_sats: amounts.best_delta,
        timestamp_delta_seconds: timestamp_delta,
        known_movement_sats: flow.known_movement_sats,
        is_already_linked: is_linked,
        fee_sats: on_chain.fee_sats,
        block_time: on_chain.block_time,
        scores,
        base_likeliness,
        ambiguity: 1.0,
    }
}

fn apply_ambiguity(
    tx: &ExchangeTransaction,
    candidates: &mut [OnChainLinkCandidate],
    scoring: &OnChainLinkScoringSettings,
) {
    let has_tx_hash = tx
        .tx_hash
        .as_deref()
        .map(str::trim)
        .is_some_and(|hash| !hash.is_empty());
    let mut order = (0..candidates.len()).collect::<Vec<_>>();
    order.sort_by(|a, b| {
        candidates[*b]
            .base_likeliness
            .partial_cmp(&candidates[*a].base_likeliness)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let best = order.first().copied();
    let second_score = order
        .get(1)
        .map(|index| candidates[*index].base_likeliness)
        .unwrap_or(0.0);
    let candidate_count = candidates.len();
    for (index, candidate) in candidates.iter_mut().enumerate() {
        let ambiguity = if candidate.scores.hash == 1.0 || candidate_count == 1 {
            1.0
        } else if Some(index) == best {
            clamp01((candidate.base_likeliness - second_score) / AMBIGUITY_MARGIN)
        } else {
            0.0
        };
        let weighted = weighted_score(has_tx_hash, scoring, candidate.scores, Some(ambiguity));
        candidate.ambiguity = ambiguity;
        candidate.likeliness = final_likeliness(candidate.source, weighted);
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
/// likelihood. The search string filters the txid, formatted date, source,
/// likeliness, movement, fee, amount delta, and local addresses.
///
/// Scoring happens before search filtering, so a candidate's confidence does
/// not change while the user narrows the list.
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
    let scoring = scoring.normalized();
    let tx_hash = normalized_txid(tx);
    let mut values = on_chain
        .iter()
        .map(|candidate| {
            score_candidate(
                tx,
                tx_hash.as_deref(),
                candidate,
                &scoring,
                linked_txids.contains(&candidate.txid),
            )
        })
        .collect::<Vec<_>>();
    apply_ambiguity(tx, &mut values, &scoring);
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

fn link_from_transaction(
    on_chain: &OnChainTransaction,
    source: OnChainLinkSource,
    linked_at: i64,
) -> OnChainLink {
    OnChainLink {
        txid: on_chain.txid.clone(),
        local_addresses: on_chain.local_addresses.clone(),
        source,
        linked_at,
    }
}

fn link_from_candidate(
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

/// Match one exchange row automatically.
///
/// Exact hashes take precedence and may be shared by several rows. Otherwise
/// the row is matched only when exactly one unclaimed candidate has strong
/// amount/date evidence. A valid txid that is not in `on_chain` stays
/// unmatched; a non-txid identifier may still fall back to amount/date.
pub fn match_transaction(
    tx: &ExchangeTransaction,
    on_chain: &[OnChainTransaction],
    scoring: &OnChainLinkScoringSettings,
    claims: &OnChainClaims,
    linked_at: i64,
) -> MatchResult {
    if !is_on_chain_linkable(tx.tx_type) {
        return MatchResult::unmatched();
    }
    let scoring = scoring.normalized();
    let tx_hash = normalized_txid(tx);

    if let Some(hash) = tx_hash.as_deref() {
        if let Some(on_chain_tx) = on_chain
            .iter()
            .find(|candidate| txid_matches(hash, &candidate.txid))
        {
            // Exact hashes are authoritative. Several ledger rows may share
            // the same txid when the exchange exported a transaction twice.
            return MatchResult::hash_match(on_chain_tx, linked_at);
        }
        if looks_like_txid(hash) {
            // The row names a real txid that is not cached yet. Do not guess
            // by amount/date; the hash is stronger evidence than a heuristic.
            return MatchResult::unmatched();
        }
    }

    if !super::csv::is_bitcoin_currency(&tx.currency) {
        return MatchResult::unmatched();
    }

    let allow_shared = tx.is_intentional_duplicate;
    let cutoff = timestamp_cutoff_seconds(&scoring);
    let mut matched = None;
    for on_chain_tx in on_chain {
        if claims.blocks(&on_chain_tx.txid, allow_shared) {
            continue;
        }
        if timestamp_delta_seconds(tx, on_chain_tx) as f64 > cutoff {
            continue;
        }
        let candidate = score_candidate(tx, tx_hash.as_deref(), on_chain_tx, &scoring, false);
        if candidate.source != OnChainLinkSource::AmountDate || candidate.base_likeliness <= 0.0 {
            continue;
        }
        if matched.is_some() {
            return MatchResult::unmatched();
        }
        matched = Some(candidate);
    }
    matched.map_or_else(MatchResult::unmatched, |candidate| {
        MatchResult::amount_date_match(&candidate, linked_at)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDRESS: &str = "bc1qtest";
    const EXTERNAL_ADDRESS: &str = "bc1qexternal";
    const BLOCK_TIME: i64 = 1_700_000_000;

    fn output(value: i64, address: &str) -> OnChainOutput {
        OnChainOutput {
            value,
            address: Some(address.into()),
            scriptpubkey_type: "v0_p2wpkh".into(),
        }
    }

    fn input(value: i64, address: &str) -> OnChainInput {
        OnChainInput {
            value,
            address: Some(address.into()),
        }
    }

    fn chain_transaction(
        txid: &str,
        inputs: Vec<OnChainInput>,
        outputs: Vec<OnChainOutput>,
        block_time: i64,
    ) -> OnChainTransaction {
        OnChainTransaction {
            txid: txid.into(),
            fee_sats: 250,
            block_time,
            inputs,
            outputs,
            local_addresses: vec![ADDRESS.into()],
        }
    }

    fn chain_tx(txid: &str, output_value: i64, input_value: i64) -> OnChainTransaction {
        chain_transaction(
            txid,
            if input_value > 0 {
                vec![input(input_value, EXTERNAL_ADDRESS)]
            } else {
                Vec::new()
            },
            vec![output(output_value, ADDRESS)],
            BLOCK_TIME,
        )
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
            converted_fee_amount: None,
            converted_fee_currency: None,
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

    fn match_with(tx: &ExchangeTransaction, on_chain: &[OnChainTransaction]) -> MatchResult {
        match_transaction(
            tx,
            on_chain,
            &OnChainLinkScoringSettings::default(),
            &OnChainClaims::default(),
            7,
        )
    }

    fn zero_weight_scoring() -> OnChainLinkScoringSettings {
        OnChainLinkScoringSettings {
            hash_weight: 0.0,
            amount_weight: 0.0,
            payment_output_weight: 0.0,
            fee_adjusted_amount_weight: 0.0,
            timestamp_weight: 0.0,
            direction_weight: 0.0,
            tx_shape_weight: 0.0,
            unmatched_weight: 0.0,
            ambiguity_weight: 0.0,
            ..OnChainLinkScoringSettings::default()
        }
    }

    #[test]
    fn exact_hash_wins() {
        let chain = chain_tx("match", 100_000, 0);
        let mut tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        tx.tx_hash = Some("match".into());
        let result = match_with(&tx, &[chain]);
        assert_eq!(result.match_state, MatchState::HashMatch);
        assert_eq!(
            result.on_chain_link.unwrap().source,
            OnChainLinkSource::Hash
        );
    }

    #[test]
    fn hash_match_is_case_insensitive() {
        let chain = chain_tx("match", 100_000, 0);
        let mut tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        tx.tx_hash = Some("MATCH".into());
        assert_eq!(match_with(&tx, &[chain]).match_state, MatchState::HashMatch);
    }

    #[test]
    fn exact_hash_may_be_shared_when_claimed() {
        let chain = chain_tx("match", 100_000, 0);
        let mut tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        tx.tx_hash = Some("match".into());
        let mut claims = OnChainClaims::default();
        claims.claim("match", false);
        let result = match_transaction(
            &tx,
            &[chain],
            &OnChainLinkScoringSettings::default(),
            &claims,
            7,
        );
        assert_eq!(result.match_state, MatchState::HashMatch);
    }

    #[test]
    fn cached_hash_is_not_replaced_by_amount_date_guess() {
        let mut tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        tx.tx_hash = Some("a".repeat(64));
        let chain = chain_tx("different", 100_000, 0);
        assert_eq!(match_with(&tx, &[chain]).match_state, MatchState::Unmatched);
    }

    #[test]
    fn non_txid_identifier_can_fall_back_to_amount_date() {
        let mut tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        tx.tx_hash = Some("internal-id".into());
        let chain = chain_tx("amount", 100_000, 0);
        assert_eq!(
            match_with(&tx, &[chain]).match_state,
            MatchState::AmountDateMatch
        );
    }

    #[test]
    fn unique_amount_date_candidate_matches() {
        let chain = chain_tx("amount", 100_250, 0);
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        assert_eq!(
            match_with(&tx, &[chain]).match_state,
            MatchState::AmountDateMatch
        );
    }

    #[test]
    fn zero_amount_rows_do_not_match() {
        let chain = chain_tx("amount", 100_250, 0);
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 0);
        assert_eq!(match_with(&tx, &[chain]).match_state, MatchState::Unmatched);
    }

    #[test]
    fn near_tolerance_amount_stays_unmatched() {
        let chain = chain_tx("amount", 104_500, 0);
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        assert_eq!(match_with(&tx, &[chain]).match_state, MatchState::Unmatched);
    }

    #[test]
    fn distant_timestamp_stays_unmatched() {
        let chain = chain_transaction(
            "amount",
            Vec::new(),
            vec![output(100_250, ADDRESS)],
            BLOCK_TIME + 12 * 24 * 60 * 60,
        );
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        assert_eq!(match_with(&tx, &[chain]).match_state, MatchState::Unmatched);
    }

    #[test]
    fn amount_date_requires_direction_evidence() {
        let chain = chain_transaction(
            "spend",
            vec![input(100_000, ADDRESS)],
            vec![output(100_000, EXTERNAL_ADDRESS)],
            BLOCK_TIME,
        );
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        assert_eq!(match_with(&tx, &[chain]).match_state, MatchState::Unmatched);
    }

    #[test]
    fn fee_adjusted_deposit_matches() {
        let chain = chain_transaction(
            "deposit",
            vec![input(100_250, ADDRESS)],
            vec![output(100_000, EXTERNAL_ADDRESS), output(250, ADDRESS)],
            BLOCK_TIME,
        );
        let tx = exchange_tx(ExchangeTxType::Deposit, 100_000);
        assert_eq!(
            match_with(&tx, &[chain]).match_state,
            MatchState::AmountDateMatch
        );
    }

    #[test]
    fn ambiguous_amount_date_candidates_stay_unmatched() {
        let chain = vec![chain_tx("one", 100_000, 0), chain_tx("two", 100_000, 0)];
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        assert_eq!(match_with(&tx, &chain).match_state, MatchState::Unmatched);
    }

    #[test]
    fn amount_date_respects_claims() {
        let chain = chain_tx("amount", 100_250, 0);
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        let mut claims = OnChainClaims::default();
        claims.claim("amount", false);
        let result = match_transaction(
            &tx,
            &[chain],
            &OnChainLinkScoringSettings::default(),
            &claims,
            7,
        );
        assert_eq!(result.match_state, MatchState::Unmatched);
    }

    #[test]
    fn amount_date_may_share_a_duplicate_claim() {
        let chain = chain_tx("amount", 100_250, 0);
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        let mut claims = OnChainClaims::default();
        claims.claim("amount", true);
        let result = match_transaction(
            &tx,
            &[chain],
            &OnChainLinkScoringSettings::default(),
            &claims,
            7,
        );
        assert_eq!(result.match_state, MatchState::AmountDateMatch);
    }

    #[test]
    fn intentional_duplicate_may_share_an_amount_date_match() {
        let chain = chain_tx("amount", 100_250, 0);
        let mut tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        tx.is_intentional_duplicate = true;
        let mut claims = OnChainClaims::default();
        claims.claim("amount", false);
        let result = match_transaction(
            &tx,
            &[chain],
            &OnChainLinkScoringSettings::default(),
            &claims,
            7,
        );
        assert_eq!(result.match_state, MatchState::AmountDateMatch);
    }

    #[test]
    fn invalid_settings_never_produce_nan_scores() {
        let scoring = OnChainLinkScoringSettings {
            hash_weight: f64::NAN,
            amount_tolerance_sats: 0.0,
            timestamp_tolerance_hours: f64::NAN,
            ..OnChainLinkScoringSettings::default()
        };
        let chain = vec![chain_tx("amount", 100_250, 0)];
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        let values = candidates(&tx, &chain, "", &scoring, &HashSet::new());
        assert_eq!(values.len(), 1);
        let candidate = &values[0];
        assert!(candidate.likeliness.is_finite());
        assert!((0.0..=1.0).contains(&candidate.likeliness));
        assert!(candidate.base_likeliness.is_finite());
        assert!(candidate.ambiguity.is_finite());
        assert!(candidate.scores.amount.is_finite());
        assert!(candidate.scores.timestamp.is_finite());
        assert_eq!(
            match_transaction(&tx, &chain, &scoring, &OnChainClaims::default(), 7).match_state,
            MatchState::AmountDateMatch
        );
    }

    #[test]
    fn zero_weights_disable_amount_date_auto_matching() {
        let chain = chain_tx("amount", 100_250, 0);
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        let result = match_transaction(
            &tx,
            &[chain],
            &zero_weight_scoring(),
            &OnChainClaims::default(),
            7,
        );
        assert_eq!(result.match_state, MatchState::Unmatched);
    }

    #[test]
    fn search_filters_candidates_by_txid_and_address() {
        let chain = vec![
            chain_tx("amount", 100_250, 0),
            chain_tx("other", 200_000, 0),
        ];
        let tx = exchange_tx(ExchangeTxType::Withdrawal, 100_000);
        let by_txid = candidates(
            &tx,
            &chain,
            "AMOUNT",
            &OnChainLinkScoringSettings::default(),
            &HashSet::new(),
        );
        assert_eq!(by_txid.len(), 1);
        assert_eq!(by_txid[0].txid, "amount");
        let by_address = candidates(
            &tx,
            &chain,
            "bc1qtest",
            &OnChainLinkScoringSettings::default(),
            &HashSet::new(),
        );
        assert_eq!(by_address.len(), 2);
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
        assert_eq!(
            match_with(&tx, &[chain_tx("one", 100_000, 0)]).match_state,
            MatchState::Unmatched
        );
    }
}
