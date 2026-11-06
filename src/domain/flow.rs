//! Funds-flow domain: filters, graph value types, and the pure builders behind
//! the Flow screen.
//!
//! Two graph shapes are produced from the same filtered dataset:
//!
//! - the exact transaction view is a time-expanded UTXO graph. A transaction
//!   node consumes output nodes created by earlier transactions and produces
//!   new output nodes. Because every output node is created once and only later
//!   consumed, the graph is acyclic by construction;
//! - the address view is a compact summary that aggregates values, nets
//!   opposing flows, and retains the largest acyclic subset. It reports the
//!   value excluded from circular flows instead of hiding it.
//!
//! The module contains no SQL, HTTP, RSX, or Dioxus code.
#![cfg_attr(not(feature = "server"), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::domain::exchange::{ExchangeTxType, MatchState};

/// Number of external counterparties kept as individual nodes on each side of
/// a flow. The remaining value is folded into a labelled bucket.
pub const TOP_EXTERNAL_NODES: usize = 12;

/// Transaction-view budget. Above these counts the service renders the address
/// summary unless the caller explicitly forces the transaction view.
pub const MAX_TRANSACTION_GRAPH_TRANSACTIONS: usize = 1_500;
pub const MAX_TRANSACTION_GRAPH_OUTPUTS: usize = 3_000;
pub const MAX_TRANSACTION_GRAPH_NODES: usize = 4_000;
pub const MAX_TRANSACTION_GRAPH_LINKS: usize = 12_000;

/// Safeguards for a single filter request; client input is rejected before it
/// can build an unbounded SQL parameter list.
pub const MAX_FILTER_VALUES: usize = 2_000;

/// Which graph shape the screen asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlowView {
    /// Exact time-expanded transaction graph.
    #[default]
    Transactions,
    /// Individual stored-address summary.
    Addresses,
}

impl FlowView {
    pub fn label(self) -> &'static str {
        match self {
            Self::Transactions => "Transactions",
            Self::Addresses => "Addresses",
        }
    }

    pub fn is_summary(self) -> bool {
        !matches!(self, Self::Transactions)
    }
}

/// Query of the flow screen.
///
/// Exchange-provider filters select exchange-owned addresses plus matching
/// ledger accounts. An explicit address selection takes precedence over that
/// exchange focus for the on-chain population.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FlowQuery {
    /// Exchange provider ids (`exchanges_types.id`).
    pub exchange_ids: Vec<i64>,
    /// Explicitly selected stored address strings.
    pub addresses: Vec<String>,
    /// Inclusive epoch-second range of `transactions.block_time`.
    pub from_time: Option<i64>,
    /// Inclusive epoch-second range of `transactions.block_time`.
    pub to_time: Option<i64>,
    pub view: FlowView,
    /// Set when the user explicitly overrides the automatic summary fallback.
    pub force_transactions: bool,
}

impl FlowQuery {
    /// Whether any owner or address filter is active.
    pub fn has_population_filter(&self) -> bool {
        !self.exchange_ids.is_empty() || !self.addresses.is_empty()
    }

    /// Whether an exchange-side filter is active. Ledger rows are limited to
    /// those providers when this is true.
    pub fn has_exchange_filter(&self) -> bool {
        !self.exchange_ids.is_empty()
    }
}

/// One selectable catalog value of the filter bar.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FlowFilterOption {
    pub id: i64,
    pub name: String,
    pub brand_color: Option<String>,
    pub address_count: i64,
    pub account_count: i64,
}

/// One selectable stored address of the filter bar.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FlowAddressOption {
    pub address: String,
    pub description: Option<String>,
    pub exchange_id: Option<i64>,
    pub exchange_name: Option<String>,
    pub exchange_color: Option<String>,
    pub wallet_id: Option<i64>,
    pub wallet_name: Option<String>,
    pub wallet_color: Option<String>,
    pub balance_sats: i64,
    pub confirmed_tx_count: i64,
    pub tx_index_rate_limited: bool,
}

/// First and last stored timestamps available to the date filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FlowTimeBounds {
    pub first_tx_time: Option<i64>,
    pub last_tx_time: Option<i64>,
    pub first_ledger_time: Option<i64>,
    pub last_ledger_time: Option<i64>,
}

/// Everything the filter bar needs in one response.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FlowFilterOptions {
    pub exchanges: Vec<FlowFilterOption>,
    pub addresses: Vec<FlowAddressOption>,
    pub time_bounds: FlowTimeBounds,
}

/// Which provider a flow node belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowOwnerKind {
    Exchange,
    Wallet,
}

/// Presentation metadata of an owner, shared by exchange/wallet nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowOwnerRef {
    pub kind: FlowOwnerKind,
    /// `None` for a manual exchange account without a catalog provider.
    pub id: Option<i64>,
    pub name: String,
    pub brand_color: Option<String>,
}

/// Persisted address metadata used by the graph builders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FlowAddressOwner {
    pub address: String,
    pub description: Option<String>,
    pub exchange_id: Option<i64>,
    pub exchange_name: Option<String>,
    pub exchange_color: Option<String>,
    pub wallet_id: Option<i64>,
    pub wallet_name: Option<String>,
    pub wallet_color: Option<String>,
    /// Spendable balance in satoshis: the exchange ledger anchor for exchange
    /// addresses, the on-chain funded-minus-spent amount otherwise.
    pub balance_sats: i64,
    pub confirmed_tx_count: i64,
    pub tx_index_rate_limited: bool,
}

impl FlowAddressOwner {
    pub fn owner_ref(&self) -> Option<FlowOwnerRef> {
        if let Some(id) = self.exchange_id {
            return Some(FlowOwnerRef {
                kind: FlowOwnerKind::Exchange,
                id: Some(id),
                name: self
                    .exchange_name
                    .clone()
                    .unwrap_or_else(|| format!("Exchange #{id}")),
                brand_color: self.exchange_color.clone(),
            });
        }
        if let Some(id) = self.wallet_id {
            return Some(FlowOwnerRef {
                kind: FlowOwnerKind::Wallet,
                id: Some(id),
                name: self
                    .wallet_name
                    .clone()
                    .unwrap_or_else(|| format!("Wallet #{id}")),
                brand_color: self.wallet_color.clone(),
            });
        }
        None
    }

    pub fn display_label(&self) -> String {
        self.description
            .clone()
            .unwrap_or_else(|| short_text(&self.address))
    }
}

/// One input of a filtered transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowInput {
    pub prev_txid: String,
    pub prev_vout: i64,
    pub address: Option<String>,
    pub value_sats: i64,
    pub is_coinbase: bool,
}

/// One output of a filtered transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowOutput {
    pub output_index: i64,
    pub address: Option<String>,
    pub value_sats: i64,
    pub scriptpubkey_type: String,
}

impl FlowOutput {
    /// Whether the output can carry value into the graph.
    pub fn is_spendable(&self) -> bool {
        self.value_sats > 0
            && self.scriptpubkey_type != "op_return"
            && self
                .address
                .as_deref()
                .is_some_and(|address| !address.trim().is_empty())
    }
}

/// A deduplicated on-chain transaction prepared for graph construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowTransaction {
    pub txid: String,
    pub block_time: i64,
    pub fee_sats: i64,
    pub inputs: Vec<FlowInput>,
    pub outputs: Vec<FlowOutput>,
}

/// One exchange ledger row in the filtered dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowLedgerRow {
    pub id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub provider_id: Option<i64>,
    pub provider_name: Option<String>,
    pub provider_color: Option<String>,
    pub timestamp: i64,
    pub tx_type: ExchangeTxType,
    pub amount_sats: i64,
    pub match_state: MatchState,
    pub on_chain_txid: Option<String>,
    pub local_addresses: Vec<String>,
}

impl FlowLedgerRow {
    pub fn is_deposit_or_withdrawal(&self) -> bool {
        matches!(
            self.tx_type,
            ExchangeTxType::Deposit | ExchangeTxType::Withdrawal
        )
    }

    pub fn is_unmatched(&self) -> bool {
        self.match_state == MatchState::Unmatched
    }

    /// Stable endpoint identity of the exchange side, even for a manual
    /// account without a catalog provider.
    pub fn exchange_endpoint_id(&self) -> String {
        match self.provider_id {
            Some(id) => format!("exchange:{id}"),
            None => format!("exchange-account:{}", self.account_id),
        }
    }

    pub fn exchange_endpoint_label(&self) -> String {
        self.provider_name
            .clone()
            .unwrap_or_else(|| self.account_name.clone())
    }

    pub fn exchange_endpoint_owner(&self) -> FlowOwnerRef {
        FlowOwnerRef {
            kind: FlowOwnerKind::Exchange,
            id: self.provider_id,
            name: self.exchange_endpoint_label(),
            brand_color: self.provider_color.clone(),
        }
    }
}

/// The filtered source data of one graph request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FlowDataset {
    pub transactions: Vec<FlowTransaction>,
    pub ledger: Vec<FlowLedgerRow>,
    pub owners: BTreeMap<String, FlowAddressOwner>,
    pub focus_addresses: Vec<String>,
}

impl FlowDataset {
    pub fn output_count(&self) -> usize {
        self.transactions
            .iter()
            .flat_map(|transaction| &transaction.outputs)
            .filter(|output| output.is_spendable())
            .count()
    }

    pub fn gross_output_sats(&self) -> i64 {
        self.transactions
            .iter()
            .flat_map(|transaction| &transaction.outputs)
            .filter(|output| output.is_spendable())
            .map(|output| output.value_sats)
            .sum()
    }

    pub fn fee_sats(&self) -> i64 {
        self.transactions
            .iter()
            .map(|transaction| transaction.fee_sats.max(0))
            .sum()
    }

    pub fn focus_balance_sats(&self) -> i64 {
        self.focus_addresses
            .iter()
            .filter_map(|address| self.owners.get(address))
            .map(|owner| owner.balance_sats)
            .sum()
    }

    pub fn incomplete_address_count(&self) -> i64 {
        self.focus_addresses
            .iter()
            .filter_map(|address| self.owners.get(address))
            .filter(|owner| owner.tx_index_rate_limited)
            .count() as i64
    }
}

/// Exact size information used by the automatic view fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FlowGraphStats {
    pub node_count: usize,
    pub link_count: usize,
    pub transaction_count: usize,
    pub output_count: usize,
}

/// Render budget of the transaction view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowRenderBudget {
    pub max_transactions: usize,
    pub max_outputs: usize,
    pub max_nodes: usize,
    pub max_links: usize,
    pub exceeded: bool,
}

impl Default for FlowRenderBudget {
    fn default() -> Self {
        Self {
            max_transactions: MAX_TRANSACTION_GRAPH_TRANSACTIONS,
            max_outputs: MAX_TRANSACTION_GRAPH_OUTPUTS,
            max_nodes: MAX_TRANSACTION_GRAPH_NODES,
            max_links: MAX_TRANSACTION_GRAPH_LINKS,
            exceeded: false,
        }
    }
}

/// Kind of a Sankey node; the chart maps this to a color role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowNodeKind {
    Transaction,
    Exchange,
    Wallet,
    Address,
    ExternalSource,
    ExternalDestination,
    /// A single external address in a summary view, where it can both send
    /// and receive. The transaction view keeps source and destination
    /// occurrences separate to preserve its time-expanded acyclicity.
    External,
    Coinbase,
    Unlinked,
}

/// One Sankey node with the metadata used by the chart and evidence views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowNode {
    /// Stable internal id; also used as the ECharts node name.
    pub id: String,
    pub label: String,
    pub kind: FlowNodeKind,
    pub owner: Option<FlowOwnerRef>,
    pub address: Option<String>,
    pub address_count: i64,
    pub transaction_count: i64,
    pub in_sats: i64,
    pub out_sats: i64,
    pub balance_sats: Option<i64>,
    pub first_time: Option<i64>,
    pub last_time: Option<i64>,
    /// Ledger annotation such as `Withdrawal · Binance`.
    pub note: Option<String>,
}

impl FlowNode {
    fn new(id: String, label: String, kind: FlowNodeKind) -> Self {
        Self {
            id,
            label,
            kind,
            owner: None,
            address: None,
            address_count: 1,
            transaction_count: 0,
            in_sats: 0,
            out_sats: 0,
            balance_sats: None,
            first_time: None,
            last_time: None,
            note: None,
        }
    }

    fn with_owner(mut self, owner: Option<FlowOwnerRef>) -> Self {
        self.owner = owner;
        self
    }

    fn with_address(mut self, address: Option<String>) -> Self {
        self.address = address;
        self
    }

    pub fn residual_sats(&self) -> i64 {
        (self.in_sats - self.out_sats).max(0)
    }
}

/// Whether a link carries exact on-chain movement, an unmatched ledger flow,
/// or an aggregated summary value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowLinkKind {
    OnChain,
    LedgerUnlinked,
    Aggregated,
}

/// One contribution behind an aggregated link. The evidence table resolves
/// these server-side; they are omitted from the chart payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowEvidenceRef {
    pub txid: Option<String>,
    pub ledger_id: Option<i64>,
    pub timestamp: Option<i64>,
    pub value_sats: i64,
    pub description: String,
}

/// One Sankey link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowLink {
    pub source: String,
    pub target: String,
    pub value_sats: i64,
    pub event_count: i64,
    pub first_time: Option<i64>,
    pub last_time: Option<i64>,
    pub kind: FlowLinkKind,
    #[serde(skip)]
    pub evidence: Vec<FlowEvidenceRef>,
}

/// KPI totals of the rendered graph.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FlowKpis {
    pub address_count: i64,
    pub transaction_count: i64,
    pub gross_output_sats: i64,
    pub fee_sats: i64,
    pub current_balance_sats: i64,
    pub origin_count: i64,
    pub destination_count: i64,
    pub internal_sats: i64,
    pub unmatched_ledger_sats: i64,
    pub unmatched_ledger_count: i64,
    pub incomplete_address_count: i64,
}

/// Exchange-ledger activity shown beside the chart. Buys, sells and rewards
/// never became on-chain edges in v1, so they are summarised here.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FlowExchangeActivity {
    pub bought_sats: i64,
    pub sold_sats: i64,
    pub rewarded_sats: i64,
    pub withdrawn_sats: i64,
    pub deposited_sats: i64,
    pub net_ledger_sats: i64,
    pub unmatched_sats: i64,
    pub unmatched_count: i64,
}

/// Structured warning category rendered as a banner by the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowWarningKind {
    IncompleteHistory,
    UnmatchedLedger,
    AutoFallback,
    ExcludedCircular,
    LargeGraph,
}

/// One warning produced by the builder or service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowWarning {
    pub kind: FlowWarningKind,
    pub message: String,
    pub count: i64,
    pub value_sats: i64,
}

/// The complete graph returned to the chart and evidence endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowGraph {
    pub nodes: Vec<FlowNode>,
    pub links: Vec<FlowLink>,
    pub stats: FlowGraphStats,
    pub kpis: FlowKpis,
    pub exchange_activity: FlowExchangeActivity,
    pub warnings: Vec<FlowWarning>,
}

impl FlowGraph {
    pub fn total_link_sats(&self) -> i64 {
        self.links.iter().map(|link| link.value_sats).sum()
    }
}

/// Response of the overview endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowOverview {
    pub requested_view: FlowView,
    pub rendered_view: FlowView,
    pub graph: FlowGraph,
    pub budget: FlowRenderBudget,
    pub auto_fallback: bool,
}

/// One aggregated row of the flow evidence table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowLinkRow {
    pub source_id: String,
    pub target_id: String,
    pub source_label: String,
    pub target_label: String,
    pub source_kind: FlowNodeKind,
    pub target_kind: FlowNodeKind,
    pub source_color: Option<String>,
    pub target_color: Option<String>,
    pub value_sats: i64,
    pub event_count: i64,
    pub first_time: Option<i64>,
    pub last_time: Option<i64>,
    pub kind: FlowLinkKind,
    /// Share of the rendered link total, in percent.
    pub share: f64,
}

/// One contributing transaction or ledger row behind a flow link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowEvidenceRow {
    pub txid: Option<String>,
    pub ledger_id: Option<i64>,
    pub timestamp: Option<i64>,
    pub value_sats: i64,
    pub description: String,
}

/// Query of the aggregated-link evidence table. It flattens the shared table
/// contract and carries the flow filters.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FlowLinkTableQuery {
    #[serde(flatten)]
    pub flow: FlowQuery,
    #[serde(flatten)]
    pub table: crate::domain::table::TableQuery,
}

/// Query of the detailed evidence behind one aggregated link.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FlowEvidenceQuery {
    #[serde(flatten)]
    pub flow: FlowQuery,
    pub source_id: String,
    pub target_id: String,
    #[serde(flatten)]
    pub table: crate::domain::table::TableQuery,
}

/// A link while it is being aggregated.
#[derive(Debug, Clone)]
struct LinkAccumulator {
    value_sats: i64,
    event_count: i64,
    first_time: Option<i64>,
    last_time: Option<i64>,
    kind: FlowLinkKind,
    evidence: Vec<FlowEvidenceRef>,
}

impl LinkAccumulator {
    fn new(kind: FlowLinkKind) -> Self {
        Self {
            value_sats: 0,
            event_count: 0,
            first_time: None,
            last_time: None,
            kind,
            evidence: Vec::new(),
        }
    }

    fn add(&mut self, value_sats: i64, timestamp: Option<i64>, evidence: FlowEvidenceRef) {
        if value_sats <= 0 {
            return;
        }
        self.value_sats += value_sats;
        self.event_count += 1;
        self.first_time = min_time(self.first_time, timestamp);
        self.last_time = max_time(self.last_time, timestamp);
        self.evidence.push(evidence);
    }

    fn merge_kind(&mut self, kind: FlowLinkKind) {
        if self.kind != kind {
            self.kind = FlowLinkKind::Aggregated;
        }
    }
}

/// Build the exact time-expanded transaction graph.
pub fn build_transaction_graph(dataset: &FlowDataset) -> FlowGraph {
    let mut transactions: Vec<&FlowTransaction> = dataset.transactions.iter().collect();
    transactions.sort_by(|left, right| {
        left.block_time
            .cmp(&right.block_time)
            .then_with(|| left.txid.cmp(&right.txid))
    });

    let ledger_by_txid = ledger_by_txid(&dataset.ledger);
    let withdrawal_endpoint_by_txid = withdrawal_endpoint_by_txid(&dataset.ledger);

    let spent_outpoints: HashSet<(String, i64)> = transactions
        .iter()
        .flat_map(|transaction| transaction.inputs.iter())
        .filter(|input| !input.is_coinbase)
        .map(|input| (input.prev_txid.clone(), input.prev_vout))
        .collect();

    let output_targets = transaction_output_targets(
        &transactions,
        &dataset.owners,
        &spent_outpoints,
        TOP_EXTERNAL_NODES,
    );

    let mut external_source_totals: BTreeMap<String, i64> = BTreeMap::new();
    for transaction in &transactions {
        let withdraws = withdrawal_endpoint_by_txid.contains_key(&transaction.txid);
        for input in &transaction.inputs {
            if input.is_coinbase || withdraws {
                continue;
            }
            if output_targets
                .outpoint_nodes
                .contains_key(&(input.prev_txid.clone(), input.prev_vout))
            {
                continue;
            }
            let Some(address) = input.address.as_deref() else {
                continue;
            };
            if dataset.owners.contains_key(address) {
                continue;
            }
            *external_source_totals
                .entry(address.to_string())
                .or_default() += input.value_sats.max(0);
        }
    }
    let source_keep = top_keys(&external_source_totals, TOP_EXTERNAL_NODES);

    let mut nodes: BTreeMap<String, FlowNode> = BTreeMap::new();
    let mut links: BTreeMap<(String, String), LinkAccumulator> = BTreeMap::new();

    for transaction in &transactions {
        let tx_node_id = transaction_node_id(&transaction.txid);
        let mut tx_node = FlowNode::new(
            tx_node_id.clone(),
            short_text(&transaction.txid),
            FlowNodeKind::Transaction,
        );
        tx_node.transaction_count = 1;
        tx_node.first_time = Some(transaction.block_time);
        tx_node.last_time = Some(transaction.block_time);
        tx_node.note = ledger_note(&ledger_by_txid, &transaction.txid);
        ensure_node(&mut nodes, tx_node);

        let withdrawal_endpoint = withdrawal_endpoint_by_txid.get(&transaction.txid);
        let mut input_total = 0_i64;
        for input in &transaction.inputs {
            let source_id = if input.is_coinbase {
                ensure_coinbase_node(&mut nodes);
                "src:coinbase".to_string()
            } else if let Some(target) = output_targets
                .outpoint_nodes
                .get(&(input.prev_txid.clone(), input.prev_vout))
            {
                if let Some(meta) = output_targets.nodes_by_id.get(target) {
                    ensure_output_node(&mut nodes, meta);
                }
                target.clone()
            } else if let Some(endpoint) = withdrawal_endpoint {
                ensure_exchange_endpoint_node(&mut nodes, endpoint);
                endpoint.id.clone()
            } else if let Some(address) = input.address.as_deref() {
                if let Some(owner) = dataset.owners.get(address) {
                    let id = format!("src:addr:{address}");
                    ensure_address_source_node(&mut nodes, id.clone(), owner);
                    id
                } else if source_keep.contains(address) {
                    let id = format!("src:{address}");
                    ensure_external_node(
                        &mut nodes,
                        id.clone(),
                        FlowNodeKind::ExternalSource,
                        address,
                        None,
                    );
                    id
                } else {
                    "src:other".to_string()
                }
            } else {
                "src:unknown".to_string()
            };

            add_link(
                &mut links,
                source_id,
                tx_node_id.clone(),
                input.value_sats,
                FlowLinkKind::OnChain,
                Some(transaction.block_time),
                FlowEvidenceRef {
                    txid: Some(transaction.txid.clone()),
                    ledger_id: None,
                    timestamp: Some(transaction.block_time),
                    value_sats: input.value_sats,
                    description: format!("Input of {}", short_text(&transaction.txid)),
                },
            );
            input_total += input.value_sats.max(0);
        }

        // Group outputs of one transaction by address. This keeps a transaction
        // paying the same address twice from creating duplicate output nodes.
        let mut grouped_outputs: BTreeMap<String, (i64, BTreeSet<i64>, bool)> = BTreeMap::new();
        for output in transaction
            .outputs
            .iter()
            .filter(|output| output.is_spendable())
        {
            let Some(address) = output.address.as_deref() else {
                continue;
            };
            let entry = grouped_outputs
                .entry(address.to_string())
                .or_insert_with(|| (0, BTreeSet::new(), false));
            entry.0 += output.value_sats;
            entry.1.insert(output.output_index);
            if spent_outpoints.contains(&(transaction.txid.clone(), output.output_index)) {
                entry.2 = true;
            }
        }

        for (address, (value_sats, _indices, _spent)) in grouped_outputs {
            let target_id = if let Some(target) = output_targets
                .group_nodes
                .get(&(transaction.txid.clone(), address.clone()))
            {
                if let Some(meta) = output_targets.nodes_by_id.get(target) {
                    ensure_output_node(&mut nodes, meta);
                }
                target.clone()
            } else {
                "dst:other".to_string()
            };
            add_link(
                &mut links,
                tx_node_id.clone(),
                target_id,
                value_sats,
                FlowLinkKind::OnChain,
                Some(transaction.block_time),
                FlowEvidenceRef {
                    txid: Some(transaction.txid.clone()),
                    ledger_id: None,
                    timestamp: Some(transaction.block_time),
                    value_sats,
                    description: format!("Output of {}", short_text(&transaction.txid)),
                },
            );
        }

        let output_total: i64 = transaction
            .outputs
            .iter()
            .filter(|output| output.is_spendable())
            .map(|output| output.value_sats)
            .sum();
        if input_total == 0 && output_total > 0 {
            ensure_coinbase_node(&mut nodes);
            add_link(
                &mut links,
                "src:coinbase".to_string(),
                tx_node_id.clone(),
                output_total,
                FlowLinkKind::OnChain,
                Some(transaction.block_time),
                FlowEvidenceRef {
                    txid: Some(transaction.txid.clone()),
                    ledger_id: None,
                    timestamp: Some(transaction.block_time),
                    value_sats: output_total,
                    description: format!("Coinbase {}", short_text(&transaction.txid)),
                },
            );
        }
    }

    add_unmatched_ledger_links_transaction_view(&mut nodes, &mut links, &dataset.ledger);

    finalize_graph(
        dataset,
        nodes,
        links,
        FlowView::Transactions,
        dataset.transactions.len(),
        dataset.output_count(),
    )
}

/// Build a compact address summary from the same filtered dataset.
pub fn build_address_summary(dataset: &FlowDataset) -> FlowGraph {
    let mut transactions: Vec<&FlowTransaction> = dataset.transactions.iter().collect();
    transactions.sort_by(|left, right| {
        left.block_time
            .cmp(&right.block_time)
            .then_with(|| left.txid.cmp(&right.txid))
    });

    let mut external_totals: BTreeMap<String, i64> = BTreeMap::new();
    for transaction in &transactions {
        for input in &transaction.inputs {
            if input.is_coinbase {
                continue;
            }
            let Some(address) = input.address.as_deref() else {
                continue;
            };
            if !dataset.owners.contains_key(address) {
                *external_totals.entry(address.to_string()).or_default() += input.value_sats.max(0);
            }
        }
        for output in transaction
            .outputs
            .iter()
            .filter(|output| output.is_spendable())
        {
            let Some(address) = output.address.as_deref() else {
                continue;
            };
            if !dataset.owners.contains_key(address) {
                *external_totals.entry(address.to_string()).or_default() += output.value_sats;
            }
        }
    }
    let external_keep = top_keys(&external_totals, TOP_EXTERNAL_NODES);

    let mut nodes: BTreeMap<String, FlowNode> = BTreeMap::new();
    let mut links: BTreeMap<(String, String), LinkAccumulator> = BTreeMap::new();
    let mut internal_sats = 0_i64;

    for transaction in &transactions {
        let mut sources: BTreeMap<String, (i64, Option<FlowOwnerRef>, String)> = BTreeMap::new();
        let mut targets: BTreeMap<String, (i64, Option<FlowOwnerRef>, String)> = BTreeMap::new();

        for input in &transaction.inputs {
            if input.is_coinbase {
                continue;
            }
            let Some(address) = input.address.as_deref() else {
                continue;
            };
            let (key, owner) =
                summary_address_endpoint(address, dataset.owners.get(address), &external_keep);
            let label = summary_label(address, dataset.owners.get(address), &key);
            let entry = sources
                .entry(key)
                .or_insert_with(|| (0, owner.clone(), label));
            entry.0 += input.value_sats.max(0);
        }

        for output in transaction
            .outputs
            .iter()
            .filter(|output| output.is_spendable())
        {
            let Some(address) = output.address.as_deref() else {
                continue;
            };
            let (key, owner) =
                summary_address_endpoint(address, dataset.owners.get(address), &external_keep);
            let label = summary_label(address, dataset.owners.get(address), &key);
            let entry = targets
                .entry(key)
                .or_insert_with(|| (0, owner.clone(), label));
            entry.0 += output.value_sats;
        }

        let output_total: i64 = targets.values().map(|(value, _, _)| *value).sum();
        let input_total: i64 = sources.values().map(|(value, _, _)| *value).sum();
        if sources.is_empty() && output_total > 0 {
            sources.insert(
                "src:coinbase".to_string(),
                (output_total, None, "Coinbase / mining".to_string()),
            );
        }
        let effective_input_total = input_total.max(output_total);
        if effective_input_total == 0 {
            continue;
        }

        let mut tx_pair_values: BTreeMap<(String, String), i64> = BTreeMap::new();
        for (target_id, (target_value, target_owner, target_label)) in &targets {
            let mut remaining = *target_value;
            let source_count = sources.len();
            for (index, (source_id, (source_value, source_owner, source_label))) in
                sources.iter().enumerate()
            {
                let allocated = if index + 1 == source_count {
                    remaining
                } else {
                    ((*target_value as i128 * *source_value as i128)
                        / effective_input_total as i128)
                        .clamp(0, i64::MAX as i128) as i64
                };
                remaining -= allocated;
                if allocated <= 0 {
                    continue;
                }
                if source_id == target_id {
                    internal_sats += allocated;
                    continue;
                }
                ensure_summary_node(
                    &mut nodes,
                    source_id,
                    source_label,
                    source_owner.as_ref(),
                    address_from_key(source_id),
                );
                ensure_summary_node(
                    &mut nodes,
                    target_id,
                    target_label,
                    target_owner.as_ref(),
                    address_from_key(target_id),
                );
                *tx_pair_values
                    .entry((source_id.clone(), target_id.clone()))
                    .or_default() += allocated;
            }
        }

        for ((source_id, target_id), value_sats) in tx_pair_values {
            add_link(
                &mut links,
                source_id,
                target_id,
                value_sats,
                FlowLinkKind::Aggregated,
                Some(transaction.block_time),
                FlowEvidenceRef {
                    txid: Some(transaction.txid.clone()),
                    ledger_id: None,
                    timestamp: Some(transaction.block_time),
                    value_sats,
                    description: format!(
                        "Transaction {} · {}",
                        short_text(&transaction.txid),
                        format_sats_simple(value_sats)
                    ),
                },
            );
        }
    }

    add_unmatched_ledger_links_summary(&mut nodes, &mut links, &dataset.ledger);

    let (nodes, links, excluded_sats) = break_summary_cycles(nodes, links);
    let mut graph = finalize_graph(
        dataset,
        nodes,
        links,
        FlowView::Addresses,
        dataset.transactions.len(),
        dataset.output_count(),
    );
    graph.kpis.internal_sats = internal_sats;
    if excluded_sats > 0 {
        graph.warnings.push(FlowWarning {
            kind: FlowWarningKind::ExcludedCircular,
            message: format!(
                "{} of circular value was removed from this summary so ECharts can draw an acyclic Sankey. Switch to the transaction view for the exact flow.",
                format_sats_simple(excluded_sats)
            ),
            count: 0,
            value_sats: excluded_sats,
        });
    }
    graph
}

fn summary_address_endpoint(
    address: &str,
    owner: Option<&FlowAddressOwner>,
    external_keep: &HashSet<String>,
) -> (String, Option<FlowOwnerRef>) {
    if let Some(owner) = owner {
        return (format!("addr:{address}"), owner.owner_ref());
    }

    if external_keep.contains(address) {
        (format!("ext:{address}"), None)
    } else {
        ("ext:other".to_string(), None)
    }
}

fn summary_label(address: &str, owner: Option<&FlowAddressOwner>, key: &str) -> String {
    if key == "ext:other" {
        return "Other external addresses".to_string();
    }
    if let Some(owner) = owner {
        return owner.display_label();
    }
    short_text(address)
}

fn summary_node_kind(owner: Option<&FlowOwnerRef>, key: &str) -> FlowNodeKind {
    if key.starts_with("ext:") {
        return FlowNodeKind::External;
    }
    if key.starts_with("unlinked") {
        return FlowNodeKind::Unlinked;
    }
    if key == "src:coinbase" {
        return FlowNodeKind::Coinbase;
    }
    if key.starts_with("addr:") {
        return FlowNodeKind::Address;
    }
    match owner.map(|owner| owner.kind) {
        Some(FlowOwnerKind::Exchange) => FlowNodeKind::Exchange,
        Some(FlowOwnerKind::Wallet) => FlowNodeKind::Wallet,
        None => FlowNodeKind::Address,
    }
}

fn ensure_summary_node(
    nodes: &mut BTreeMap<String, FlowNode>,
    id: &str,
    label: &str,
    owner: Option<&FlowOwnerRef>,
    address: Option<String>,
) {
    if nodes.contains_key(id) {
        return;
    }
    let mut node = FlowNode::new(
        id.to_string(),
        label.to_string(),
        summary_node_kind(owner, id),
    )
    .with_owner(owner.cloned())
    .with_address(address);
    if id.starts_with("ext:") {
        node.address_count = if id.ends_with(":other") { 0 } else { 1 };
    }
    if id == "src:coinbase" {
        node.label = "Coinbase / mining".to_string();
        node.kind = FlowNodeKind::Coinbase;
    }
    nodes.insert(id.to_string(), node);
}

struct OutputTargets {
    outpoint_nodes: HashMap<(String, i64), String>,
    group_nodes: HashMap<(String, String), String>,
    nodes_by_id: BTreeMap<String, FlowNode>,
}

fn transaction_output_targets(
    transactions: &[&FlowTransaction],
    owners: &BTreeMap<String, FlowAddressOwner>,
    spent_outpoints: &HashSet<(String, i64)>,
    top_external: usize,
) -> OutputTargets {
    let mut external_terminal_totals: BTreeMap<String, i64> = BTreeMap::new();
    for transaction in transactions {
        let mut groups: BTreeMap<String, (i64, bool)> = BTreeMap::new();
        for output in transaction
            .outputs
            .iter()
            .filter(|output| output.is_spendable())
        {
            let Some(address) = output.address.as_deref() else {
                continue;
            };
            let entry = groups.entry(address.to_string()).or_insert((0, false));
            entry.0 += output.value_sats;
            if spent_outpoints.contains(&(transaction.txid.clone(), output.output_index)) {
                entry.1 = true;
            }
        }
        for (address, (value, spent)) in groups {
            if !spent && !owners.contains_key(&address) {
                *external_terminal_totals.entry(address).or_default() += value;
            }
        }
    }
    let terminal_keep = top_keys(&external_terminal_totals, top_external);

    let mut outpoint_nodes = HashMap::new();
    let mut group_nodes = HashMap::new();
    let mut nodes_by_id: BTreeMap<String, FlowNode> = BTreeMap::new();

    for transaction in transactions {
        let mut groups: BTreeMap<String, (i64, bool)> = BTreeMap::new();
        for output in transaction
            .outputs
            .iter()
            .filter(|output| output.is_spendable())
        {
            let Some(address) = output.address.as_deref() else {
                continue;
            };
            let entry = groups.entry(address.to_string()).or_insert((0, false));
            entry.0 += output.value_sats;
            if spent_outpoints.contains(&(transaction.txid.clone(), output.output_index)) {
                entry.1 = true;
            }
        }
        for (address, (_value, spent)) in groups {
            let stored = owners.get(&address);
            let target_id = if stored.is_some() || spent {
                let id = format!("out:{}:{address}", transaction.txid);
                let (kind, owner) = if stored.is_some() {
                    address_node_meta(stored)
                } else {
                    (FlowNodeKind::ExternalDestination, None)
                };
                let mut node =
                    FlowNode::new(id.clone(), address_display_label(stored, &address), kind)
                        .with_owner(owner)
                        .with_address(Some(address.clone()));
                node.transaction_count = 1;
                node.first_time = Some(transaction.block_time);
                node.last_time = Some(transaction.block_time);
                nodes_by_id.insert(id.clone(), node);
                group_nodes.insert((transaction.txid.clone(), address.clone()), id.clone());
                id
            } else if terminal_keep.contains(&address) {
                let id = format!("dst:{address}");
                let mut node = FlowNode::new(
                    id.clone(),
                    short_text(&address),
                    FlowNodeKind::ExternalDestination,
                )
                .with_address(Some(address.clone()));
                node.first_time = Some(transaction.block_time);
                node.last_time = Some(transaction.block_time);
                nodes_by_id.insert(id.clone(), node);
                group_nodes.insert((transaction.txid.clone(), address.clone()), id.clone());
                id
            } else {
                let id = "dst:other".to_string();
                nodes_by_id.entry(id.clone()).or_insert_with(|| {
                    let mut node = FlowNode::new(
                        id.clone(),
                        "Other external destinations".to_string(),
                        FlowNodeKind::ExternalDestination,
                    );
                    node.in_sats = 0;
                    node
                });
                group_nodes.insert((transaction.txid.clone(), address.clone()), id.clone());
                id
            };
            for output in transaction
                .outputs
                .iter()
                .filter(|output| output.address.as_deref() == Some(address.as_str()))
            {
                outpoint_nodes.insert(
                    (transaction.txid.clone(), output.output_index),
                    target_id.clone(),
                );
            }
        }
    }

    OutputTargets {
        outpoint_nodes,
        group_nodes,
        nodes_by_id,
    }
}

fn address_node_meta(owner: Option<&FlowAddressOwner>) -> (FlowNodeKind, Option<FlowOwnerRef>) {
    match owner.and_then(FlowAddressOwner::owner_ref) {
        Some(owner_ref) if owner_ref.kind == FlowOwnerKind::Exchange => {
            (FlowNodeKind::Exchange, Some(owner_ref))
        }
        Some(owner_ref) if owner_ref.kind == FlowOwnerKind::Wallet => {
            (FlowNodeKind::Wallet, Some(owner_ref))
        }
        _ => (FlowNodeKind::Address, None),
    }
}

fn address_display_label(owner: Option<&FlowAddressOwner>, address: &str) -> String {
    owner
        .map(FlowAddressOwner::display_label)
        .unwrap_or_else(|| short_text(address))
}

fn ensure_node(nodes: &mut BTreeMap<String, FlowNode>, node: FlowNode) {
    nodes.entry(node.id.clone()).or_insert(node);
}

fn ensure_output_node(nodes: &mut BTreeMap<String, FlowNode>, meta: &FlowNode) {
    nodes.entry(meta.id.clone()).or_insert_with(|| meta.clone());
}

fn ensure_address_source_node(
    nodes: &mut BTreeMap<String, FlowNode>,
    id: String,
    owner: &FlowAddressOwner,
) {
    let (kind, owner_ref) = address_node_meta(Some(owner));
    nodes.entry(id.clone()).or_insert_with(|| {
        FlowNode::new(id, owner.display_label(), kind)
            .with_owner(owner_ref)
            .with_address(Some(owner.address.clone()))
    });
}

fn ensure_external_node(
    nodes: &mut BTreeMap<String, FlowNode>,
    id: String,
    kind: FlowNodeKind,
    address: &str,
    label: Option<String>,
) {
    nodes.entry(id.clone()).or_insert_with(|| {
        FlowNode::new(id, label.unwrap_or_else(|| short_text(address)), kind)
            .with_address(Some(address.to_string()))
    });
}

fn ensure_coinbase_node(nodes: &mut BTreeMap<String, FlowNode>) {
    nodes.entry("src:coinbase".to_string()).or_insert_with(|| {
        FlowNode::new(
            "src:coinbase".to_string(),
            "Coinbase / mining".to_string(),
            FlowNodeKind::Coinbase,
        )
    });
}

fn ensure_exchange_endpoint_node(
    nodes: &mut BTreeMap<String, FlowNode>,
    endpoint: &ExchangeEndpoint,
) {
    nodes.entry(endpoint.id.clone()).or_insert_with(|| {
        FlowNode::new(
            endpoint.id.clone(),
            endpoint.label.clone(),
            FlowNodeKind::Exchange,
        )
        .with_owner(Some(endpoint.owner.clone()))
    });
}

#[derive(Debug, Clone)]
struct ExchangeEndpoint {
    id: String,
    label: String,
    owner: FlowOwnerRef,
}

fn withdrawal_endpoint_by_txid(ledger: &[FlowLedgerRow]) -> HashMap<String, ExchangeEndpoint> {
    let mut values = HashMap::new();
    for row in ledger {
        if row.tx_type != ExchangeTxType::Withdrawal
            || row.is_unmatched()
            || row.on_chain_txid.is_none()
        {
            continue;
        }
        let Some(txid) = row.on_chain_txid.clone() else {
            continue;
        };
        values.entry(txid).or_insert_with(|| ExchangeEndpoint {
            id: row.exchange_endpoint_id(),
            label: row.exchange_endpoint_label(),
            owner: row.exchange_endpoint_owner(),
        });
    }
    values
}

fn ledger_by_txid(ledger: &[FlowLedgerRow]) -> HashMap<String, Vec<&FlowLedgerRow>> {
    let mut values: HashMap<String, Vec<&FlowLedgerRow>> = HashMap::new();
    for row in ledger {
        if row.is_unmatched() || !row.is_deposit_or_withdrawal() {
            continue;
        }
        if let Some(txid) = row.on_chain_txid.as_deref() {
            values.entry(txid.to_string()).or_default().push(row);
        }
    }
    values
}

fn ledger_note(ledger: &HashMap<String, Vec<&FlowLedgerRow>>, txid: &str) -> Option<String> {
    let rows = ledger.get(txid)?;
    let notes = rows
        .iter()
        .map(|row| {
            let kind = match row.tx_type {
                ExchangeTxType::Withdrawal => "Withdrawal",
                ExchangeTxType::Deposit => "Deposit",
                _ => "Ledger",
            };
            format!("{kind} · {}", row.exchange_endpoint_label())
        })
        .collect::<Vec<_>>();
    Some(notes.join(" · "))
}

fn add_unmatched_ledger_links_transaction_view(
    nodes: &mut BTreeMap<String, FlowNode>,
    links: &mut BTreeMap<(String, String), LinkAccumulator>,
    ledger: &[FlowLedgerRow],
) {
    for row in ledger.iter().filter(|row| row.is_deposit_or_withdrawal()) {
        if !row.is_unmatched() {
            continue;
        }
        let value_sats = row.amount_sats.abs();
        if value_sats == 0 {
            continue;
        }
        let endpoint_id = row.exchange_endpoint_id();
        ensure_exchange_endpoint_node(
            nodes,
            &ExchangeEndpoint {
                id: endpoint_id.clone(),
                label: row.exchange_endpoint_label(),
                owner: row.exchange_endpoint_owner(),
            },
        );
        let unlinked_id = match row.tx_type {
            ExchangeTxType::Withdrawal => "unlinked:withdrawal",
            ExchangeTxType::Deposit => "unlinked:deposit",
            _ => continue,
        };
        nodes.entry(unlinked_id.to_string()).or_insert_with(|| {
            FlowNode::new(
                unlinked_id.to_string(),
                if row.tx_type == ExchangeTxType::Withdrawal {
                    "Unlinked withdrawals".to_string()
                } else {
                    "Unlinked deposits".to_string()
                },
                FlowNodeKind::Unlinked,
            )
        });
        let (source, target) = match row.tx_type {
            ExchangeTxType::Withdrawal => (endpoint_id, unlinked_id.to_string()),
            ExchangeTxType::Deposit => (unlinked_id.to_string(), endpoint_id),
            _ => continue,
        };
        add_link(
            links,
            source,
            target,
            value_sats,
            FlowLinkKind::LedgerUnlinked,
            Some(row.timestamp),
            FlowEvidenceRef {
                txid: None,
                ledger_id: Some(row.id),
                timestamp: Some(row.timestamp),
                value_sats,
                description: format!(
                    "{} · {}",
                    row.tx_type.as_str(),
                    row.exchange_endpoint_label()
                ),
            },
        );
    }
}

fn add_unmatched_ledger_links_summary(
    nodes: &mut BTreeMap<String, FlowNode>,
    links: &mut BTreeMap<(String, String), LinkAccumulator>,
    ledger: &[FlowLedgerRow],
) {
    for row in ledger.iter().filter(|row| row.is_deposit_or_withdrawal()) {
        if !row.is_unmatched() {
            continue;
        }
        let value_sats = row.amount_sats.abs();
        if value_sats == 0 {
            continue;
        }
        let endpoint_id = row.exchange_endpoint_id();
        let owner = row.exchange_endpoint_owner();
        ensure_summary_node(
            nodes,
            &endpoint_id,
            &row.exchange_endpoint_label(),
            Some(&owner),
            None,
        );
        let unlinked_id = match row.tx_type {
            ExchangeTxType::Withdrawal => "unlinked:withdrawal",
            ExchangeTxType::Deposit => "unlinked:deposit",
            _ => continue,
        };
        nodes.entry(unlinked_id.to_string()).or_insert_with(|| {
            FlowNode::new(
                unlinked_id.to_string(),
                if row.tx_type == ExchangeTxType::Withdrawal {
                    "Unlinked withdrawals".to_string()
                } else {
                    "Unlinked deposits".to_string()
                },
                FlowNodeKind::Unlinked,
            )
        });
        let (source, target) = match row.tx_type {
            ExchangeTxType::Withdrawal => (endpoint_id, unlinked_id.to_string()),
            ExchangeTxType::Deposit => (unlinked_id.to_string(), endpoint_id),
            _ => continue,
        };
        add_link(
            links,
            source,
            target,
            value_sats,
            FlowLinkKind::LedgerUnlinked,
            Some(row.timestamp),
            FlowEvidenceRef {
                txid: None,
                ledger_id: Some(row.id),
                timestamp: Some(row.timestamp),
                value_sats,
                description: format!(
                    "{} · {}",
                    row.tx_type.as_str(),
                    row.exchange_endpoint_label()
                ),
            },
        );
    }
}

fn add_link(
    links: &mut BTreeMap<(String, String), LinkAccumulator>,
    source: String,
    target: String,
    value_sats: i64,
    kind: FlowLinkKind,
    timestamp: Option<i64>,
    evidence: FlowEvidenceRef,
) {
    if value_sats <= 0 || source == target {
        return;
    }
    let entry = links
        .entry((source.clone(), target.clone()))
        .or_insert_with(|| LinkAccumulator::new(kind));
    entry.merge_kind(kind);
    entry.add(value_sats, timestamp, evidence);
}

type SummaryGraphMaps = (
    BTreeMap<String, FlowNode>,
    BTreeMap<(String, String), LinkAccumulator>,
    i64,
);

fn break_summary_cycles(
    nodes: BTreeMap<String, FlowNode>,
    links: BTreeMap<(String, String), LinkAccumulator>,
) -> SummaryGraphMaps {
    // Net opposing flows first. This is the common case and keeps a wallet's
    // deposits and withdrawals from drawing two opposing bands.
    let mut net: BTreeMap<(String, String), LinkAccumulator> = BTreeMap::new();
    let mut excluded = 0_i64;
    let mut grouped: BTreeMap<
        (String, String),
        (Option<LinkAccumulator>, Option<LinkAccumulator>),
    > = BTreeMap::new();
    for ((source, target), link) in links {
        let pair = if source <= target {
            (source.clone(), target.clone())
        } else {
            (target.clone(), source.clone())
        };
        let entry = grouped.entry(pair).or_default();
        if source <= target {
            entry.0 = Some(link);
        } else {
            entry.1 = Some(link);
        }
    }
    for ((left, right), (forward, reverse)) in grouped.into_iter() {
        match (forward, reverse) {
            (Some(forward), Some(reverse)) => {
                excluded += forward.value_sats.min(reverse.value_sats);
                if forward.value_sats >= reverse.value_sats {
                    let mut value = forward;
                    value.value_sats -= reverse.value_sats;
                    if value.value_sats > 0 {
                        net.insert((left, right), value);
                    }
                } else {
                    let mut value = reverse;
                    value.value_sats -= forward.value_sats;
                    if value.value_sats > 0 {
                        net.insert((right, left), value);
                    }
                }
            }
            (Some(forward), None) => {
                net.insert((left.clone(), right.clone()), forward);
            }
            (None, Some(reverse)) => {
                net.insert((right, left), reverse);
            }
            (None, None) => {}
        }
    }

    // Greedy maximum-value acyclic subset. The summary graph is small, so a
    // reachability check per edge is cheap and deterministic.
    let mut sorted: Vec<((String, String), LinkAccumulator)> = net.into_iter().collect();
    sorted.sort_by(
        |((source_a, target_a), link_a), ((source_b, target_b), link_b)| {
            link_b
                .value_sats
                .cmp(&link_a.value_sats)
                .then_with(|| source_a.cmp(source_b))
                .then_with(|| target_a.cmp(target_b))
        },
    );
    let mut kept: BTreeMap<(String, String), LinkAccumulator> = BTreeMap::new();
    let mut adjacency: HashMap<String, HashSet<String>> = HashMap::new();
    for ((source, target), link) in sorted {
        if reaches(&adjacency, &target, &source) {
            excluded += link.value_sats;
            continue;
        }
        adjacency
            .entry(source.clone())
            .or_default()
            .insert(target.clone());
        kept.insert((source, target), link);
    }

    // Drop nodes that lost all their links.
    let connected: HashSet<String> = kept
        .keys()
        .flat_map(|(source, target)| [source.clone(), target.clone()])
        .collect();
    let nodes = nodes
        .into_iter()
        .filter(|(id, _)| connected.contains(id))
        .collect();
    (nodes, kept, excluded)
}

fn reaches(adjacency: &HashMap<String, HashSet<String>>, from: &str, to: &str) -> bool {
    if from == to {
        return true;
    }
    let mut stack = vec![from.to_string()];
    let mut visited = HashSet::new();
    while let Some(node) = stack.pop() {
        if node == to {
            return true;
        }
        if !visited.insert(node.clone()) {
            continue;
        }
        if let Some(next) = adjacency.get(&node) {
            stack.extend(next.iter().cloned());
        }
    }
    false
}

fn finalize_graph(
    dataset: &FlowDataset,
    mut nodes: BTreeMap<String, FlowNode>,
    links: BTreeMap<(String, String), LinkAccumulator>,
    view: FlowView,
    transaction_count: usize,
    output_count: usize,
) -> FlowGraph {
    for (source, target) in links.keys() {
        nodes
            .entry(source.clone())
            .or_insert_with(|| default_node_for_id(source));
        nodes
            .entry(target.clone())
            .or_insert_with(|| default_node_for_id(target));
    }

    for ((source, target), link) in &links {
        if let Some(node) = nodes.get_mut(source) {
            node.out_sats += link.value_sats;
            if node.kind != FlowNodeKind::Transaction {
                node.transaction_count += link.event_count;
            }
            node.first_time = min_time(node.first_time, link.first_time);
            node.last_time = max_time(node.last_time, link.last_time);
        }
        if let Some(node) = nodes.get_mut(target) {
            node.in_sats += link.value_sats;
            if node.kind != FlowNodeKind::Transaction {
                node.transaction_count += link.event_count;
            }
            node.first_time = min_time(node.first_time, link.first_time);
            node.last_time = max_time(node.last_time, link.last_time);
        }
    }

    // Attach balances and address counts to summary address nodes.
    if view.is_summary() {
        let mut balances: BTreeMap<String, (i64, i64)> = BTreeMap::new();
        for address in &dataset.focus_addresses {
            let Some(owner) = dataset.owners.get(address) else {
                continue;
            };
            let key = format!("addr:{}", owner.address);
            let entry = balances.entry(key).or_default();
            entry.0 += owner.balance_sats;
            entry.1 += 1;
        }
        for (id, (balance, count)) in balances {
            if let Some(node) = nodes.get_mut(&id) {
                node.balance_sats = Some(balance);
                node.address_count = count;
            }
        }
    }

    let links = links
        .into_iter()
        .map(|((source, target), link)| {
            let evidence = dedupe_evidence(link.evidence);
            let event_count = evidence.len() as i64;
            FlowLink {
                source,
                target,
                value_sats: link.value_sats,
                event_count,
                first_time: link.first_time,
                last_time: link.last_time,
                kind: link.kind,
                evidence,
            }
        })
        .collect::<Vec<_>>();

    let origin_count = nodes
        .values()
        .filter(|node| node.in_sats == 0 && node.out_sats > 0)
        .count() as i64;
    let destination_count = nodes
        .values()
        .filter(|node| node.out_sats == 0 && node.in_sats > 0)
        .count() as i64;
    let exchange_activity = exchange_activity(&dataset.ledger);
    let mut warnings = Vec::new();
    let incomplete = dataset.incomplete_address_count();
    if incomplete > 0 {
        warnings.push(FlowWarning {
            kind: FlowWarningKind::IncompleteHistory,
            message: format!(
                "{incomplete} selected address{} have incomplete transaction history because a mempool rate limit interrupted indexing. Some flows may be missing.",
                if incomplete == 1 { "" } else { "es" }
            ),
            count: incomplete,
            value_sats: 0,
        });
    }
    if exchange_activity.unmatched_count > 0 {
        warnings.push(FlowWarning {
            kind: FlowWarningKind::UnmatchedLedger,
            message: format!(
                "{} ledger row{} ({}) are not linked to on-chain data and appear as unlinked flows.",
                exchange_activity.unmatched_count,
                if exchange_activity.unmatched_count == 1 { "" } else { "s" },
                format_sats_simple(exchange_activity.unmatched_sats)
            ),
            count: exchange_activity.unmatched_count,
            value_sats: exchange_activity.unmatched_sats,
        });
    }

    let kpis = FlowKpis {
        address_count: dataset.focus_addresses.len() as i64,
        transaction_count: transaction_count as i64,
        gross_output_sats: dataset.gross_output_sats(),
        fee_sats: dataset.fee_sats(),
        current_balance_sats: dataset.focus_balance_sats(),
        origin_count,
        destination_count,
        internal_sats: 0,
        unmatched_ledger_sats: exchange_activity.unmatched_sats,
        unmatched_ledger_count: exchange_activity.unmatched_count,
        incomplete_address_count: incomplete,
    };

    FlowGraph {
        stats: FlowGraphStats {
            node_count: nodes.len(),
            link_count: links.len(),
            transaction_count,
            output_count,
        },
        nodes: nodes.into_values().collect(),
        links,
        kpis,
        exchange_activity,
        warnings,
    }
}

fn default_node_for_id(id: &str) -> FlowNode {
    let (label, kind) = match id {
        "src:coinbase" => ("Coinbase / mining".to_string(), FlowNodeKind::Coinbase),
        "src:other" => (
            "Other external sources".to_string(),
            FlowNodeKind::ExternalSource,
        ),
        "src:unknown" => (
            "Unknown external source".to_string(),
            FlowNodeKind::ExternalSource,
        ),
        "dst:other" => (
            "Other external destinations".to_string(),
            FlowNodeKind::ExternalDestination,
        ),
        "ext:other" => (
            "Other external addresses".to_string(),
            FlowNodeKind::External,
        ),
        "unlinked:withdrawal" => ("Unlinked withdrawals".to_string(), FlowNodeKind::Unlinked),
        "unlinked:deposit" => ("Unlinked deposits".to_string(), FlowNodeKind::Unlinked),
        _ if id.starts_with("exchange:") || id.starts_with("exchange-account:") => {
            ("Exchange".to_string(), FlowNodeKind::Exchange)
        }
        _ if id.starts_with("wallet:") => ("Wallet".to_string(), FlowNodeKind::Wallet),
        _ if id.starts_with("addr:") => ("Address".to_string(), FlowNodeKind::Address),
        _ if id.starts_with("src:") => {
            ("External source".to_string(), FlowNodeKind::ExternalSource)
        }
        _ if id.starts_with("dst:") => (
            "External destination".to_string(),
            FlowNodeKind::ExternalDestination,
        ),
        _ if id.starts_with("ext:") => ("External address".to_string(), FlowNodeKind::External),
        _ => (id.to_string(), FlowNodeKind::Address),
    };
    FlowNode::new(id.to_string(), label, kind)
}

fn dedupe_evidence(evidence: Vec<FlowEvidenceRef>) -> Vec<FlowEvidenceRef> {
    let mut seen: HashSet<(Option<String>, Option<i64>)> = HashSet::new();
    let mut values = Vec::new();
    for item in evidence {
        let key = (item.txid.clone(), item.ledger_id);
        if seen.insert(key) {
            values.push(item);
        }
    }
    values
}

fn exchange_activity(ledger: &[FlowLedgerRow]) -> FlowExchangeActivity {
    let mut activity = FlowExchangeActivity::default();
    for row in ledger {
        let amount = row.amount_sats.abs();
        match row.tx_type {
            ExchangeTxType::Buy => activity.bought_sats += amount,
            ExchangeTxType::Sell => activity.sold_sats += amount,
            ExchangeTxType::Reward => activity.rewarded_sats += amount,
            ExchangeTxType::Withdrawal => activity.withdrawn_sats += amount,
            ExchangeTxType::Deposit => activity.deposited_sats += amount,
            ExchangeTxType::Unknown => {}
        }
        if row.is_deposit_or_withdrawal() && row.is_unmatched() {
            activity.unmatched_count += 1;
            activity.unmatched_sats += amount;
        }
    }
    activity.net_ledger_sats =
        activity.bought_sats + activity.rewarded_sats + activity.deposited_sats
            - activity.sold_sats
            - activity.withdrawn_sats;
    activity
}

fn top_keys(totals: &BTreeMap<String, i64>, limit: usize) -> HashSet<String> {
    let mut values: Vec<(&String, &i64)> = totals.iter().collect();
    values.sort_by(|(left_key, left_value), (right_key, right_value)| {
        right_value
            .cmp(left_value)
            .then_with(|| left_key.cmp(right_key))
    });
    values
        .into_iter()
        .take(limit)
        .map(|(key, _)| key.clone())
        .collect()
}

fn transaction_node_id(txid: &str) -> String {
    format!("tx:{txid}")
}

fn address_from_key(key: &str) -> Option<String> {
    key.strip_prefix("addr:")
        .filter(|value| *value != "unclassified")
        .or_else(|| {
            key.strip_prefix("out:")
                .and_then(|rest| rest.split_once(':').map(|(_, address)| address))
        })
        .or_else(|| key.strip_prefix("ext:").filter(|value| *value != "other"))
        .or_else(|| {
            key.strip_prefix("src:")
                .filter(|value| *value != "other" && *value != "unknown")
        })
        .or_else(|| key.strip_prefix("dst:").filter(|value| *value != "other"))
        .map(str::to_string)
}

fn min_time(current: Option<i64>, next: Option<i64>) -> Option<i64> {
    match (current, next) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn max_time(current: Option<i64>, next: Option<i64>) -> Option<i64> {
    match (current, next) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn short_text(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 14 {
        return value.to_string();
    }
    let head: String = chars.iter().take(7).collect();
    let tail: String = chars.iter().skip(chars.len().saturating_sub(5)).collect();
    format!("{head}...{tail}")
}

fn format_sats_simple(sats: i64) -> String {
    let sign = if sats < 0 { "-" } else { "" };
    let absolute = (sats as i128).abs();
    let whole = absolute / 100_000_000;
    let fraction = absolute % 100_000_000;
    if fraction == 0 {
        format!("{sign}{whole} BTC")
    } else {
        let digits = format!("{fraction:08}");
        let digits = digits.trim_end_matches('0');
        format!("{sign}{whole}.{digits} BTC")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(address: &str, exchange_id: Option<i64>, wallet_id: Option<i64>) -> FlowAddressOwner {
        FlowAddressOwner {
            address: address.to_string(),
            exchange_id,
            exchange_name: exchange_id.map(|_| "Exchange".to_string()),
            wallet_id,
            wallet_name: wallet_id.map(|_| "Wallet".to_string()),
            balance_sats: 0,
            ..FlowAddressOwner::default()
        }
    }

    fn tx(
        txid: &str,
        block_time: i64,
        inputs: Vec<(&str, i64, bool)>,
        outputs: Vec<(&str, i64)>,
    ) -> FlowTransaction {
        FlowTransaction {
            txid: txid.to_string(),
            block_time,
            fee_sats: 0,
            inputs: inputs
                .into_iter()
                .enumerate()
                .map(|(index, (address, value, coinbase))| FlowInput {
                    prev_txid: if coinbase {
                        String::new()
                    } else {
                        format!("source-{index}")
                    },
                    prev_vout: index as i64,
                    address: Some(address.to_string()),
                    value_sats: value,
                    is_coinbase: coinbase,
                })
                .collect(),
            outputs: outputs
                .into_iter()
                .enumerate()
                .map(|(index, (address, value))| FlowOutput {
                    output_index: index as i64,
                    address: Some(address.to_string()),
                    value_sats: value,
                    scriptpubkey_type: "v0_p2wpkh".to_string(),
                })
                .collect(),
        }
    }

    fn dataset(owners: Vec<FlowAddressOwner>, transactions: Vec<FlowTransaction>) -> FlowDataset {
        FlowDataset {
            focus_addresses: owners.iter().map(|owner| owner.address.clone()).collect(),
            owners: owners
                .into_iter()
                .map(|owner| (owner.address.clone(), owner))
                .collect(),
            transactions,
            ledger: Vec::new(),
        }
    }

    fn link_value(graph: &FlowGraph, source: &str, target: &str) -> i64 {
        graph
            .links
            .iter()
            .find(|link| link.source == source && link.target == target)
            .map(|link| link.value_sats)
            .unwrap_or(0)
    }

    #[test]
    fn transaction_graph_links_outputs_to_the_spending_transaction() {
        let data = dataset(
            vec![owner("a", None, Some(1))],
            vec![
                tx(
                    "one",
                    10,
                    vec![("a", 1000, false)],
                    vec![("a", 700), ("b", 200)],
                ),
                tx("two", 20, vec![("b", 200, false)], vec![("a", 150)]),
            ],
        );
        // The second transaction's prevout does not match a stored outpoint,
        // so the source is an external source; the first output still produces
        // an exact output node.
        let graph = build_transaction_graph(&data);
        assert!(graph.links.iter().any(|link| link.target == "tx:one"));
        assert!(graph.links.iter().any(|link| link.source == "tx:one"));
        assert!(graph.links.iter().any(|link| link.target == "tx:two"));
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.kind == FlowNodeKind::Transaction));
    }

    #[test]
    fn transaction_graph_is_acyclic_and_keeps_external_buckets() {
        let data = dataset(
            vec![owner("a", None, Some(1))],
            vec![tx(
                "one",
                10,
                vec![("external", 1000, false)],
                vec![("external-2", 500), ("a", 400)],
            )],
        );
        let graph = build_transaction_graph(&data);
        assert!(graph.links.iter().any(|link| link.target == "tx:one"));
        assert!(graph.nodes.iter().any(|node| node.id == "src:external"));
        assert!(graph.nodes.iter().any(|node| node.id == "dst:external-2"));
        assert!(graph_is_acyclic(&graph));
    }

    fn graph_is_acyclic(graph: &FlowGraph) -> bool {
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
        for link in &graph.links {
            adjacency
                .entry(link.source.as_str())
                .or_default()
                .push(link.target.as_str());
        }
        fn visit<'a>(
            node: &'a str,
            adjacency: &HashMap<&'a str, Vec<&'a str>>,
            visiting: &mut HashSet<&'a str>,
            done: &mut HashSet<&'a str>,
        ) -> bool {
            if done.contains(node) {
                return true;
            }
            if !visiting.insert(node) {
                return false;
            }
            if let Some(next) = adjacency.get(node) {
                for candidate in next {
                    if !visit(candidate, adjacency, visiting, done) {
                        return false;
                    }
                }
            }
            visiting.remove(node);
            done.insert(node);
            true
        }
        let mut visiting = HashSet::new();
        let mut done = HashSet::new();
        graph
            .nodes
            .iter()
            .all(|node| visit(&node.id, &adjacency, &mut visiting, &mut done))
    }

    #[test]
    fn summary_nets_opposing_flows_and_reports_the_excluded_value() {
        let data = dataset(
            vec![owner("a", None, Some(1)), owner("b", None, Some(2))],
            vec![
                tx("one", 10, vec![("a", 1000, false)], vec![("b", 1000)]),
                tx("two", 20, vec![("b", 600, false)], vec![("a", 600)]),
            ],
        );
        let graph = build_address_summary(&data);
        assert_eq!(link_value(&graph, "addr:a", "addr:b"), 400);
        assert_eq!(link_value(&graph, "addr:b", "addr:a"), 0);
    }

    #[test]
    fn address_summary_keeps_stored_addresses_separate() {
        let data = dataset(
            vec![
                owner("a", None, Some(1)),
                owner("b", None, Some(1)),
                owner("c", Some(9), None),
            ],
            vec![tx(
                "one",
                10,
                vec![("c", 1000, false)],
                vec![("a", 700), ("b", 300)],
            )],
        );
        let graph = build_address_summary(&data);
        assert_eq!(link_value(&graph, "addr:c", "addr:a"), 700);
        assert_eq!(link_value(&graph, "addr:c", "addr:b"), 300);
        assert!(graph.nodes.iter().any(|node| node.id == "addr:a"));
        assert!(graph.nodes.iter().any(|node| node.id == "addr:b"));
    }

    #[test]
    fn matching_a_withdrawal_labels_external_inputs_as_the_exchange_origin() {
        let mut data = dataset(
            vec![owner("a", None, Some(1))],
            vec![tx(
                "one",
                10,
                vec![("unknown", 1000, false)],
                vec![("a", 900)],
            )],
        );
        data.ledger.push(FlowLedgerRow {
            id: 7,
            account_id: 1,
            account_name: "Main".to_string(),
            provider_id: Some(2),
            provider_name: Some("Binance".to_string()),
            provider_color: None,
            timestamp: 10,
            tx_type: ExchangeTxType::Withdrawal,
            amount_sats: 900,
            match_state: MatchState::HashMatch,
            on_chain_txid: Some("one".to_string()),
            local_addresses: vec!["a".to_string()],
        });
        let graph = build_transaction_graph(&data);
        assert!(graph.links.iter().any(|link| link.source == "exchange:2"));
        assert!(graph.nodes.iter().any(|node| node.id == "exchange:2"));
        assert!(graph
            .nodes
            .iter()
            .find(|node| node.id == "tx:one")
            .and_then(|node| node.note.clone())
            .is_some());
    }

    #[test]
    fn unmatched_ledger_rows_create_directional_unlinked_flows() {
        let mut data = dataset(vec![owner("a", None, Some(1))], Vec::new());
        data.ledger.push(FlowLedgerRow {
            id: 1,
            account_id: 1,
            account_name: "Main".to_string(),
            provider_id: Some(2),
            provider_name: Some("Binance".to_string()),
            provider_color: None,
            timestamp: 10,
            tx_type: ExchangeTxType::Withdrawal,
            amount_sats: 500,
            match_state: MatchState::Unmatched,
            on_chain_txid: None,
            local_addresses: Vec::new(),
        });
        let graph = build_transaction_graph(&data);
        assert_eq!(link_value(&graph, "exchange:2", "unlinked:withdrawal"), 500);
        assert_eq!(
            graph
                .warnings
                .iter()
                .filter(|warning| warning.kind == FlowWarningKind::UnmatchedLedger)
                .count(),
            1
        );
    }

    #[test]
    fn spending_transaction_links_to_the_exact_produced_output_node() {
        let data = dataset(
            vec![owner("a", None, Some(1)), owner("b", None, Some(2))],
            vec![
                FlowTransaction {
                    txid: "one".to_string(),
                    block_time: 10,
                    fee_sats: 100,
                    inputs: vec![FlowInput {
                        prev_txid: "before".to_string(),
                        prev_vout: 0,
                        address: Some("a".to_string()),
                        value_sats: 1_000,
                        is_coinbase: false,
                    }],
                    outputs: vec![FlowOutput {
                        output_index: 0,
                        address: Some("b".to_string()),
                        value_sats: 900,
                        scriptpubkey_type: "v0_p2wpkh".to_string(),
                    }],
                },
                FlowTransaction {
                    txid: "two".to_string(),
                    block_time: 20,
                    fee_sats: 0,
                    inputs: vec![FlowInput {
                        prev_txid: "one".to_string(),
                        prev_vout: 0,
                        address: Some("b".to_string()),
                        value_sats: 900,
                        is_coinbase: false,
                    }],
                    outputs: vec![FlowOutput {
                        output_index: 0,
                        address: Some("a".to_string()),
                        value_sats: 800,
                        scriptpubkey_type: "v0_p2wpkh".to_string(),
                    }],
                },
            ],
        );
        let graph = build_transaction_graph(&data);
        assert_eq!(link_value(&graph, "out:one:b", "tx:two"), 900);
        assert_eq!(link_value(&graph, "tx:one", "out:one:b"), 900);
        assert!(!graph
            .links
            .iter()
            .any(|link| link.target == "tx:two" && link.source == "src:other"));
        assert!(graph_is_acyclic(&graph));
    }

    #[test]
    fn summary_external_self_flow_is_kept_internal_instead_of_forming_a_self_edge() {
        let data = dataset(
            vec![owner("a", None, Some(1))],
            vec![tx(
                "one",
                10,
                vec![("external-x", 1_000, false)],
                vec![("external-x", 900), ("a", 100)],
            )],
        );
        let graph = build_address_summary(&data);
        assert_eq!(link_value(&graph, "ext:external-x", "addr:a"), 100);
        assert!(!graph.links.iter().any(|link| link.source == link.target));
        assert!(graph.kpis.internal_sats >= 900);
    }

    #[test]
    fn graph_ids_are_unique_and_links_reference_existing_nodes() {
        let data = dataset(
            vec![owner("a", None, Some(1)), owner("b", Some(2), None)],
            vec![
                tx(
                    "one",
                    10,
                    vec![("a", 1000, false)],
                    vec![("b", 900), ("external", 100)],
                ),
                tx("two", 20, vec![("b", 900, false)], vec![("a", 800)]),
            ],
        );
        let graph = build_transaction_graph(&data);
        let ids: HashSet<&str> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
        assert_eq!(ids.len(), graph.nodes.len());
        for link in &graph.links {
            assert!(ids.contains(link.source.as_str()));
            assert!(ids.contains(link.target.as_str()));
        }
        assert!(graph.kpis.transaction_count > 0);
    }
}
