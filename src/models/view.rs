//! Saved database views and their cached query results.
use super::paging::PagingCursor;
use crate::ids::{DataSourceId, PageId, ViewId, ViewQueryId};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ViewReference {
    pub id: ViewId,
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ViewList {
    pub results: Vec<ViewReference>,
    pub has_more: bool,
    pub next_cursor: Option<PagingCursor>,
}

/// Unknown view types remain readable; consumers choose which layouts to render.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct View {
    pub id: ViewId,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type")]
    pub view_type: String,
    pub data_source_id: Option<DataSourceId>,
    pub configuration: Option<ViewConfiguration>,
    pub filter: Option<Value>,
    pub sorts: Option<Vec<Value>>,
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ViewConfiguration {
    #[serde(rename = "type", default)]
    pub view_type: String,
    pub properties: Option<Vec<ViewProperty>>,
    pub group_by: Option<GroupBy>,
    pub sub_group_by: Option<GroupBy>,
    pub wrap_cells: Option<bool>,
    pub frozen_column_index: Option<usize>,
    pub show_vertical_lines: Option<bool>,
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ViewProperty {
    pub property_id: String,
    pub visible: Option<bool>,
    pub width: Option<u32>,
    pub wrap: Option<bool>,
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

/// `group_by` is a string mode, or a nested GroupBy for formula results.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GroupBy {
    #[serde(rename = "type")]
    pub property_type: String,
    #[serde(default)]
    pub property_id: String,
    pub group_by: Option<Value>,
    pub sort: Option<GroupSort>,
    pub hide_empty_groups: Option<bool>,
    pub start_day_of_week: Option<u8>,
    pub range_start: Option<f64>,
    pub range_end: Option<f64>,
    pub range_size: Option<f64>,
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GroupSort {
    #[serde(rename = "type")]
    pub sort_type: String,
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

/// Keeps raw properties, including truncation metadata, and accepts ID-only pages.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ViewPage {
    pub id: PageId,
    pub properties: Option<HashMap<String, Value>>,
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ViewQuery {
    pub id: ViewQueryId,
    pub view_id: ViewId,
    pub expires_at: Option<String>,
    pub total_count: Option<usize>,
    pub results: Vec<ViewPage>,
    pub has_more: bool,
    pub next_cursor: Option<PagingCursor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ViewQueryResults {
    pub results: Vec<ViewPage>,
    pub has_more: bool,
    pub next_cursor: Option<PagingCursor>,
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DeletedViewQuery {
    pub id: ViewQueryId,
    pub deleted: bool,
}
