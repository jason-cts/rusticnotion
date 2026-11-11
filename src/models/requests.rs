//! Write-only request models for the 2026-03-11 API.
//!
//! Optional fields are omitted. For nullable fields, `Some(None)` sends JSON null.
//! Response models contain server-owned IDs and timestamps; do not use them as patches.

use super::block::{CodeLanguage, ExternalFileObject, NativeIconObject};
use super::properties::{Color, DateValue, Formula, NumberDetails, Relation, RelationValue};
use super::text::{Annotations, Link, MentionObject, TextColor};
use crate::ids::{BlockId, DatabaseId, PageId, UserId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct IdReference<T> {
    pub id: T,
}

/// File uploads must already have been uploaded through Notion's file upload API.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct FileUploadReference {
    pub id: String,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FileInput {
    External { external: ExternalFileObject },
    FileUpload { file_upload: FileUploadReference },
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IconInput {
    Emoji { emoji: String },
    CustomEmoji { custom_emoji: IdReference<String> },
    Icon { icon: NativeIconObject },
    External { external: ExternalFileObject },
    FileUpload { file_upload: FileUploadReference },
}

/// Rich text without response-only `plain_text` and `href` fields.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct RichTextInput {
    #[serde(flatten)]
    pub content: RichTextContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RichTextContent {
    Text { text: TextInput },
    Mention { mention: MentionObject },
    Equation { equation: super::block::Equation },
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct TextInput {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
}

impl From<&str> for RichTextInput {
    fn from(content: &str) -> Self {
        Self::from(content.to_owned())
    }
}

impl From<String> for RichTextInput {
    fn from(content: String) -> Self {
        Self {
            content: RichTextContent::Text {
                text: TextInput {
                    content,
                    link: None,
                },
            },
            annotations: None,
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct FileValueInput {
    pub name: String,
    #[serde(flatten)]
    pub source: FileInput,
}

/// Writable page values, keyed by property name or ID in `PageUpdateRequest`.
/// Read-only formulas, rollups, and generated metadata are deliberately excluded.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PagePropertyInput {
    Title(Vec<RichTextInput>),
    RichText(Vec<RichTextInput>),
    Number(Option<serde_json::Number>),
    Select(Option<SelectValueInput>),
    Status(Option<SelectValueInput>),
    MultiSelect(Vec<SelectValueInput>),
    Date(Option<DateValue>),
    Relation(Vec<RelationValue>),
    People(Vec<IdReference<UserId>>),
    Files(Vec<FileValueInput>),
    Checkbox(bool),
    Url(Option<String>),
    Email(Option<String>),
    PhoneNumber(Option<String>),
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum SelectValueInput {
    Id { id: String },
    Name { name: String },
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct PageUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, PagePropertyInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<Option<IconInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<Option<FileInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_trash: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_locked: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct EmptyObject {}

/// A partial schema definition. Omit `kind` to rename an existing property only.
#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct PropertySchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Option<String>>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub kind: Option<PropertySchemaType>,
}

impl From<PropertySchemaType> for PropertySchema {
    fn from(kind: PropertySchemaType) -> Self {
        Self {
            kind: Some(kind),
            ..Self::default()
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PropertySchemaType {
    Title(EmptyObject),
    RichText(EmptyObject),
    Number(NumberDetails),
    Select(SelectSchema),
    MultiSelect(SelectSchema),
    Date(EmptyObject),
    People(EmptyObject),
    Files(EmptyObject),
    Checkbox(EmptyObject),
    Url(EmptyObject),
    Email(EmptyObject),
    PhoneNumber(EmptyObject),
    Formula(Formula),
    Relation(Relation),
    Rollup(RollupSchema),
    CreatedTime(EmptyObject),
    CreatedBy(EmptyObject),
    LastEditedTime(EmptyObject),
    LastEditedBy(EmptyObject),
    UniqueId(super::properties::UniqueIdConfiguration),
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct SelectSchema {
    pub options: Vec<SelectOptionInput>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct SelectOptionInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct RollupSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation_property_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation_property_id: Option<crate::ids::PropertyId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollup_property_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollup_property_id: Option<crate::ids::PropertyId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<super::properties::RollupFunction>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DatabaseParent {
    PageId { page_id: PageId },
    Workspace { workspace: bool },
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct DataSourceParent {
    pub database_id: DatabaseId,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct InitialDataSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, PropertySchema>>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct DatabaseCreateRequest {
    pub parent: DatabaseParent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Vec<RichTextInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Vec<RichTextInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_inline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_data_source: Option<InitialDataSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<IconInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<FileInput>,
}

impl DatabaseCreateRequest {
    pub fn new(parent: DatabaseParent) -> Self {
        Self {
            parent,
            title: None,
            description: None,
            is_inline: None,
            initial_data_source: None,
            icon: None,
            cover: None,
        }
    }
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct DatabaseUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<DatabaseParent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Vec<RichTextInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Vec<RichTextInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_inline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<Option<IconInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<Option<FileInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_trash: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_locked: Option<bool>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct DataSourceCreateRequest {
    pub parent: DataSourceParent,
    pub properties: HashMap<String, PropertySchema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Vec<RichTextInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<IconInput>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct DataSourceUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Vec<RichTextInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<Option<IconInput>>,
    /// Keys are property names or IDs. `None` removes a property; an empty map changes none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Option<PropertySchema>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<DataSourceParent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_trash: Option<bool>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockPosition {
    Start,
    End,
    AfterBlock { after_block: IdReference<BlockId> },
}

/// A block payload used for append. Child blocks use write models recursively.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockCreate {
    Paragraph {
        paragraph: TextBlockInput,
    },
    #[serde(rename = "heading_1")]
    Heading1 {
        heading_1: TextBlockInput,
    },
    #[serde(rename = "heading_2")]
    Heading2 {
        heading_2: TextBlockInput,
    },
    #[serde(rename = "heading_3")]
    Heading3 {
        heading_3: TextBlockInput,
    },
    BulletedListItem {
        bulleted_list_item: TextBlockInput,
    },
    NumberedListItem {
        numbered_list_item: TextBlockInput,
    },
    Quote {
        quote: TextBlockInput,
    },
    ToDo {
        to_do: TextBlockInput,
    },
    Toggle {
        toggle: TextBlockInput,
    },
    Callout {
        callout: TextBlockInput,
    },
    Code {
        code: CodeBlockInput,
    },
    Embed {
        embed: UrlBlockInput,
    },
    Bookmark {
        bookmark: UrlBlockInput,
    },
    Image {
        image: MediaBlockInput,
    },
    Video {
        video: MediaBlockInput,
    },
    Pdf {
        pdf: MediaBlockInput,
    },
    File {
        file: MediaBlockInput,
    },
    Audio {
        audio: MediaBlockInput,
    },
    Equation {
        equation: super::block::Equation,
    },
    Divider {
        divider: EmptyObject,
    },
    Breadcrumb {
        breadcrumb: EmptyObject,
    },
    TableOfContents {
        table_of_contents: ColorBlockInput,
    },
    ColumnList {
        column_list: ChildrenInput,
    },
    Column {
        column: ChildrenInput,
    },
    Table {
        table: TableInput,
    },
    TableRow {
        table_row: TableRowInput,
    },
    LinkToPage {
        link_to_page: super::block::LinkToPageFields,
    },
    SyncedBlock {
        synced_block: SyncedBlockInput,
    },
}

/// Only set fields valid for the enclosing block type. `children` is for append only.
#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct TextBlockInput {
    pub rich_text: Vec<RichTextInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<TextColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_toggleable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<IconInput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<BlockCreate>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct CodeBlockInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rich_text: Option<Vec<RichTextInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<CodeLanguage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<Vec<RichTextInput>>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct UrlBlockInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<Vec<RichTextInput>>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct MediaBlockInput {
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub source: Option<FileInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<Vec<RichTextInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct ColorBlockInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<TextColor>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct ChildrenInput {
    pub children: Vec<BlockCreate>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct TableInput {
    pub table_width: u64,
    pub has_column_header: bool,
    pub has_row_header: bool,
    pub children: Vec<BlockCreate>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct TableRowInput {
    pub cells: Vec<Vec<RichTextInput>>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncedBlockInput {
    pub synced_from: Option<SyncedBlockReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<BlockCreate>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncedBlockReference {
    BlockId { block_id: BlockId },
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct BlockAppendRequest {
    pub children: Vec<BlockCreate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<BlockPosition>,
}

/// Partial text update: `None` leaves content intact; `Some(vec![])` clears it.
#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct TextBlockUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rich_text: Option<Vec<RichTextInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<TextColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_toggleable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<IconInput>,
}

/// One block's content patch, with no `children` field.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlockUpdate {
    Paragraph(TextBlockUpdate),
    #[serde(rename = "heading_1")]
    Heading1(TextBlockUpdate),
    #[serde(rename = "heading_2")]
    Heading2(TextBlockUpdate),
    #[serde(rename = "heading_3")]
    Heading3(TextBlockUpdate),
    BulletedListItem(TextBlockUpdate),
    NumberedListItem(TextBlockUpdate),
    Quote(TextBlockUpdate),
    ToDo(TextBlockUpdate),
    Toggle(TextBlockUpdate),
    Callout(TextBlockUpdate),
    Code(CodeBlockInput),
    Embed(UrlBlockInput),
    Bookmark(UrlBlockInput),
    Image(MediaBlockInput),
    Video(MediaBlockInput),
    Pdf(MediaBlockInput),
    File(MediaBlockInput),
    Audio(MediaBlockInput),
    Equation(super::block::Equation),
    LinkToPage(super::block::LinkToPageFields),
    TableOfContents(ColorBlockInput),
    TableRow(TableRowInput),
    Table(TableUpdate),
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct TableUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_column_header: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_row_header: Option<bool>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct BlockUpdateRequest {
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub block: Option<BlockUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_trash: Option<bool>,
}
