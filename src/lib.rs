use crate::ids::{BlockId, DataSourceId, DatabaseId};
use crate::models::error::ErrorResponse;
use crate::models::paging::PagingCursor;
use crate::models::search::{DataSourceQuery, SearchRequest};
use crate::models::{DataSource, Database, ListResponse, Object, Page};
use ids::{AsIdentifier, Identifier, PageId, PropertyId};
use models::block::Block;
use models::paging::Paging;
use models::property_item::PagePropertyResponse;
use models::requests::*;
use models::search::NotionSearch;
use models::PageCreateRequest;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{header, Client, ClientBuilder, RequestBuilder};
use std::boxed::Box;
use std::option::Option;
use tracing::Instrument;

pub mod ids;
pub mod models;
mod views;

pub use chrono;

pub const NOTION_API_VERSION: &str = "2026-03-11";

/// An wrapper Error type for all errors produced by the [`NotionApi`](NotionApi) client.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Rate limited; wait {retry_after_secs} seconds before trying again")]
    RateLimited { retry_after_secs: u64 },

    #[error("Invalid Notion API Token: {}", source)]
    InvalidApiToken { source: header::InvalidHeaderValue },

    #[error("Unable to build reqwest HTTP client: {}", source)]
    ErrorBuildingClient { source: reqwest::Error },

    #[error("Error sending HTTP request: {}", source)]
    RequestFailed {
        #[from]
        source: reqwest::Error,
    },

    #[error("Error reading response: {}", source)]
    ResponseIoError { source: reqwest::Error },

    #[error("Error parsing json response: {}", source)]
    JsonParseError { source: serde_json::Error },

    #[error("Unexpected API Response")]
    UnexpectedResponse { response: Object },

    #[error("API Error {}({}): {}", .error.code, .error.status, .error.message)]
    ApiError { error: ErrorResponse },
}

/// An API client for Notion.
/// Create a client by using [new(api_token: String)](Self::new()).
#[derive(Clone)]
pub struct NotionApi {
    client: Client,
    base_url: String,
}

impl NotionApi {
    /// Creates an instance of NotionApi.
    /// May fail if the provided api_token is an improper value.
    pub fn new(api_token: String) -> Result<Self, Error> {
        Self::with_base_url(api_token, "https://api.notion.com/v1")
    }

    /// Creates a client with an alternative API root (for proxies and local tests).
    /// Only use a trusted endpoint: requests carry the integration token.
    pub fn with_base_url(
        api_token: String,
        base_url: impl Into<String>,
    ) -> Result<Self, Error> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Notion-Version",
            HeaderValue::from_static(NOTION_API_VERSION),
        );

        let mut auth_value = HeaderValue::from_str(&format!("Bearer {}", api_token))
            .map_err(|source| Error::InvalidApiToken { source })?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        let client = ClientBuilder::new()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|source| Error::ErrorBuildingClient { source })?;

        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        })
    }

    async fn make_json_request(
        &self,
        request: RequestBuilder,
    ) -> Result<Object, Error> {
        self.make_request(request).await
    }

    async fn make_request<T: serde::de::DeserializeOwned>(
        &self,
        request: RequestBuilder,
    ) -> Result<T, Error> {
        let request = request.build()?;
        let url = request.url();
        tracing::trace!(
            method = request.method().as_str(),
            url = url.as_str(),
            "Sending request"
        );
        let response = self
            .client
            .execute(request)
            .instrument(tracing::trace_span!("Sending request"))
            .await
            .map_err(|source| Error::RequestFailed { source })?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after_secs = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok())
                .unwrap_or(1);
            return Err(Error::RateLimited { retry_after_secs });
        }
        let json = response
            .text()
            .instrument(tracing::trace_span!("Reading response"))
            .await
            .map_err(|source| Error::ResponseIoError { source })?;

        tracing::debug!("JSON Response: {}", json);
        #[cfg(test)]
        {
            dbg!(serde_json::from_str::<serde_json::Value>(&json)
                .map_err(|source| Error::JsonParseError { source })?);
        }
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|source| Error::JsonParseError { source })?;
        if value.get("object").and_then(serde_json::Value::as_str) == Some("error") {
            let error =
                serde_json::from_value(value).map_err(|source| Error::JsonParseError { source })?;
            return Err(Error::ApiError { error });
        }
        serde_json::from_value(value).map_err(|source| Error::JsonParseError { source })
    }

    /// Lists one page of data sources shared with the integration via search.
    /// Use [search()](Self::search()) with a data source filter and a cursor
    /// to retrieve subsequent pages.
    pub async fn list_data_sources(&self) -> Result<ListResponse<DataSource>, Error> {
        self.search(NotionSearch::filter_by_data_sources())
            .await
            .map(|response| response.only_data_sources())
    }

    /// Search pages and data sources by title.
    /// `query` can either be a [SearchRequest] or a slightly more convenient
    /// [NotionSearch](models::search::NotionSearch) query.
    pub async fn search<T: Into<SearchRequest>>(
        &self,
        query: T,
    ) -> Result<ListResponse<Object>, Error> {
        let result = self
            .make_json_request(
                self.client
                    .post(format!("{}/search", self.base_url))
                    .json(&query.into()),
            )
            .await?;

        match result {
            Object::List { list } => Ok(list),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Gets a database container and its child data source IDs by [DatabaseId].
    /// Use [get_data_source()](Self::get_data_source()) to retrieve a schema.
    pub async fn get_database<T: AsIdentifier<DatabaseId>>(
        &self,
        database_id: T,
    ) -> Result<Database, Error> {
        let result = self
            .make_json_request(self.client.get(format!(
                "{}/databases/{}",
                self.base_url,
                database_id.as_id()
            )))
            .await?;

        match result {
            Object::Database { database } => Ok(database),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Gets a data source, including its property schema, by [DataSourceId].
    pub async fn get_data_source<T: AsIdentifier<DataSourceId>>(
        &self,
        data_source_id: T,
    ) -> Result<DataSource, Error> {
        let result = self
            .make_json_request(self.client.get(format!(
                "{}/data_sources/{}",
                self.base_url,
                data_source_id.as_id()
            )))
            .await?;

        match result {
            Object::DataSource { data_source } => Ok(data_source),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Get a page by [PageId].
    pub async fn get_page<T: AsIdentifier<PageId>>(
        &self,
        page_id: T,
    ) -> Result<Page, Error> {
        let result = self
            .make_json_request(self.client.get(format!(
                "{}/pages/{}",
                self.base_url,
                page_id.as_id()
            )))
            .await?;

        match result {
            Object::Page { page } => Ok(page),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Creates a new page and return the created page
    pub async fn create_page<T: Into<PageCreateRequest>>(
        &self,
        page: T,
    ) -> Result<Page, Error> {
        let result = self
            .make_json_request(
                self.client
                    .post(format!("{}/pages", self.base_url))
                    .json(&page.into()),
            )
            .await?;

        match result {
            Object::Page { page } => Ok(page),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Queries a specific data source and returns one page of matching pages.
    /// Sets `result_type: "page"` so wiki data source results are excluded.
    pub async fn query_data_source<D, T>(
        &self,
        data_source: D,
        query: T,
    ) -> Result<ListResponse<Page>, Error>
    where
        T: Into<DataSourceQuery>,
        D: AsIdentifier<DataSourceId>,
    {
        #[derive(serde::Serialize)]
        struct PageQuery {
            #[serde(flatten)]
            query: DataSourceQuery,
            result_type: &'static str,
        }

        let result = self
            .make_json_request(
                self.client
                    .post(format!(
                        "{}/data_sources/{data_source_id}/query",
                        self.base_url,
                        data_source_id = data_source.as_id()
                    ))
                    .json(&PageQuery {
                        query: query.into(),
                        result_type: "page",
                    }),
            )
            .await?;
        match result {
            Object::List { list } => Ok(list.expect_pages()?),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Retrieves one block without loading descendants.
    pub async fn get_block_shallow<T: AsIdentifier<BlockId>>(
        &self,
        block_id: T,
    ) -> Result<Block, Error> {
        let result = self
            .make_json_request(self.client.get(format!(
                "{}/blocks/{}",
                self.base_url,
                block_id.as_id()
            )))
            .await?;
        match result {
            Object::Block { block } => Ok(block),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    pub async fn get_block<T: AsIdentifier<BlockId>>(
        &self,
        block_id: T,
    ) -> Result<Block, Error> {
        let mut block = self.get_block_shallow(block_id).await?;
        if matches!(block, Block::Unknown) || !block.common().has_children {
            return Ok(block);
        }
        let mut children = Vec::new();
        let mut cursor = None;
        loop {
            let response = self.get_block_children(&block.common().id, cursor).await?;
            for child in response.results {
                if matches!(child, Block::Unknown) {
                    children.push(Box::new(child));
                } else {
                    children.push(Box::new(
                        Box::pin(self.get_block(&child.common().id)).await?,
                    ));
                }
            }
            if !response.has_more {
                break;
            }
            cursor = response.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        block.set_children(children);
        Ok(block)
    }

    pub async fn get_block_children<T>(
        &self,
        block_id: T,
        cursor: Option<PagingCursor>,
    ) -> Result<ListResponse<Block>, Error>
    where
        T: AsIdentifier<BlockId>,
    {
        let mut request = self.client.get(format!(
            "{}/blocks/{}/children",
            self.base_url,
            block_id.as_id()
        ));
        if let Some(cursor) = cursor {
            request = request.query(&[("start_cursor", cursor.0)]);
        }
        let result = self.make_json_request(request).await?;

        match result {
            Object::List { list } => Ok(list.expect_blocks()?),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Retrieves one property value or one page of property items (including rollup metadata).
    /// IDs may be raw or URL-encoded; they are encoded as a single path segment.
    /// Continue lists with `list.next_cursor` until `list.has_more` is false.
    pub async fn get_page_property<P, R>(
        &self,
        page_id: P,
        property_id: R,
        paging: Paging,
    ) -> Result<PagePropertyResponse, Error>
    where
        P: AsIdentifier<PageId>,
        R: AsIdentifier<PropertyId>,
    {
        let decoded =
            percent_encoding::percent_decode_str(property_id.as_id().value()).decode_utf8_lossy();
        let encoded =
            percent_encoding::utf8_percent_encode(&decoded, percent_encoding::NON_ALPHANUMERIC);
        self.make_request(
            self.client
                .get(format!(
                    "{}/pages/{}/properties/{}",
                    self.base_url,
                    page_id.as_id(),
                    encoded
                ))
                .query(&paging),
        )
        .await
    }

    /// Updates page values, icon/cover, lock state, or trash state.
    /// Omitted fields are unchanged; nullable fields can be explicitly cleared.
    pub async fn update_page<P: AsIdentifier<PageId>>(
        &self,
        page_id: P,
        update: PageUpdateRequest,
    ) -> Result<Page, Error> {
        let result = self
            .make_json_request(
                self.client
                    .patch(format!("{}/pages/{}", self.base_url, page_id.as_id()))
                    .json(&update),
            )
            .await?;
        match result {
            Object::Page { page } => Ok(page),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Appends up to 100 children in one request. Uses the 2026 `position` shape.
    /// Returns the newly created first-level blocks. Requests are not split or retried.
    pub async fn append_block_children<B: AsIdentifier<BlockId>>(
        &self,
        block_id: B,
        append: BlockAppendRequest,
    ) -> Result<ListResponse<Block>, Error> {
        let result = self
            .make_json_request(
                self.client
                    .patch(format!(
                        "{}/blocks/{}/children",
                        self.base_url,
                        block_id.as_id()
                    ))
                    .json(&append),
            )
            .await?;
        match result {
            Object::List { list } => list.expect_blocks(),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Patches block content or trash state. Children must be appended separately.
    pub async fn update_block<B: AsIdentifier<BlockId>>(
        &self,
        block_id: B,
        update: BlockUpdateRequest,
    ) -> Result<Block, Error> {
        let result = self
            .make_json_request(
                self.client
                    .patch(format!("{}/blocks/{}", self.base_url, block_id.as_id()))
                    .json(&update),
            )
            .await?;
        match result {
            Object::Block { block } => Ok(block),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Moves a block to the trash and returns it. This is not permanent deletion.
    /// Restore with `update_block` and `in_trash: Some(false)`.
    pub async fn delete_block<B: AsIdentifier<BlockId>>(
        &self,
        block_id: B,
    ) -> Result<Block, Error> {
        let result = self
            .make_json_request(self.client.delete(format!(
                "{}/blocks/{}",
                self.base_url,
                block_id.as_id()
            )))
            .await?;
        match result {
            Object::Block { block } => Ok(block),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Creates a container and its initial data source. Schemas go under `initial_data_source`.
    pub async fn create_database(
        &self,
        create: DatabaseCreateRequest,
    ) -> Result<Database, Error> {
        let result = self
            .make_json_request(
                self.client
                    .post(format!("{}/databases", self.base_url))
                    .json(&create),
            )
            .await?;
        match result {
            Object::Database { database } => Ok(database),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Updates container metadata. Use `update_data_source` to change property schemas.
    pub async fn update_database<D: AsIdentifier<DatabaseId>>(
        &self,
        database_id: D,
        update: DatabaseUpdateRequest,
    ) -> Result<Database, Error> {
        let result = self
            .make_json_request(
                self.client
                    .patch(format!(
                        "{}/databases/{}",
                        self.base_url,
                        database_id.as_id()
                    ))
                    .json(&update),
            )
            .await?;
        match result {
            Object::Database { database } => Ok(database),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Creates another data source inside an existing database container.
    pub async fn create_data_source(
        &self,
        create: DataSourceCreateRequest,
    ) -> Result<DataSource, Error> {
        let result = self
            .make_json_request(
                self.client
                    .post(format!("{}/data_sources", self.base_url))
                    .json(&create),
            )
            .await?;
        match result {
            Object::DataSource { data_source } => Ok(data_source),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }

    /// Updates a data source's title, icon, schema, parent database, or trash state.
    /// In `properties`, null removes a column; a partial definition can rename it.
    pub async fn update_data_source<D: AsIdentifier<DataSourceId>>(
        &self,
        data_source_id: D,
        update: DataSourceUpdateRequest,
    ) -> Result<DataSource, Error> {
        let result = self
            .make_json_request(
                self.client
                    .patch(format!(
                        "{}/data_sources/{}",
                        self.base_url,
                        data_source_id.as_id()
                    ))
                    .json(&update),
            )
            .await?;
        match result {
            Object::DataSource { data_source } => Ok(data_source),
            response => Err(Error::UnexpectedResponse { response }),
        }
    }
}

#[cfg(test)]
mod compatibility_tests;

#[cfg(test)]
mod api_version_tests;

#[cfg(test)]
mod core_api_tests;
