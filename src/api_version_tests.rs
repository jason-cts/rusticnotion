use super::*;
use crate::models::{
    paging::{Pageable, Paging},
    properties::{PropertyConfiguration, Relation, RelationType},
    search::{DataSourceSort, DataSourceSortTimestamp, SortDirection},
    Parent, Properties,
};
use serde_json::{json, Value};
use wiremock::{
    matchers::{body_json, header, method, path},
    Mock, MockBuilder, MockServer, ResponseTemplate,
};

fn database() -> Value {
    serde_json::from_str(include_str!("models/tests/database.json")).unwrap()
}

fn data_source() -> Value {
    serde_json::from_str(include_str!("models/tests/data_source.json")).unwrap()
}

fn page() -> Value {
    let mut page: Value = serde_json::from_str(include_str!("models/tests/page.json")).unwrap();
    page["parent"] = json!({
        "type": "data_source_id", "data_source_id": "projects-source",
        "database_id": "database-container"
    });
    page
}

fn list(
    results: Vec<Value>,
    cursor: Option<&str>,
) -> Value {
    json!({"object":"list", "results":results, "next_cursor":cursor,
        "has_more":cursor.is_some(), "type":"page_or_data_source", "page_or_data_source":{}})
}

fn api_mock(
    verb: &str,
    endpoint: &str,
) -> MockBuilder {
    Mock::given(method(verb))
        .and(path(endpoint))
        .and(header("Notion-Version", "2026-03-11"))
        .and(header("Authorization", "Bearer test-token"))
}

#[tokio::test]
async fn database_discovery_and_schema_use_distinct_ids_and_endpoints() {
    let server = MockServer::start().await;
    api_mock("GET", "/databases/database-container")
        .respond_with(ResponseTemplate::new(200).set_body_json(database()))
        .expect(1)
        .mount(&server)
        .await;
    api_mock("GET", "/data_sources/projects-source")
        .respond_with(ResponseTemplate::new(200).set_body_json(data_source()))
        .expect(1)
        .mount(&server)
        .await;
    let api = NotionApi::with_base_url("test-token".into(), server.uri()).unwrap();
    let database = api
        .get_database("database-container".parse::<DatabaseId>().unwrap())
        .await
        .unwrap();
    assert_eq!(database.data_sources.len(), 2);
    assert!(!database.in_trash);
    // Explicitly choose the second source; neither ID is the database ID.
    let source = api
        .get_data_source(&database.data_sources[1].id)
        .await
        .unwrap();
    assert_eq!(source.id, database.data_sources[1].id);
    assert_eq!(source.title_plain_text(), "Projects");
    assert_eq!(source.database_parent, database.parent);
    assert!(matches!(
        source.properties["Name"],
        PropertyConfiguration::Title { .. }
    ));
    match &source.properties["Tasks"] {
        PropertyConfiguration::Relation { relation, .. } => {
            assert_eq!(relation.data_source_id.to_string(), "tasks-source");
            assert!(matches!(relation.kind, RelationType::DualProperty { .. }));
        }
        _ => panic!("Expected a relation schema"),
    }
}

#[tokio::test]
async fn search_decodes_mixed_results_and_data_source_pagination() {
    let server = MockServer::start().await;
    api_mock("POST", "/search")
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(
            vec![page(), data_source()],
            Some("opaque&cursor+next"),
        )))
        .expect(1)
        .mount(&server)
        .await;
    let filter = json!({"property":"object", "value":"data_source"});
    api_mock("POST", "/search")
        .and(body_json(json!({"filter":filter})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(list(vec![data_source()], Some("opaque&cursor+next"))),
        )
        .expect(1)
        .mount(&server)
        .await;
    api_mock("POST", "/search")
        .and(body_json(
            json!({"filter":filter, "start_cursor":"opaque&cursor+next"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![data_source()], None)))
        .expect(1)
        .mount(&server)
        .await;
    let api = NotionApi::with_base_url("test-token".into(), server.uri()).unwrap();
    let mixed = api.search(SearchRequest::default()).await.unwrap();
    assert!(matches!(mixed.results[0], Object::Page { .. }));
    assert!(mixed.results[1].is_data_source());
    let sources = mixed.only_data_sources();
    assert_eq!(sources.results.len(), 1);
    assert!(sources.has_more);
    let listed = api.list_data_sources().await.unwrap();
    assert_eq!(listed, sources);
    let request: SearchRequest = NotionSearch::filter_by_data_sources().into();
    let next = api
        .search(request.start_from(listed.next_cursor))
        .await
        .unwrap();
    assert!(!next.has_more);
    assert!(next.results[0].is_data_source());
}

#[tokio::test]
async fn query_preserves_filter_sort_cursor_and_page_only_result_contract() {
    use crate::models::search::{FilterCondition, PropertyCondition, TextCondition};

    let server = MockServer::start().await;
    api_mock("POST", "/data_sources/projects-source/query")
        .and(body_json(json!({
            "filter":{"property":"Description", "rich_text":{"contains":"migration"}},
            "sorts":[{"timestamp":"last_edited_time", "direction":"descending"}],
            "start_cursor":"opaque&cursor+next", "page_size":7, "result_type":"page"
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(list(vec![page()], Some("another-cursor"))),
        )
        .expect(1)
        .mount(&server)
        .await;
    let api = NotionApi::with_base_url("test-token".into(), server.uri()).unwrap();
    let result = api
        .query_data_source(
            "projects-source".parse::<DataSourceId>().unwrap(),
            DataSourceQuery {
                sorts: Some(vec![DataSourceSort {
                    property: None,
                    timestamp: Some(DataSourceSortTimestamp::LastEditedTime),
                    direction: SortDirection::Descending,
                }]),
                filter: Some(FilterCondition::Property {
                    property: "Description".into(),
                    condition: PropertyCondition::RichText(TextCondition::Contains(
                        "migration".into(),
                    )),
                }),
                paging: Some(Paging {
                    start_cursor: None,
                    page_size: Some(7),
                }),
            }
            .start_from(Some(
                serde_json::from_value(json!("opaque&cursor+next")).unwrap(),
            )),
        )
        .await
        .unwrap();
    assert_eq!(result.results.len(), 1);
    assert!(matches!(
        result.results[0].parent,
        Parent::DataSource { .. }
    ));
    assert!(result.has_more);
    assert_eq!(
        serde_json::to_value(result.next_cursor).unwrap(),
        "another-cursor"
    );
}

#[tokio::test]
async fn create_page_uses_data_source_parent_and_keeps_child_page_creation() {
    let server = MockServer::start().await;
    let api = NotionApi::with_base_url("test-token".into(), server.uri()).unwrap();
    for parent in [
        Parent::DataSource {
            data_source_id: "projects-source".parse().unwrap(),
        },
        Parent::Page {
            page_id: "parent-page".parse().unwrap(),
        },
    ] {
        let mut response = page();
        response["parent"] = serde_json::to_value(&parent).unwrap();
        api_mock("POST", "/pages")
            .and(body_json(
                json!({"parent": parent, "properties":{}, "children":[
                    {"type":"divider"}
                ]}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .expect(1)
            .mount(&server)
            .await;
        let result = api
            .create_page(PageCreateRequest {
                parent: parent.clone(),
                properties: Properties {
                    properties: Default::default(),
                },
                children: Some(vec![crate::models::block::CreateBlock::Divider]),
            })
            .await
            .unwrap();
        assert_eq!(result.parent, parent);
        assert!(!result.in_trash);
    }
}

#[tokio::test]
async fn page_and_block_trash_status_decode_without_archived() {
    let server = MockServer::start().await;
    let mut page = page();
    page["in_trash"] = json!(true);
    assert!(page.get("archived").is_none());
    api_mock("GET", "/pages/page")
        .respond_with(ResponseTemplate::new(200).set_body_json(page))
        .expect(1)
        .mount(&server)
        .await;
    let mut block: Value =
        serde_json::from_str(include_str!("models/block/tests/heading_1.json")).unwrap();
    block["in_trash"] = json!(true);
    assert!(block.get("archived").is_none());
    api_mock("GET", "/blocks/block")
        .respond_with(ResponseTemplate::new(200).set_body_json(block))
        .expect(1)
        .mount(&server)
        .await;
    let api = NotionApi::with_base_url("test-token".into(), server.uri()).unwrap();
    assert!(
        api.get_page("page".parse::<PageId>().unwrap())
            .await
            .unwrap()
            .in_trash
    );
    assert!(
        api.get_block("block".parse::<BlockId>().unwrap())
            .await
            .unwrap()
            .common()
            .in_trash
    );
}

#[test]
fn relations_decode_both_directions_and_serialize_data_source_targets() {
    for (kind, settings) in [
        ("single_property", json!({})),
        (
            "dual_property",
            json!({"synced_property_name":"Projects", "synced_property_id":"projects"}),
        ),
    ] {
        let value = json!({"database_id":"database-container", "data_source_id":"tasks-source",
            "type":kind, kind:settings});
        let relation: Relation = serde_json::from_value(value).unwrap();
        assert_eq!(
            relation.database_id.as_ref().unwrap().to_string(),
            "database-container"
        );
        let encoded = serde_json::to_value(relation).unwrap();
        assert_eq!(
            encoded,
            json!({"data_source_id":"tasks-source", "type":kind, kind:settings})
        );
    }
    let parent: Parent = serde_json::from_value(page()["parent"].clone()).unwrap();
    assert_eq!(
        serde_json::to_value(parent).unwrap(),
        json!({"type":"data_source_id", "data_source_id":"projects-source"})
    );
    assert!(serde_json::from_value::<Parent>(json!({"type":"data_source_id"})).is_err());
}
