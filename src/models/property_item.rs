//! Responses from GET /pages/{page_id}/properties/{property_id}.
use super::paging::PagingCursor;
use super::properties::{
    DateValue, FileReference, FormulaResultValue, RelationValue, SelectedValue, UniqueIdValue,
    VerificationValue,
};
use super::text::RichText;
use super::users::User;
use super::{DateTime, Utc};
use crate::ids::PropertyId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "object", rename_all = "snake_case")]
pub enum PagePropertyResponse {
    PropertyItem {
        #[serde(flatten)]
        property_item: PropertyItem,
    },
    List {
        #[serde(flatten)]
        list: PropertyItemList,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PropertyItem {
    pub id: PropertyId,
    #[serde(flatten)]
    pub value: PropertyItemValue,
}

/// Unlike a page's property value, each paginated item contains only one value.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PropertyItemValue {
    Title {
        title: RichText,
    },
    RichText {
        rich_text: RichText,
    },
    People {
        people: User,
    },
    Relation {
        relation: RelationValue,
    },
    Number {
        number: Option<serde_json::Number>,
    },
    Select {
        select: Option<SelectedValue>,
    },
    Status {
        status: Option<SelectedValue>,
    },
    MultiSelect {
        multi_select: Vec<SelectedValue>,
    },
    Date {
        date: Option<DateValue>,
    },
    Formula {
        formula: FormulaResultValue,
    },
    Rollup {
        rollup: RollupItem,
    },
    Files {
        files: Vec<FileReference>,
    },
    Checkbox {
        checkbox: bool,
    },
    Url {
        url: Option<String>,
    },
    Email {
        email: Option<String>,
    },
    PhoneNumber {
        phone_number: Option<String>,
    },
    CreatedTime {
        created_time: DateTime<Utc>,
    },
    CreatedBy {
        created_by: User,
    },
    LastEditedTime {
        last_edited_time: DateTime<Utc>,
    },
    LastEditedBy {
        last_edited_by: User,
    },
    UniqueId {
        unique_id: UniqueIdValue,
    },
    Verification {
        verification: Option<VerificationValue>,
    },
    Button {},
}

/// Rollup calculations are final only on the last page of the property response.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RollupItem {
    pub function: String,
    #[serde(flatten)]
    pub value: RollupItemValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RollupItemValue {
    Number {
        number: Option<serde_json::Number>,
    },
    Date {
        date: Option<DateValue>,
    },
    Array {
        array: Vec<PropertyItemValue>,
    },
    Incomplete {
        incomplete: super::requests::EmptyObject,
    },
    Unsupported {
        unsupported: super::requests::EmptyObject,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PropertyItemList {
    pub results: Vec<PropertyItem>,
    pub has_more: bool,
    pub next_cursor: Option<PagingCursor>,
    pub property_item: PropertyItemMetadata,
}

/// List metadata has empty objects for title/text/people/relation. Rollups carry
/// the current calculation. Extra metadata is preserved for forward compatibility.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PropertyItemMetadata {
    pub id: PropertyId,
    pub next_url: Option<String>,
    #[serde(rename = "type")]
    pub property_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollup: Option<RollupItem>,
    #[serde(flatten)]
    pub additional: HashMap<String, serde_json::Value>,
}
