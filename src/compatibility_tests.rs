use super::*;
use crate::models::{paging::Pageable, search::SearchRequest};
use serde_json::{json, Value};
use wiremock::{
    matchers::{body_json, header, method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn api_headers_search_pagination_and_encoded_child_cursor() {
    let server = MockServer::start().await;
    let api = NotionApi::with_base_url("test-token".into(), server.uri()).unwrap();
    Mock::given(method("POST")).and(path("/search"))
        .and(header("Notion-Version", "2022-06-28"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(json!({"query":"Notes", "filter":{"property":"object","value":"page"},"start_cursor":"a&b?c+d"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"object":"list","results":[],"has_more":false,"next_cursor":null})))
        .expect(1).mount(&server).await;
    let cursor: PagingCursor = serde_json::from_value(json!("a&b?c+d")).unwrap();
    api.search(SearchRequest::pages("Notes").start_from(Some(cursor.clone())))
        .await
        .unwrap();
    Mock::given(method("GET"))
        .and(path("/blocks/block/children"))
        .and(query_param("start_cursor", "a&b?c+d"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"object":"list","results":[],"has_more":false,"next_cursor":null}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    api.get_block_children("block".parse::<BlockId>().unwrap(), Some(cursor))
        .await
        .unwrap();
}

#[test]
fn omitted_child_arrays_and_table_cells_decode() {
    let common: Value =
        serde_json::from_str(include_str!("models/block/tests/heading_1.json")).unwrap();
    for kind in [
        "paragraph",
        "quote",
        "bulleted_list_item",
        "numbered_list_item",
        "to_do",
        "toggle",
        "column_list",
        "column",
        "template",
        "synced_block",
        "table",
    ] {
        let mut value = common.clone();
        value.as_object_mut().unwrap().remove("heading_1");
        value["type"] = json!(kind);
        value["has_children"] = json!(true);
        value[kind] = json!({"rich_text":[],"color":"default","checked":false,
            "synced_from":null,"table_width":2,"has_column_header":false,"has_row_header":false});
        let block: Block =
            serde_json::from_value(value).unwrap_or_else(|e| panic!("{}: {}", kind, e));
        assert!(block.common().has_children);
    }
    let mut value = common;
    value["type"] = json!("table_row");
    value["table_row"] = json!({"cells":[[],[]]});
    assert!(matches!(
        serde_json::from_value::<Block>(value).unwrap(),
        Block::TableRow { .. }
    ));
}

#[tokio::test]
async fn shallow_block_does_not_fetch_children() {
    let server = MockServer::start().await;
    let mut block: Value =
        serde_json::from_str(include_str!("models/block/tests/heading_1.json")).unwrap();
    block["has_children"] = json!(true);
    Mock::given(method("GET"))
        .and(path("/blocks/block"))
        .respond_with(ResponseTemplate::new(200).set_body_json(block))
        .expect(1)
        .mount(&server)
        .await;
    let api = NotionApi::with_base_url("test".into(), server.uri()).unwrap();
    assert!(
        api.get_block_shallow("block".parse::<BlockId>().unwrap())
            .await
            .unwrap()
            .common()
            .has_children
    );
}

#[test]
fn page_and_database_mentions_decode_id_references() {
    for kind in ["page", "database"] {
        let value = json!({"type":"mention","plain_text":"Reference",
            "mention":{"type":kind,kind:{"id":"reference"}}});
        let text: crate::models::text::RichText = serde_json::from_value(value).unwrap();
        assert_eq!(text.plain_text(), "Reference");
    }
}

#[tokio::test]
async fn rate_limit_wait_time_is_preserved_without_retry() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/search"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "17")
                .set_body_json(
                    json!({"object":"error","status":429,"code":"rate_limited","message":"Wait"}),
                ),
        )
        .expect(1)
        .mount(&server)
        .await;
    let api = NotionApi::with_base_url("test".into(), server.uri()).unwrap();
    assert!(matches!(
        api.search(SearchRequest::pages("")).await,
        Err(Error::RateLimited {
            retry_after_secs: 17
        })
    ));
}

#[tokio::test]
async fn recursive_child_failures_return_errors() {
    let server = MockServer::start().await;
    let mut block: Value =
        serde_json::from_str(include_str!("models/block/tests/heading_1.json")).unwrap();
    block["has_children"] = json!(true);
    block["id"] = json!("block");
    Mock::given(method("GET"))
        .and(path("/blocks/block"))
        .respond_with(ResponseTemplate::new(200).set_body_json(block))
        .expect(1)
        .mount(&server)
        .await;
    let mut child: Value =
        serde_json::from_str(include_str!("models/block/tests/heading_1.json")).unwrap();
    child["id"] = json!("child");
    Mock::given(method("GET"))
        .and(path("/blocks/block/children"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"object":"list","results":[child],"has_more":false,"next_cursor":null}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/blocks/child"))
        .respond_with(ResponseTemplate::new(404).set_body_json(
            json!({"object":"error","status":404,"code":"object_not_found","message":"Gone"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let api = NotionApi::with_base_url("test".into(), server.uri()).unwrap();
    assert!(matches!(
        api.get_block("block".parse::<BlockId>().unwrap()).await,
        Err(Error::ApiError { .. })
    ));
}

#[test]
fn block_parent_round_trips_and_requires_an_id() {
    use crate::models::Parent;

    let value = json!({"type":"block_id","block_id":"toggle-block"});
    let parent: Parent = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        parent,
        Parent::Block {
            block_id: "toggle-block".parse().unwrap(),
        }
    );
    assert_eq!(serde_json::to_value(parent).unwrap(), value);
    assert!(serde_json::from_value::<Parent>(json!({"type":"block_id"})).is_err());
}

#[test]
fn verification_properties_decode_null_and_populated_values() {
    use crate::models::properties::{PropertyConfiguration, PropertyValue};

    let schema = json!({"id":"vrfy","type":"verification","verification":{}});
    assert!(matches!(
        serde_json::from_value::<PropertyConfiguration>(schema).unwrap(),
        PropertyConfiguration::Verification { .. }
    ));
    for verification in [
        Value::Null,
        json!({"state":"unverified","date":null,"verified_by":null}),
        json!({"state":"verified","date":{
            "start":"2026-09-01T09:00:00Z","end":"2026-10-01T09:00:00Z","time_zone":null
        },"verified_by":{"object":"user","id":"verifier"}}),
        json!({"state":"expired","date":null,"verified_by":null}),
    ] {
        let property = json!({"id":"vrfy","type":"verification","verification":verification});
        let decoded: PropertyValue = serde_json::from_value(property).unwrap();
        if let PropertyValue::Verification { verification: value, .. } = &decoded {
            assert_eq!(value.is_none(), verification.is_null());
            if let Some(value) = value {
                assert_eq!(value.state, verification["state"].as_str().unwrap());
            }
        } else {
            panic!("expected verification property");
        }
        let encoded = serde_json::to_value(decoded).unwrap();
        assert_eq!(encoded["type"], "verification");
        assert_eq!(encoded["verification"].is_null(), verification.is_null());
    }
}

#[test]
fn file_block_captions_and_names_stay_inside_the_file_object() {
    use crate::models::{block::CreateBlock, text::RichText};

    let common: Value =
        serde_json::from_str(include_str!("models/block/tests/heading_1.json")).unwrap();
    for kind in ["file", "external"] {
        for caption in [
            None,
            Some(json!([])),
            Some(json!([{"type":"text","plain_text":"Attachment caption",
                "text":{"content":"Attachment caption"},"annotations":{"bold":true}}])),
        ] {
            let mut value = common.clone();
            value.as_object_mut().unwrap().remove("heading_1");
            value["type"] = json!("file");
            value["file"] = json!({"type":kind,kind:{"url":"https://example.com/attachment.txt"}});
            if kind == "file" {
                value["file"][kind]["expiry_time"] = json!("2026-11-11T00:00:00Z");
            }
            if let Some(caption) = &caption {
                value["file"]["caption"] = caption.clone();
                value["file"]["name"] = json!("attachment.txt");
            }
            let block: Block = serde_json::from_value(value).unwrap();
            let encoded = serde_json::to_value(&block).unwrap();
            assert!(encoded.get("caption").is_none());
            let expected: Vec<RichText> =
                serde_json::from_value(caption.as_ref().cloned().unwrap_or_else(|| json!([])))
                    .unwrap();
            let actual: Vec<RichText> =
                serde_json::from_value(encoded["file"]["caption"].clone()).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(encoded["file"]["type"], kind);
            assert_eq!(
                encoded["file"][kind]["url"],
                "https://example.com/attachment.txt"
            );
            if caption.is_some() {
                assert_eq!(encoded["file"]["name"], "attachment.txt");
            } else {
                assert!(encoded["file"].get("name").is_none());
            }
            let create = serde_json::to_value(CreateBlock::from(block)).unwrap();
            assert!(create.get("caption").is_none());
            assert_eq!(create["file"], encoded["file"]);
        }
    }
}
