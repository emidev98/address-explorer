//! Browser wrappers for the funds-flow endpoints.

use crate::domain::flow::{
    FlowEvidenceQuery, FlowEvidenceRow, FlowFilterOptions, FlowLinkTableQuery, FlowOverview,
    FlowQuery,
};
use crate::domain::table::TablePage;
use dioxus_fullstack_core::ServerFnError;

pub async fn flow_filter_options_api() -> Result<FlowFilterOptions, ServerFnError> {
    crate::web_get_query!("flow.filter_options", "/api/flow/filter-options", {})
}

pub async fn flow_overview_api(query: FlowQuery) -> Result<FlowOverview, ServerFnError> {
    crate::web_post_body!("flow.overview", "/api/flow/overview", { "query": query })
}

pub async fn flow_link_table_api(
    query: FlowLinkTableQuery,
) -> Result<TablePage<crate::domain::flow::FlowLinkRow>, ServerFnError> {
    crate::web_post_body!("flow.links", "/api/flow/links", { "query": query })
}

pub async fn flow_link_evidence_api(
    query: FlowEvidenceQuery,
) -> Result<TablePage<FlowEvidenceRow>, ServerFnError> {
    crate::web_post_body!("flow.links_evidence", "/api/flow/links/evidence", { "query": query })
}
