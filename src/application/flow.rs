//! Flow service: validates filter input, loads the filtered dataset, chooses
//! the exact transaction graph or a bounded summary, and paginates the
//! evidence tables.

use std::cmp::Ordering;

use crate::data::repositories::flow as flow_repository;
use crate::data::repositories::flow::{FlowEvidenceSortColumn, FlowLinkSortColumn};
use crate::data::{Database, DatabaseError};
use crate::domain::flow::{
    build_address_summary, build_transaction_graph, FlowEvidenceQuery, FlowEvidenceRow, FlowGraph,
    FlowLinkRow, FlowLinkTableQuery, FlowOverview, FlowQuery, FlowRenderBudget, FlowView,
    FlowWarning, FlowWarningKind,
};
use crate::domain::table::{SortDirection, TablePage};

const MAX_PAGE_SIZE: u32 = 100;

#[derive(Debug, thiserror::Error)]
pub enum FlowServiceError {
    #[error("{0}")]
    Validation(String),
    #[error("flow link not found")]
    NotFound,
    #[error("database access failed: {0}")]
    Database(#[from] DatabaseError),
}

pub struct FlowService {
    database: Database,
}

impl FlowService {
    pub fn connect(database: Database) -> Self {
        Self { database }
    }

    pub fn filter_options(
        &self,
    ) -> Result<crate::domain::flow::FlowFilterOptions, FlowServiceError> {
        Ok(flow_repository::filter_options(&self.database)?)
    }

    pub fn overview(&self, query: &FlowQuery) -> Result<FlowOverview, FlowServiceError> {
        flow_repository::validate_query(query).map_err(FlowServiceError::Validation)?;
        let dataset = flow_repository::load_dataset(&self.database, query)?;
        let budget = FlowRenderBudget::default();
        let dataset_exceeded = dataset.transactions.len() > budget.max_transactions
            || dataset.output_count() > budget.max_outputs;

        let mut rendered_view = query.view;
        let mut auto_fallback = false;
        let mut graph = if query.view == FlowView::Transactions {
            if dataset_exceeded && !query.force_transactions {
                rendered_view = FlowView::Addresses;
                auto_fallback = true;
                build_address_summary(&dataset)
            } else {
                build_transaction_graph(&dataset)
            }
        } else {
            build_address_summary(&dataset)
        };

        let graph_exceeded =
            graph.stats.node_count > budget.max_nodes || graph.stats.link_count > budget.max_links;
        if query.view == FlowView::Transactions
            && !query.force_transactions
            && rendered_view == FlowView::Transactions
            && graph_exceeded
        {
            rendered_view = FlowView::Addresses;
            auto_fallback = true;
            graph = build_address_summary(&dataset);
        }

        let budget = FlowRenderBudget {
            exceeded: dataset_exceeded || graph_exceeded,
            ..budget
        };
        if auto_fallback {
            graph.warnings.push(FlowWarning {
                kind: FlowWarningKind::AutoFallback,
                message: format!(
                    "This selection contains {} transactions. The address summary is shown to keep the chart responsive; the transaction detail is still available.",
                    dataset.transactions.len()
                ),
                count: dataset.transactions.len() as i64,
                value_sats: 0,
            });
        } else if graph_exceeded || (query.force_transactions && dataset_exceeded) {
            graph.warnings.push(FlowWarning {
                kind: FlowWarningKind::LargeGraph,
                message: format!(
                    "This transaction graph has {} nodes, {} links and {} transactions. Rendering may be slow; narrow the date range or filters for a lighter view.",
                    graph.stats.node_count,
                    graph.stats.link_count,
                    graph.stats.transaction_count
                ),
                count: graph.stats.node_count as i64,
                value_sats: 0,
            });
        }

        Ok(FlowOverview {
            requested_view: query.view,
            rendered_view,
            graph,
            budget,
            auto_fallback,
        })
    }

    pub fn link_table(
        &self,
        query: &FlowLinkTableQuery,
    ) -> Result<TablePage<FlowLinkRow>, FlowServiceError> {
        let overview = self.overview(&query.flow)?;
        let link_sort = flow_repository::parse_link_sort(query.table.sort_by.as_deref())
            .map_err(FlowServiceError::Validation)?;
        let page_size = query.table.page_size.clamp(1, MAX_PAGE_SIZE);
        let page = query.table.page;
        let mut rows = link_rows(&overview.graph);
        sort_link_rows(&mut rows, link_sort, query.table.sort_dir);
        let total = rows.len() as u64;
        let start = (page as usize).saturating_mul(page_size as usize);
        let items = rows
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();
        Ok(TablePage { items, total })
    }

    pub fn evidence(
        &self,
        query: &FlowEvidenceQuery,
    ) -> Result<TablePage<FlowEvidenceRow>, FlowServiceError> {
        let overview = self.overview(&query.flow)?;
        let link = overview
            .graph
            .links
            .iter()
            .find(|link| link.source == query.source_id && link.target == query.target_id)
            .ok_or(FlowServiceError::NotFound)?;
        let sort = flow_repository::parse_evidence_sort(query.table.sort_by.as_deref())
            .map_err(FlowServiceError::Validation)?;
        let page_size = query.table.page_size.clamp(1, MAX_PAGE_SIZE);
        let mut rows: Vec<FlowEvidenceRow> = link
            .evidence
            .iter()
            .map(|evidence| FlowEvidenceRow {
                txid: evidence.txid.clone(),
                ledger_id: evidence.ledger_id,
                timestamp: evidence.timestamp,
                value_sats: evidence.value_sats,
                description: evidence.description.clone(),
            })
            .collect();
        sort_evidence_rows(&mut rows, sort, query.table.sort_dir);
        let total = rows.len() as u64;
        let start = (query.table.page as usize).saturating_mul(page_size as usize);
        let items = rows
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();
        Ok(TablePage { items, total })
    }
}

fn link_rows(graph: &FlowGraph) -> Vec<FlowLinkRow> {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<std::collections::HashMap<_, _>>();
    let total = graph.total_link_sats();
    graph
        .links
        .iter()
        .filter_map(|link| {
            let source = nodes.get(link.source.as_str())?;
            let target = nodes.get(link.target.as_str())?;
            Some(FlowLinkRow {
                source_id: link.source.clone(),
                target_id: link.target.clone(),
                source_label: source.label.clone(),
                target_label: target.label.clone(),
                source_kind: source.kind,
                target_kind: target.kind,
                source_color: source
                    .owner
                    .as_ref()
                    .and_then(|owner| owner.brand_color.clone()),
                target_color: target
                    .owner
                    .as_ref()
                    .and_then(|owner| owner.brand_color.clone()),
                value_sats: link.value_sats,
                event_count: link.event_count,
                first_time: link.first_time,
                last_time: link.last_time,
                kind: link.kind,
                share: if total > 0 {
                    link.value_sats as f64 / total as f64 * 100.0
                } else {
                    0.0
                },
            })
        })
        .collect()
}

fn sort_link_rows(
    rows: &mut [FlowLinkRow],
    column: Option<FlowLinkSortColumn>,
    direction: SortDirection,
) {
    rows.sort_by(|left, right| {
        let order = match column.unwrap_or(FlowLinkSortColumn::Value) {
            FlowLinkSortColumn::Value => left
                .value_sats
                .cmp(&right.value_sats)
                .then_with(|| left.source_id.cmp(&right.source_id))
                .then_with(|| left.target_id.cmp(&right.target_id)),
            FlowLinkSortColumn::Source => left
                .source_label
                .to_lowercase()
                .cmp(&right.source_label.to_lowercase())
                .then_with(|| {
                    left.target_label
                        .to_lowercase()
                        .cmp(&right.target_label.to_lowercase())
                }),
            FlowLinkSortColumn::Target => left
                .target_label
                .to_lowercase()
                .cmp(&right.target_label.to_lowercase())
                .then_with(|| {
                    left.source_label
                        .to_lowercase()
                        .cmp(&right.source_label.to_lowercase())
                }),
            FlowLinkSortColumn::TxCount => left
                .event_count
                .cmp(&right.event_count)
                .then_with(|| left.value_sats.cmp(&right.value_sats)),
            FlowLinkSortColumn::FirstTime => compare_time(left.first_time, right.first_time),
            FlowLinkSortColumn::LastTime => compare_time(left.last_time, right.last_time),
        };
        match direction {
            SortDirection::Asc => order,
            SortDirection::Desc => order.reverse(),
        }
        .then_with(|| left.source_id.cmp(&right.source_id))
        .then_with(|| left.target_id.cmp(&right.target_id))
    });
}

fn sort_evidence_rows(
    rows: &mut [FlowEvidenceRow],
    column: Option<FlowEvidenceSortColumn>,
    direction: SortDirection,
) {
    rows.sort_by(|left, right| {
        let order = match column.unwrap_or(FlowEvidenceSortColumn::Date) {
            FlowEvidenceSortColumn::Date => compare_time(left.timestamp, right.timestamp),
            FlowEvidenceSortColumn::Value => left
                .value_sats
                .cmp(&right.value_sats)
                .then_with(|| compare_time(left.timestamp, right.timestamp)),
        };
        match direction {
            SortDirection::Asc => order,
            SortDirection::Desc => order.reverse(),
        }
        .then_with(|| left.txid.cmp(&right.txid))
        .then_with(|| left.ledger_id.cmp(&right.ledger_id))
    });
}

fn compare_time(left: Option<i64>, right: Option<i64>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::flow::{
        FlowEvidenceRef, FlowGraphStats, FlowKpis, FlowLink, FlowLinkKind, FlowNode, FlowNodeKind,
    };

    fn node(id: &str, label: &str) -> FlowNode {
        FlowNode {
            id: id.to_string(),
            label: label.to_string(),
            kind: FlowNodeKind::Address,
            owner: None,
            address: None,
            address_count: 1,
            transaction_count: 1,
            in_sats: 0,
            out_sats: 0,
            balance_sats: None,
            first_time: None,
            last_time: None,
            note: None,
        }
    }

    fn link(source: &str, target: &str, value_sats: i64) -> FlowLink {
        FlowLink {
            source: source.to_string(),
            target: target.to_string(),
            value_sats,
            event_count: 1,
            first_time: Some(10),
            last_time: Some(10),
            kind: FlowLinkKind::OnChain,
            evidence: vec![FlowEvidenceRef {
                txid: Some("tx".to_string()),
                ledger_id: None,
                timestamp: Some(10),
                value_sats,
                description: "test".to_string(),
            }],
        }
    }

    fn graph() -> FlowGraph {
        FlowGraph {
            nodes: vec![node("a", "A"), node("b", "B")],
            links: vec![link("a", "b", 10)],
            stats: FlowGraphStats::default(),
            kpis: FlowKpis::default(),
            exchange_activity: Default::default(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn link_rows_have_a_share_of_the_rendered_total() {
        let rows = link_rows(&graph());
        assert_eq!(rows.len(), 1);
        assert!((rows[0].share - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn link_rows_sort_with_a_deterministic_secondary_key() {
        let mut rows = link_rows(&graph());
        rows.push(FlowLinkRow {
            source_id: "b".to_string(),
            target_id: "a".to_string(),
            source_label: "B".to_string(),
            target_label: "A".to_string(),
            source_kind: FlowNodeKind::Address,
            target_kind: FlowNodeKind::Address,
            source_color: None,
            target_color: None,
            value_sats: 10,
            event_count: 1,
            first_time: Some(10),
            last_time: Some(10),
            kind: FlowLinkKind::OnChain,
            share: 0.0,
        });
        sort_link_rows(
            &mut rows,
            Some(FlowLinkSortColumn::Source),
            SortDirection::Asc,
        );
        assert_eq!(rows[0].source_label, "A");
        assert_eq!(rows[1].source_label, "B");
    }
}
