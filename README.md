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

This release updates existing client functionality, adds data source retrieval,
and implements the core property and write endpoints listed below. It does not
implement every Notion endpoint. `get_page()`, `create_page()`, and the block read
functions keep their names. Block types without a dedicated variant, including
`meeting_notes`, continue to decode as `Block::Unknown`. Block append uses the new
`position` parameter.

Official references:

- [Upgrade to 2025-09-03](https://developers.notion.com/guides/get-started/upgrade-guide-2025-09-03)
- [Upgrade to 2026-03-11](https://developers.notion.com/guides/get-started/upgrade-guide-2026-03-11)
- [Database objects](https://developers.notion.com/reference/database)
- [Data source objects](https://developers.notion.com/reference/data-source)

## Core property and write APIs

All methods use `Notion-Version: 2026-03-11` and the existing `Error` type,
including `Error::ApiError` and `Error::RateLimited { retry_after_secs }`.
Writes are sent once; the client does not automatically retry or split requests.

| Method | REST endpoint | Request / response |
| --- | --- | --- |
| `get_page_property(page, property, paging)` | `GET /pages/{id}/properties/{property_id}` | `Paging` → `PagePropertyResponse` |
| `update_page(page, update)` | `PATCH /pages/{id}` | `PageUpdateRequest` → `Page` |
| `append_block_children(block, append)` | `PATCH /blocks/{id}/children` | `BlockAppendRequest` → `ListResponse<Block>` |
| `update_block(block, update)` | `PATCH /blocks/{id}` | `BlockUpdateRequest` → `Block` |
| `delete_block(block)` | `DELETE /blocks/{id}` | → `Block` |
| `create_database(create)` | `POST /databases` | `DatabaseCreateRequest` → `Database` |
| `update_database(database, update)` | `PATCH /databases/{id}` | `DatabaseUpdateRequest` → `Database` |
| `create_data_source(create)` | `POST /data_sources` | `DataSourceCreateRequest` → `DataSource` |
| `update_data_source(source, update)` | `PATCH /data_sources/{id}` | `DataSourceUpdateRequest` → `DataSource` |

Write models live in `rusticnotion::models::requests`; property responses live in
`rusticnotion::models::property_item`. The new write models omit response-only
IDs, timestamps, and rich text `plain_text`. The existing `PageCreateRequest`
and `block::CreateBlock` remain available for existing page creation code;
new append requests use `requests::BlockCreate`, whose children are recursive
write payloads rather than retrieved blocks.

For update requests, `None` omits a field and leaves its value unchanged.
Nullable icon/cover fields use `Some(None)` to send JSON `null` and clear them;
`Some(Some(value))` sets them. `Some(false)` and `Some(vec![])` are sent as-is.
API validation, capabilities, and access requirements still apply.

### Retrieve complete page properties

Unlike the properties embedded in `get_page()`, this endpoint supports reading
all items of a long title, rich text, people, or relation property. A scalar
returns `PagePropertyResponse::PropertyItem`; paginated values return `List`,
even when empty. Each list result holds a **single** rich text/user/relation
value, not an array. Multi-select and files return a scalar item containing an array.
Raw and URL-encoded property IDs are accepted and encoded as one path segment.

```rust
use rusticnotion::{Error, NotionApi};
use rusticnotion::ids::{PageId, PropertyId};
use rusticnotion::models::paging::Paging;
use rusticnotion::models::property_item::PagePropertyResponse;

async fn read_property(api: &NotionApi, page: PageId, property: PropertyId) -> Result<(), Error> {
    let mut paging = Paging::default();
    loop {
        match api.get_page_property(&page, &property, paging).await? {
            PagePropertyResponse::PropertyItem { property_item } => {
                println!("{:?}", property_item.value);
                break;
            }
            PagePropertyResponse::List { list } => {
                println!("{:?}", list.results);
                if !list.has_more {
                    // A rollup's final calculation is here, not in its partial pages.
                    println!("{:?}", list.property_item.rollup);
                    break;
                }
                paging = Paging { start_cursor: list.next_cursor, page_size: None };
            }
        }
    }
    Ok(())
}
```

Rollup list metadata is retained in `list.property_item`, including `next_url`
and the typed calculation. `RollupItemValue::Incomplete` means more pages must
be read; `Unsupported` means Notion cannot compute that result. Use the returned
cursor through this client instead of following `next_url` with your token.

### Update page values

Use property names or IDs as map keys. `PagePropertyInput` contains writable
types; formulas, rollups, and generated metadata are read-only. Set scalar
values to `None` to clear nullable properties, or use empty arrays for title,
rich text, multi-select, people, files, and relations. Changing page content
requires the block methods rather than `update_page()`.

```rust
use std::collections::HashMap;
use rusticnotion::{Error, NotionApi};
use rusticnotion::ids::PageId;
use rusticnotion::models::requests::{PagePropertyInput, PageUpdateRequest};

async fn update_row(api: &NotionApi, page: PageId) -> Result<(), Error> {
    api.update_page(page, PageUpdateRequest {
        properties: Some(HashMap::from([
            ("Name".into(), PagePropertyInput::Title(vec!["Updated task".into()])),
            ("Done".into(), PagePropertyInput::Checkbox(true)),
            ("Website".into(), PagePropertyInput::Url(None)),
        ])),
        icon: Some(None),
        ..Default::default()
    }).await?;
    Ok(())
}
```

### Append, update, and delete blocks

Append accepts up to 100 children and up to two levels of nesting per API request.
Omit `position` (or use `End`) to append; use `Start` or
`AfterBlock { after_block: IdReference { id } }` to choose a position.
Updating a block replaces only the fields you supply; it cannot replace
children or change the block's type. Update child-page/database titles through
`update_page()` / `update_database()`.

```rust
use rusticnotion::{Error, NotionApi};
use rusticnotion::ids::BlockId;
use rusticnotion::models::requests::*;

async fn edit_blocks(api: &NotionApi, parent: BlockId) -> Result<(), Error> {
    let appended = api.append_block_children(parent, BlockAppendRequest {
        children: vec![BlockCreate::Paragraph { paragraph: TextBlockInput {
            rich_text: vec!["New paragraph".into()],
            ..Default::default()
        }}],
        position: Some(BlockPosition::Start),
    }).await?;
    let block = appended.results[0].common().id.clone();
    api.update_block(&block, BlockUpdateRequest {
        block: Some(BlockUpdate::Paragraph(TextBlockUpdate {
            rich_text: Some(vec!["Revised paragraph".into()]),
            ..Default::default()
        })),
        ..Default::default()
    }).await?;
    api.delete_block(&block).await?;
    // DELETE moves the block to trash. Restore it with the 2026 field name:
    api.update_block(block, BlockUpdateRequest {
        in_trash: Some(false), ..Default::default()
    }).await?;
    Ok(())
}
```

### Create and update databases and data sources

`create_database()` creates a container, its first data source, and a table view.
Supply the first schema under `initial_data_source.properties`. Use
`create_data_source()` to add another source to a container. Database updates
change container metadata; **schema updates belong to data sources**.

`PropertySchema` has optional `name`, `description`, and `kind`. A rename only
needs `name`; creating or changing a type uses `PropertySchemaType`. Simple
types take `EmptyObject {}`. Select options do not require server-generated IDs.
Relations target a `DataSourceId`, using the existing `Relation` model.

```rust
use std::collections::HashMap;
use rusticnotion::{Error, NotionApi};
use rusticnotion::ids::PageId;
use rusticnotion::models::requests::*;

async fn create_tables(api: &NotionApi, parent: PageId) -> Result<(), Error> {
    let mut create = DatabaseCreateRequest::new(DatabaseParent::PageId { page_id: parent });
    create.title = Some(vec!["Tasks".into()]);
    create.initial_data_source = Some(InitialDataSource {
        properties: Some(HashMap::from([
            ("Name".into(), PropertySchemaType::Title(EmptyObject {}).into()),
        ])),
    });
    let database = api.create_database(create).await?;
    api.update_database(&database.id, DatabaseUpdateRequest {
        title: Some(vec!["Team tasks".into()]), ..Default::default()
    }).await?;
    let source = api.create_data_source(DataSourceCreateRequest {
        parent: DataSourceParent { database_id: database.id },
        title: Some(vec!["Backlog".into()]),
        properties: HashMap::from([
            ("Name".into(), PropertySchemaType::Title(EmptyObject {}).into()),
            ("Done".into(), PropertySchemaType::Checkbox(EmptyObject {}).into()),
            ("Obsolete".into(), PropertySchemaType::Checkbox(EmptyObject {}).into()),
        ]),
        icon: None,
    }).await?;
    api.update_data_source(source.id, DataSourceUpdateRequest {
        properties: Some(HashMap::from([
            ("Done".into(), Some(PropertySchema {
                name: Some("Complete".into()), ..Default::default()
            })),
            ("Notes".into(), Some(PropertySchemaType::RichText(EmptyObject {}).into())),
            ("Obsolete".into(), None),
        ])),
        ..Default::default()
    }).await?;
    Ok(())
}
```

In schema updates, an absent key leaves that column unchanged; a key with
`None` removes it. An empty map removes nothing. The title property cannot be
removed or converted. These models cover the core write operations above;
template application, page moves, markdown endpoints, file uploading, and the
remaining Notion endpoints are outside this scope.

Official endpoint references: [page properties](https://developers.notion.com/reference/retrieve-a-page-property),
[page updates](https://developers.notion.com/reference/patch-page),
[block append](https://developers.notion.com/reference/patch-block-children),
[block updates](https://developers.notion.com/reference/update-a-block),
[block deletion](https://developers.notion.com/reference/delete-a-block),
[database creation](https://developers.notion.com/reference/create-database),
[database updates](https://developers.notion.com/reference/update-database),
[data source creation](https://developers.notion.com/reference/create-a-data-source),
and [data source updates](https://developers.notion.com/reference/update-a-data-source).

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

## Saved database views

The read APIs support saved list, table, board, and other database views on the
configured Notion API version. Use `list_views(database_id, Paging)` to discover
view IDs, then `get_view(view_id)` for each view's name, data source, saved filter,
sorts, property visibility, and grouping configuration. Listing returns identity
references rather than complete view definitions.

`create_view_query(view_id, Some(100))` executes the saved filter and sorts and
returns the first result page plus a query ID. Continue with
`get_view_query_results(view_id, query_id, Paging)` while `has_more` is true, then
call `delete_view_query(view_id, query_id)` to release the temporary cache. This
deletes the query cache only, not the saved view or its pages. Queries expire;
consumers should restart an expired snapshot rather than combine snapshots.

View results preserve raw properties and accept ID-only pages. Fetch partial
pages with `get_page` and complete truncated property values with
`get_page_property`. `get_user` can resolve partial people references when the
connection has the required user access. Unknown layouts and extra configuration
fields survive decoding so consumers can handle future additions gracefully.

## Contributing

Contributions are always welcome!
If you have an idea, it's best to float it by us before working on it to ensure no effort is wasted.
If there's already an open issue for it, knock yourself out.

If you have any questions, feel free to use [Discussions](https://github.com/snuna/rusticnotion/discussions).
Please don't hesitate to ask questions!
