use super::*;
use crate::models::property_item::{PropertyItemValue, RollupItemValue};
use serde_json::{json, Value};
use wiremock::{
    matchers::{body_json, header, method, path, query_param},
    Mock, MockBuilder, MockServer, ResponseTemplate,
};

fn fixture(name: &str) -> Value {
    serde_json::from_str(match name {
        "page" => include_str!("models/tests/page.json"),
        "database" => include_str!("models/tests/database.json"),
        "data_source" => include_str!("models/tests/data_source.json"),
        _ => include_str!("models/block/tests/heading_1.json"),
    })
    .unwrap()
}

fn endpoint(
    verb: &str,
    route: &str,
) -> MockBuilder {
    Mock::given(method(verb))
        .and(path(route))
        .and(header("Notion-Version", NOTION_API_VERSION))
        .and(header("Authorization", "Bearer test-token"))
}

fn api(server: &MockServer) -> NotionApi {
    NotionApi::with_base_url("test-token".into(), server.uri()).unwrap()
}

#[test]
fn write_responses_keep_audio_ids_unique_ids_and_updated_metadata() {
    let mut audio = fixture("block");
    audio["type"] = json!("audio");
    audio["audio"] = json!({"type":"external", "external":{"url":"https://example.com/audio.mp3"}, "caption":[]});
    let block: Block = serde_json::from_value(audio).unwrap();
    assert!(matches!(block, Block::Audio { .. }));
    assert_eq!(
        block.as_id().value(),
        "9e891834-6a03-475c-a2b8-421e17f0f3aa"
    );
    let mut page = fixture("page");
    page["properties"]["Task ID"] =
        json!({"id":"task", "type":"unique_id", "unique_id":{"prefix":"TASK", "number":42}});
    page["cover"] = json!({"type":"external", "external":{"url":"https://example.com/cover.png"}});
    page["is_locked"] = json!(true);
    let page: Page = serde_json::from_value(page).unwrap();
    assert!(page.cover.is_some());
    assert_eq!(page.is_locked, Some(true));
    assert!(
        matches!(&page.properties.properties["Task ID"], models::properties::PropertyValue::UniqueId { unique_id, .. } if unique_id.number == 42)
    );
    let mut source = fixture("data_source");
    source["properties"]["Task ID"] =
        json!({"id":"task", "type":"unique_id", "unique_id":{"prefix":"TASK"}});
    let source: DataSource = serde_json::from_value(source).unwrap();
    assert!(
        matches!(&source.properties["Task ID"], models::properties::PropertyConfiguration::UniqueId { unique_id, .. } if unique_id.prefix.as_deref() == Some("TASK"))
    );
    let mut database = fixture("database");
    database["description"] = json!([]);
    database["is_inline"] = json!(false);
    database["is_locked"] = json!(false);
    let database: Database = serde_json::from_value(database).unwrap();
    assert_eq!(database.is_inline, Some(false));
    assert_eq!(database.is_locked, Some(false));
}

fn property_list(
    kind: &str,
    values: Vec<Value>,
    cursor: Option<&str>,
    metadata: Value,
) -> Value {
    json!({"object":"list", "type":"property_item", "results":values,
        "has_more":cursor.is_some(), "next_cursor":cursor,
        "property_item":{"id":"prop", "type":kind,
            "next_url":cursor.map(|_| "https://api.notion.com/next"), kind:metadata}})
}

#[tokio::test]
async fn property_retrieval_handles_scalar_and_encoded_ids_without_path_injection() {
    let server = MockServer::start().await;
    endpoint("GET", "/pages/page/properties/p%3A%2F%3F%23%20")
        .and(query_param("page_size", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object":"property_item", "id":"p%3A%2F%3F%23%20", "type":"number", "number":null
        })))
        .expect(2)
        .mount(&server)
        .await;
    for id in ["p:/?# ", "p%3A%2F%3F%23%20"] {
        let response = api(&server)
            .get_page_property(
                "page".parse::<PageId>().unwrap(),
                id.parse::<PropertyId>().unwrap(),
                Paging {
                    page_size: Some(1),
                    ..Paging::default()
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            response,
            PagePropertyResponse::PropertyItem {
                property_item: models::property_item::PropertyItem {
                    value: PropertyItemValue::Number { number: None },
                    ..
                }
            }
        ));
    }
}

#[tokio::test]
async fn property_pagination_preserves_rollup_calculation_and_opaque_cursor() {
    let server = MockServer::start().await;
    let relation = json!({"object":"property_item", "id":"prop", "type":"relation",
        "relation":{"id":"related-page"}});
    endpoint("GET", "/pages/page/properties/prop")
        .and(query_param("page_size", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(property_list(
            "rollup",
            vec![relation.clone()],
            Some("opaque&cursor+/?"),
            json!({"type":"incomplete", "incomplete":{}, "function":"sum"}),
        )))
        .expect(1)
        .mount(&server)
        .await;
    let client = api(&server);
    let page_id = "page".parse::<PageId>().unwrap();
    let property_id = "prop".parse::<PropertyId>().unwrap();
    let first = client
        .get_page_property(
            &page_id,
            &property_id,
            Paging {
                page_size: Some(1),
                ..Paging::default()
            },
        )
        .await
        .unwrap();
    let list = match first {
        PagePropertyResponse::List { list } => list,
        _ => panic!(),
    };
    assert!(list.has_more);
    assert!(matches!(
        list.results[0].value,
        PropertyItemValue::Relation { .. }
    ));
    assert!(matches!(
        list.property_item.rollup.unwrap().value,
        RollupItemValue::Incomplete { .. }
    ));
    assert!(list.property_item.next_url.is_some());
    endpoint("GET", "/pages/page/properties/prop")
        .and(query_param("start_cursor", "opaque&cursor+/?"))
        .respond_with(ResponseTemplate::new(200).set_body_json(property_list(
            "rollup",
            vec![relation],
            None,
            json!({"type":"number", "number":13, "function":"sum"}),
        )))
        .expect(1)
        .with_priority(1)
        .mount(&server)
        .await;
    let last = client
        .get_page_property(
            page_id,
            property_id,
            Paging {
                start_cursor: list.next_cursor,
                page_size: None,
            },
        )
        .await
        .unwrap();
    let list = match last {
        PagePropertyResponse::List { list } => list,
        _ => panic!(),
    };
    assert!(!list.has_more);
    assert_eq!(list.next_cursor, None);
    assert_eq!(list.property_item.next_url, None);
    assert!(matches!(list.property_item.rollup.unwrap().value,
        RollupItemValue::Number { number: Some(n) } if n == 13.into()));
}

#[test]
fn paginated_properties_use_single_values_and_keep_empty_lists() {
    let rich_text = json!({"type":"text", "text":{"content":"one"}, "plain_text":"one"});
    for (kind, value) in [
        ("title", rich_text.clone()),
        ("rich_text", rich_text),
        ("people", json!({"object":"user", "id":"person"})),
        ("relation", json!({"id":"page"})),
    ] {
        let response: PagePropertyResponse = serde_json::from_value(property_list(
            kind,
            vec![json!({"object":"property_item", "id":"prop", "type":kind, kind:value})],
            None,
            json!({}),
        ))
        .unwrap();
        let list = match response {
            PagePropertyResponse::List { list } => list,
            _ => panic!(),
        };
        assert_eq!(list.results.len(), 1);
        assert_eq!(list.property_item.additional[kind], json!({}));
        let empty: PagePropertyResponse =
            serde_json::from_value(property_list(kind, vec![], None, json!({}))).unwrap();
        assert!(matches!(empty, PagePropertyResponse::List { list } if list.results.is_empty()));
    }
    for rollup in [
        json!({"type":"date", "date":{"start":"2026-03-11", "end":null}, "function":"latest_date"}),
        json!({"type":"array", "array":[], "function":"show_original"}),
        json!({"type":"unsupported", "unsupported":{}, "function":"sum"}),
    ] {
        serde_json::from_value::<PagePropertyResponse>(property_list(
            "rollup",
            vec![],
            None,
            rollup,
        ))
        .unwrap();
    }
}

#[tokio::test]
async fn page_update_sends_writable_values_explicit_nulls_and_false() {
    let server = MockServer::start().await;
    let mut response = fixture("page");
    response["in_trash"] = json!(false);
    response["properties"]["Name"]["title"] = json!([]);
    endpoint("PATCH", "/pages/page")
        .and(body_json(json!({"properties":{
            "Name":{"title":[{"type":"text", "text":{"content":"Renamed"}}]},
            "Website":{"url":null}, "Done":{"checkbox":false}, "Links":{"relation":[]},
            "State":{"status":{"name":"Done"}}, "Tags":{"multi_select":[]}
        }, "icon":null, "cover":null, "in_trash":false, "is_locked":false})))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .expect(1)
        .mount(&server)
        .await;
    let updated = api(&server)
        .update_page(
            "page".parse::<PageId>().unwrap(),
            PageUpdateRequest {
                properties: Some(
                    [
                        (
                            "Name".into(),
                            PagePropertyInput::Title(vec!["Renamed".into()]),
                        ),
                        ("Website".into(), PagePropertyInput::Url(None)),
                        ("Done".into(), PagePropertyInput::Checkbox(false)),
                        ("Links".into(), PagePropertyInput::Relation(vec![])),
                        (
                            "State".into(),
                            PagePropertyInput::Status(Some(SelectValueInput::Name {
                                name: "Done".into(),
                            })),
                        ),
                        ("Tags".into(), PagePropertyInput::MultiSelect(vec![])),
                    ]
                    .iter()
                    .cloned()
                    .collect(),
                ),
                icon: Some(None),
                cover: Some(None),
                in_trash: Some(false),
                is_locked: Some(false),
            },
        )
        .await
        .unwrap();
    assert!(!updated.in_trash);
    assert_eq!(
        serde_json::to_value(PageUpdateRequest::default()).unwrap(),
        json!({})
    );
}

#[tokio::test]
async fn append_supports_positions_nested_write_blocks_and_returns_created_blocks() {
    let server = MockServer::start().await;
    let client = api(&server);
    for (position, expected) in [
        (None, None),
        (Some(BlockPosition::Start), Some(json!({"type":"start"}))),
        (Some(BlockPosition::End), Some(json!({"type":"end"}))),
        (
            Some(BlockPosition::AfterBlock {
                after_block: IdReference {
                    id: "previous".parse().unwrap(),
                },
            }),
            Some(json!({"type":"after_block", "after_block":{"id":"previous"}})),
        ),
    ] {
        let mut body = json!({"children":[{"type":"paragraph", "paragraph":{
            "rich_text":[{"type":"text", "text":{"content":"Parent"}}],
            "children":[{"type":"paragraph", "paragraph":{"rich_text":[]}}]
        }}]});
        if let Some(expected) = expected {
            body["position"] = expected;
        }
        endpoint("PATCH", "/blocks/parent/children")
            .and(body_json(body))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "object":"list", "results":[fixture("block")], "has_more":false, "next_cursor":null
            })))
            .expect(1)
            .mount(&server)
            .await;
        let blocks = client
            .append_block_children(
                "parent".parse::<BlockId>().unwrap(),
                BlockAppendRequest {
                    position,
                    children: vec![BlockCreate::Paragraph {
                        paragraph: TextBlockInput {
                            rich_text: vec!["Parent".into()],
                            children: vec![BlockCreate::Paragraph {
                                paragraph: TextBlockInput::default(),
                            }],
                            ..TextBlockInput::default()
                        },
                    }],
                },
            )
            .await
            .unwrap();
        assert_eq!(blocks.results.len(), 1);
        assert!(matches!(blocks.results[0], Block::Heading1 { .. }));
    }
}

#[tokio::test]
async fn block_update_delete_and_restore_use_distinct_methods_and_partial_patches() {
    let server = MockServer::start().await;
    let client = api(&server);
    endpoint("PATCH", "/blocks/block")
        .and(body_json(json!({"to_do":{"checked":false}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("block")))
        .expect(1)
        .mount(&server)
        .await;
    client
        .update_block(
            "block".parse::<BlockId>().unwrap(),
            BlockUpdateRequest {
                block: Some(BlockUpdate::ToDo(TextBlockUpdate {
                    checked: Some(false),
                    ..Default::default()
                })),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let mut trashed = fixture("block");
    trashed["in_trash"] = json!(true);
    endpoint("DELETE", "/blocks/block")
        .respond_with(ResponseTemplate::new(200).set_body_json(trashed))
        .expect(1)
        .mount(&server)
        .await;
    assert!(
        client
            .delete_block("block".parse::<BlockId>().unwrap())
            .await
            .unwrap()
            .common()
            .in_trash
    );
    endpoint("PATCH", "/blocks/block")
        .and(body_json(json!({"in_trash":false})))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("block")))
        .expect(1)
        .mount(&server)
        .await;
    assert!(
        !client
            .update_block(
                "block".parse::<BlockId>().unwrap(),
                BlockUpdateRequest {
                    in_trash: Some(false),
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .common()
            .in_trash
    );
}

#[tokio::test]
async fn database_create_and_update_keep_schema_under_initial_data_source() {
    let server = MockServer::start().await;
    endpoint("POST", "/databases")
        .and(body_json(json!({
            "parent":{"type":"page_id", "page_id":"parent"},
            "title":[{"type":"text", "text":{"content":"Tasks"}}],
            "initial_data_source":{"properties":{"Name":{"title":{}}}}, "is_inline":false
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("database")))
        .expect(1)
        .mount(&server)
        .await;
    let mut create = DatabaseCreateRequest::new(DatabaseParent::PageId {
        page_id: "parent".parse().unwrap(),
    });
    create.title = Some(vec!["Tasks".into()]);
    create.is_inline = Some(false);
    create.initial_data_source = Some(InitialDataSource {
        properties: Some(
            [(
                "Name".into(),
                PropertySchemaType::Title(EmptyObject {}).into(),
            )]
            .iter()
            .cloned()
            .collect(),
        ),
    });
    let client = api(&server);
    let database = client.create_database(create).await.unwrap();
    assert_eq!(database.data_sources.len(), 2);
    endpoint("PATCH", "/databases/database-container")
        .and(body_json(json!({
            "title":[], "description":[], "icon":null, "cover":null, "in_trash":false
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("database")))
        .expect(1)
        .mount(&server)
        .await;
    client
        .update_database(
            database.id,
            DatabaseUpdateRequest {
                title: Some(vec![]),
                description: Some(vec![]),
                icon: Some(None),
                cover: Some(None),
                in_trash: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn data_source_create_and_schema_update_support_add_rename_remove_and_relation() {
    let server = MockServer::start().await;
    endpoint("POST", "/data_sources")
        .and(body_json(json!({
            "parent":{"database_id":"database-container"}, "properties":{"Name":{"title":{}}}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("data_source")))
        .expect(1)
        .mount(&server)
        .await;
    let client = api(&server);
    let source = client
        .create_data_source(DataSourceCreateRequest {
            parent: DataSourceParent {
                database_id: "database-container".parse().unwrap(),
            },
            properties: [(
                "Name".into(),
                PropertySchemaType::Title(EmptyObject {}).into(),
            )]
            .iter()
            .cloned()
            .collect(),
            title: None,
            icon: None,
        })
        .await
        .unwrap();
    let relation = models::properties::Relation {
        data_source_id: "related-source".parse().unwrap(),
        database_id: None,
        kind: models::properties::RelationType::SingleProperty {
            single_property: Default::default(),
        },
    };
    endpoint("PATCH", "/data_sources/projects-source").and(body_json(json!({
        "properties":{
            "old":{"name":"Renamed"}, "remove":null, "Done":{"checkbox":{}},
            "Related":{"relation":{"data_source_id":"related-source", "type":"single_property", "single_property":{}}}
        }, "icon":null, "parent":{"database_id":"other-container"}, "in_trash":false
    }))).respond_with(ResponseTemplate::new(200).set_body_json(fixture("data_source")))
        .expect(1).mount(&server).await;
    client
        .update_data_source(
            source.id,
            DataSourceUpdateRequest {
                properties: Some(
                    [
                        (
                            "old".into(),
                            Some(PropertySchema {
                                name: Some("Renamed".into()),
                                ..Default::default()
                            }),
                        ),
                        ("remove".into(), None),
                        (
                            "Done".into(),
                            Some(PropertySchemaType::Checkbox(EmptyObject {}).into()),
                        ),
                        (
                            "Related".into(),
                            Some(PropertySchemaType::Relation(relation).into()),
                        ),
                    ]
                    .iter()
                    .cloned()
                    .collect(),
                ),
                icon: Some(None),
                parent: Some(DataSourceParent {
                    database_id: "other-container".parse().unwrap(),
                }),
                in_trash: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn property_and_write_errors_preserve_api_errors_and_rate_limits_without_retry() {
    let server = MockServer::start().await;
    endpoint("GET", "/pages/page/properties/prop")
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "object":"error", "status":404, "code":"object_not_found", "message":"Missing property"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let error = api(&server)
        .get_page_property(
            "page".parse::<PageId>().unwrap(),
            "prop".parse::<PropertyId>().unwrap(),
            Paging::default(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, Error::ApiError { error } if error.status.code() == 404));
    endpoint("PATCH", "/pages/page")
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "7"))
        .expect(1)
        .mount(&server)
        .await;
    let error = api(&server)
        .update_page(
            "page".parse::<PageId>().unwrap(),
            PageUpdateRequest::default(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        Error::RateLimited {
            retry_after_secs: 7
        }
    ));
    endpoint("POST", "/data_sources")
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "object":"error", "status":400, "code":"validation_error", "message":"Invalid schema"
        })))
        .expect(1)
        .mount(&server)
        .await;
    assert!(
        matches!(api(&server).create_data_source(DataSourceCreateRequest {
        parent: DataSourceParent { database_id: "db".parse().unwrap() }, properties: Default::default(),
        title: None, icon: None,
    }).await, Err(Error::ApiError { error }) if error.status.code() == 400)
    );
}
