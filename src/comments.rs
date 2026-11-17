//! Comment endpoints, using the client's shared authentication and error handling.
use crate::ids::{AsIdentifier, BlockId, CommentId};
use crate::models::{comment::*, paging::Paging, ListResponse};
use crate::{Error, NotionApi};

impl NotionApi {
    /// Lists one page of unresolved comments attached to a page or block.
    /// Use `Paging::start_cursor` to retrieve subsequent pages. Page IDs can be
    /// converted to `BlockId` using `BlockId::from(page_id)`.
    pub async fn list_comments<B: AsIdentifier<BlockId>>(
        &self,
        block: B,
        paging: Paging,
    ) -> Result<ListResponse<Comment>, Error> {
        self.make_request(
            self.client
                .get(format!("{}/comments", self.base_url))
                .query(&[("block_id", block.as_id().to_string())])
                .query(&paging),
        )
        .await
    }

    /// Retrieves one comment by ID. Requires read-comment capability.
    pub async fn get_comment<C: AsIdentifier<CommentId>>(
        &self,
        comment: C,
    ) -> Result<Comment, Error> {
        self.make_request(self.client.get(format!(
            "{}/comments/{}",
            self.base_url,
            comment.as_id()
        )))
        .await
    }

    /// Creates a page/block comment or replies to an existing discussion.
    /// Requires insert-comment capability. Writes are never automatically retried.
    pub async fn create_comment(
        &self,
        request: CommentCreateRequest,
    ) -> Result<CommentReference, Error> {
        self.make_request(
            self.client
                .post(format!("{}/comments", self.base_url))
                .json(&request),
        )
        .await
    }

    /// Replaces the comment's rich-text body, subject to Notion's permissions.
    pub async fn update_comment<C: AsIdentifier<CommentId>>(
        &self,
        comment: C,
        request: CommentUpdateRequest,
    ) -> Result<CommentReference, Error> {
        self.make_request(
            self.client
                .patch(format!("{}/comments/{}", self.base_url, comment.as_id()))
                .json(&request),
        )
        .await
    }

    /// Deletes a comment created by this connection. Deleting the final comment
    /// also removes its discussion. Writes are never automatically retried.
    pub async fn delete_comment<C: AsIdentifier<CommentId>>(
        &self,
        comment: C,
    ) -> Result<CommentReference, Error> {
        self.make_request(self.client.delete(format!(
            "{}/comments/{}",
            self.base_url,
            comment.as_id()
        )))
        .await
    }
}

#[cfg(test)]
mod tests;
