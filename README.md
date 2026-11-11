# rusticnotion

[![Build](https://github.com/snuna/rusticnotion/actions/workflows/build.yml/badge.svg)](https://github.com/snuna/rusticnotion/actions/workflows/build.yml)
[![Crates.io](https://img.shields.io/crates/v/rusticnotion?style=for-the-badge)](https://crates.io/crates/rusticnotion)

Notion Offical API client library for rust.

Maintained fork, based on the awesome [jakeswenson/notion](https://github.com/jakeswenson/notion).

Under active development.

## How to use

Version 0.7.0 targets Notion API **2026-03-11**. Every client request sends
`Notion-Version: 2026-03-11`; the version is fixed by the library's models and
cannot be changed through client configuration. This is a breaking migration
from the previous `2022-06-28` API. See the migration guide below before upgrading.

During this phase we recommend using git for linking the library to your project.

```toml
rusticnotion = { git = "https://github.com/snuna/rusticnotion.git" }
```

Once Version 0.7.0 is released, we will publish the library to crates.io.

```toml
rusticnotion = "0.7.0"
```

## Migrating from 2022-06-28 to 2026-03-11

The migration includes the database/data source split introduced in
`2025-09-03`, plus the trash field changes in `2026-03-11`. A **database** is now
a container with one or more **data sources**. Each data source has its own
property schema and rows (pages). Database IDs and data source IDs are distinct
and must not be substituted for one another.

### Function and model changes

| Previous API | 0.7.0 API | Migration |
| --- | --- | --- |
| `get_database(database_id)` for schema | `get_data_source(data_source_id)` | Schema is now in `DataSource.properties`. |
| `get_database(database_id)` for metadata | `get_database(database_id)` | Returns the container and `Database.data_sources`, an array of `DataSourceReference { id, name }`. `Database.properties` was removed. |
| `query_database(database_id, query)` | `query_data_source(data_source_id, query)` | Uses `POST /v1/data_sources/{data_source_id}/query`. |
| `DatabaseQuery`, `DatabaseSort`, `DatabaseSortTimestamp` | `DataSourceQuery`, `DataSourceSort`, `DataSourceSortTimestamp` | Filtering, sorting, and cursor pagination keep the same structure. |
| `list_databases()` | `list_data_sources()` | Returns one page of data sources through search. |
| `NotionSearch::filter_by_databases()` | `NotionSearch::filter_by_data_sources()` | The search filter now sends `data_source`. |
| `FilterValue::Database` | `FilterValue::DataSource` | Match `Object::DataSource { data_source }` in search results. |
| `only_databases()` after search | `only_data_sources()` | Preserves the response's cursor and `has_more`. |
| `is_database()` after search | `is_data_source()` | Search returns pages and data sources, rather than database containers. |
| `Parent::Database { database_id }` when creating a row | `Parent::DataSource { data_source_id }` | `Parent::Database` still represents a data source's containing database in responses. |
| `Page.archived` | `Page.in_trash` | The `archived` response field was removed in API `2026-03-11`. |
| Flat relation schema fields | `Relation.data_source_id` and `Relation.kind` | `RelationType::SingleProperty` / `DualProperty` hold the nested relation settings. |

These are public Rust API changes; old query/list/filter names are not retained
as aliases. `Object::Database`, `only_databases()`, and `is_database()` remain
available for database objects, but search no longer returns those objects.
Database mentions in rich text still use `DatabaseId`.

### Discover and select a data source

Retrieve the database to discover its data sources, then select the intended
source explicitly. Do not automatically choose the first entry: a database can
contain multiple independent schemas. You can also copy a data source ID from
Notion's database menu under **Manage data sources**.

```rust
use rusticnotion::{
    ids::{DataSourceId, DatabaseId},
    models::{search::DataSourceQuery, ListResponse, Page},
    NotionApi,
};

async fn query_selected_source(
    api: &NotionApi,
    database_id: &DatabaseId,
    selected_source_id: &DataSourceId,
) -> Result<ListResponse<Page>, Box<dyn std::error::Error>> {
    let database = api.get_database(database_id).await?;
    let selected = database
        .data_sources
        .iter()
        .find(|source| &source.id == selected_source_id)
        .ok_or("Select a data source belonging to this database")?;

    let source = api.get_data_source(&selected.id).await?;
    // Inspect source.properties to discover its property schema.
    let pages = api
        .query_data_source(&source.id, DataSourceQuery::default())
        .await?;
    Ok(pages)
}
```

`query_data_source()` continues to return `ListResponse<Page>`. It sends
`result_type: "page"`, excluding nested data source results in wikis. Use
`models::paging::Pageable::start_from()` with `next_cursor` for subsequent pages;
the same pagination mechanism works with `SearchRequest`. For paginated data
source discovery, use `search(NotionSearch::filter_by_data_sources())`, convert
the filter to a `SearchRequest`, and apply the cursor. `list_data_sources()` is
a convenience call for the first page only.

### Create pages and read parents

When creating a database row, pass the selected **data source** as its parent.
Page-under-page creation still uses `Parent::Page`.

```rust
use rusticnotion::{
    ids::DataSourceId,
    models::{Page, PageCreateRequest, Parent, Properties},
    Error, NotionApi,
};

async fn create_row(
    api: &NotionApi,
    data_source_id: DataSourceId,
    properties: Properties,
) -> Result<Page, Error> {
    api.create_page(PageCreateRequest {
        parent: Parent::DataSource { data_source_id },
        properties,
        children: None,
    })
    .await
}
```

A page response's `data_source_id` parent may also contain a convenience
`database_id`. `Parent::DataSource` retains the data source ID and ignores that
additional field; retrieve the data source to inspect its database parent.
The `database_parent` field on `DataSource` describes the container's own parent.

### Trash status and relation schemas

Read `Page.in_trash`, `Database.in_trash`, `DataSource.in_trash`, and
`Block::common().in_trash` for trash status. Update code and saved JSON fixtures
that previously required `archived`.

Relation schemas now identify the target using `Relation.data_source_id`.
The optional `Relation.database_id` is response metadata and is omitted when
serializing. Synced property details are nested under
`RelationType::DualProperty { dual_property }`; one-way relations use
`RelationType::SingleProperty { single_property }`. Page relation **values**
remain arrays of page ID references.

### Scope of this migration

This release updates existing client functionality and adds data source
retrieval to replace legacy schema retrieval. It does not implement every
Notion endpoint. `get_page()`, `create_page()`, and the block read functions keep
their names. Block types without a dedicated variant, including `meeting_notes`,
continue to decode as `Block::Unknown`. The `after` to `position` change does not
affect this client because it does not yet expose an append-block endpoint.

Official references:

- [Upgrade to 2025-09-03](https://developers.notion.com/guides/get-started/upgrade-guide-2025-09-03)
- [Upgrade to 2026-03-11](https://developers.notion.com/guides/get-started/upgrade-guide-2026-03-11)
- [Database objects](https://developers.notion.com/reference/database)
- [Data source objects](https://developers.notion.com/reference/data-source)

## Docs

The generated documentation site is available here: https://docs.rs/rusticnotion/

## Building

```bash
cargo build
```

## Testing

To run integration tests, a env variable of `NOTION_API_TOKEN` must be set.
You can use the provided token in `example.env` by duplicating it to `.env`.

You can also create your own token [here](https://www.notion.so/my-integrations) with minimal permissions. And duplicating this [test page](https://snuna.notion.site/snuna/rusticnotion-test-b8b944b5cc3d444ea25ca7ddacd528cb) into your notion workspace. And then giving your integration only access to this page.

NOTE: While making the integration tests more useful, the template will probably change a lot.

The read-only integration tests also work with your own workspace. Share at least
one page and one database with the integration, and ensure the first data source
returned by search contains a page with a nonempty title. The query tests use
that data source's title property and page titles; they do not require specific
names or icons.

```bash
# Run all tests
cargo test

# Run tests with tracing enabled
RUST_LOG=debug cargo test

# Run tests without integration tests (offline)
cargo test --lib

```

## Contributing

Contributions are always welcome!
If you have an idea, it's best to float it by us before working on it to ensure no effort is wasted.
If there's already an open issue for it, knock yourself out.

If you have any questions, feel free to use [Discussions](https://github.com/snuna/rusticnotion/discussions).
Please don't hesitate to ask questions!
