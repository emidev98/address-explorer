//! Funds-flow endpoints.
#[cfg(feature = "server")]
use super::open_database;
#[cfg(feature = "server")]
use crate::application::flow::FlowService;
use crate::domain::flow::{
    FlowEvidenceQuery, FlowEvidenceRow, FlowFilterOptions, FlowLinkTableQuery, FlowOverview,
    FlowQuery,
};
use crate::domain::table::TablePage;
use dioxus::prelude::*;
#[server]
#[get("/api/flow/filter-options")]
pub async fn flow_filter_options_api() -> Result<FlowFilterOptions, ServerFnError> {
    remote_get!("/api/flow/filter-options", [], []);
    FlowService::connect(open_database()?)
        .filter_options()
        .map_err(|error| ServerFnError::new(error.to_string()))
}
#[server]
#[post("/api/flow/overview")]
pub async fn flow_overview_api(query: FlowQuery) -> Result<FlowOverview, ServerFnError> {
    remote_post!("/api/flow/overview", [], [], [query]);
    FlowService::connect(open_database()?)
        .overview(&query)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
#[server]
#[post("/api/flow/links")]
pub async fn flow_link_table_api(
    query: FlowLinkTableQuery,
) -> Result<TablePage<crate::domain::flow::FlowLinkRow>, ServerFnError> {
    remote_post!("/api/flow/links", [], [], [query]);
    FlowService::connect(open_database()?)
        .link_table(&query)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
#[server]
#[post("/api/flow/links/evidence")]
pub async fn flow_link_evidence_api(
    query: FlowEvidenceQuery,
) -> Result<TablePage<FlowEvidenceRow>, ServerFnError> {
    remote_post!("/api/flow/links/evidence", [], [], [query]);
    FlowService::connect(open_database()?)
        .evidence(&query)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
