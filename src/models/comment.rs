//! Comments attached to pages, blocks, or existing discussion threads.
use super::{requests::RichTextInput, users::UserCommon, DateTime, Utc};
use crate::ids::{AsIdentifier, BlockId, CommentId, DiscussionId, PageId, PropertyId};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A complete comment returned by the list and retrieve endpoints.
///
/// Rich text and attachments are retained as JSON to preserve mentions, equations,
/// and future variants without losing data when an application displays a comment.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Comment {
    pub id: CommentId,
    pub discussion_id: DiscussionId,
    pub created_time: DateTime<Utc>,
    pub last_edited_time: DateTime<Utc>,
    pub created_by: Option<UserCommon>,
    pub parent: Option<CommentParent>,
    pub rich_text: Vec<Value>,
    #[serde(default)]
    pub attachments: Vec<Value>,
    pub display_name: Option<CommentDisplayName>,
    #[serde(default)]
    pub has_pending_attachments: bool,
    #[serde(default)]
    pub original_content_deleted: bool,
    pub property_id: Option<PropertyId>,
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

impl AsIdentifier<CommentId> for Comment {
    fn as_id(&self) -> &CommentId {
        &self.id
    }
}

impl AsIdentifier<CommentId> for &Comment {
    fn as_id(&self) -> &CommentId {
        &self.id
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommentDisplayName {
    #[serde(rename = "type")]
    pub name_type: Option<String>,
    pub resolved_name: String,
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

/// Page and block parents are also accepted without the optional `type` field.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum CommentParent {
    Page { page_id: PageId },
    Block { block_id: BlockId },
}

/// Exactly one target is serialized: a parent or an existing discussion.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum CommentCreateTarget {
    Parent { parent: CommentParent },
    Discussion { discussion_id: DiscussionId },
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CommentCreateRequest {
    #[serde(flatten)]
    pub target: CommentCreateTarget,
    pub rich_text: Vec<RichTextInput>,
}

impl CommentCreateRequest {
    pub fn on_block(
        block_id: BlockId,
        rich_text: Vec<RichTextInput>,
    ) -> Self {
        Self {
            target: CommentCreateTarget::Parent {
                parent: CommentParent::Block { block_id },
            },
            rich_text,
        }
    }

    pub fn on_page(
        page_id: PageId,
        rich_text: Vec<RichTextInput>,
    ) -> Self {
        Self {
            target: CommentCreateTarget::Parent {
                parent: CommentParent::Page { page_id },
            },
            rich_text,
        }
    }

    pub fn reply(
        discussion_id: DiscussionId,
        rich_text: Vec<RichTextInput>,
    ) -> Self {
        Self {
            target: CommentCreateTarget::Discussion { discussion_id },
            rich_text,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CommentUpdateRequest {
    pub rich_text: Vec<RichTextInput>,
}

/// Writes may return only `object` and `id` without read-comment capability.
/// A full response's fields are retained in `additional` when available.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CommentReference {
    pub id: CommentId,
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

impl AsIdentifier<CommentId> for CommentReference {
    fn as_id(&self) -> &CommentId {
        &self.id
    }
}

impl AsIdentifier<CommentId> for &CommentReference {
    fn as_id(&self) -> &CommentId {
        &self.id
    }
}
