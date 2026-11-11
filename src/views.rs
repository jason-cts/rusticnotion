//! Read saved views using the API version configured on NotionApi.
use crate::ids::{AsIdentifier, DatabaseId, ViewId, ViewQueryId};
use crate::models::{paging::Paging, view::*};
use crate::{Error, NotionApi};

impl NotionApi {
    /// Retrieves a user profile when the connection has user access.
    pub async fn get_user<U: AsIdentifier<crate::ids::UserId>>(
        &self,
        user: U,
    ) -> Result<crate::models::users::User, Error> {
        self.make_request(
            self.client
                .get(format!("{}/users/{}", self.base_url, user.as_id())),
        )
        .await
    }

    pub async fn list_views<D: AsIdentifier<DatabaseId>>(
        &self,
        database: D,
        paging: Paging,
    ) -> Result<ViewList, Error> {
        self.make_request(
            self.client
                .get(format!("{}/views", self.base_url))
                .query(&[("database_id", database.as_id().to_string())])
                .query(&paging),
        )
        .await
    }

    pub async fn get_view<V: AsIdentifier<ViewId>>(
        &self,
        view: V,
    ) -> Result<View, Error> {
        self.make_request(
            self.client
                .get(format!("{}/views/{}", self.base_url, view.as_id())),
        )
        .await
    }

    /// Executes the saved filter and sorts; does not modify the view or its pages.
    pub async fn create_view_query<V: AsIdentifier<ViewId>>(
        &self,
        view: V,
        page_size: Option<u8>,
    ) -> Result<ViewQuery, Error> {
        let paging = Paging {
            page_size,
            start_cursor: None,
        };
        self.make_request(
            self.client
                .post(format!("{}/views/{}/queries", self.base_url, view.as_id()))
                .json(&paging),
        )
        .await
    }

    pub async fn get_view_query_results<V: AsIdentifier<ViewId>, Q: AsIdentifier<ViewQueryId>>(
        &self,
        view: V,
        query: Q,
        paging: Paging,
    ) -> Result<ViewQueryResults, Error> {
        self.make_request(
            self.client
                .get(format!(
                    "{}/views/{}/queries/{}",
                    self.base_url,
                    view.as_id(),
                    query.as_id()
                ))
                .query(&paging),
        )
        .await
    }

    /// Deletes only the temporary query cache, never the saved view or pages.
    pub async fn delete_view_query<V: AsIdentifier<ViewId>, Q: AsIdentifier<ViewQueryId>>(
        &self,
        view: V,
        query: Q,
    ) -> Result<DeletedViewQuery, Error> {
        self.make_request(self.client.delete(format!(
            "{}/views/{}/queries/{}",
            self.base_url,
            view.as_id(),
            query.as_id()
        )))
        .await
    }
}

#[cfg(test)]
mod tests;
