use super::*;
use crate::models::{comment::CommentCreateTarget, requests::RichTextInput};
use crate::{
    ids::{DiscussionId, PageId},
    NOTION_API_VERSION,
};
use serde_json::{json, Value};
use wiremock::{
    matchers::{body_json, header, method, path, query_param, query_param_is_missing},
    Mock, MockBuilder, MockServer, ResponseTemplate,
};

fn endpoint(
    verb: &str,
    route: &str,
) -> MockBuilder {
    Mock::given(method(verb))
        .and(path(route))
        .and(header("Authorization", "Bearer test-token"))
        .and(header("Notion-Version", NOTION_API_VERSION))
}

fn api(server: &MockServer) -> NotionApi {
    NotionApi::with_base_url("test-token".into(), server.uri()).unwrap()
}

fn comment(id: &str) -> Value {
    json!({"object":"comment","id":id,"discussion_id":"discussion",
        "parent":{"type":"block_id","block_id":"block"},
        "created_time":"2026-01-01T00:00:00Z","last_edited_time":"2026-01-02T00:00:00Z",
        "created_by":{"object":"user","id":"author"},
        "display_name":{"type":"integration","resolved_name":"Integration"},
        "rich_text":[{"type":"text","plain_text":"你好","text":{"content":"你好","link":{"url":"https://example.com/"}},"annotations":{"bold":true}}],
        "attachments":[{"category":"image","file":{"url":"https://example.com/image"}}],
        "has_pending_attachments":true,"original_content_deleted":false,"future_field":{"keep":true}})
}

#[tokio::test]
async fn list_comments_preserves_metadata_and_supports_page_size_and_cursors() {
    let server = MockServer::start().await;
    endpoint("GET", "/comments")
        .and(query_param("block_id", "block")).and(query_param("page_size", "1"))
        .and(query_param_is_missing("start_cursor"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"object":"list","results":[comment("first")],"has_more":true,"next_cursor":"next"})))
        .expect(1).mount(&server).await;
    endpoint("GET", "/comments")
        .and(query_param("block_id", "block")).and(query_param("page_size", "100"))
        .and(query_param("start_cursor", "next"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"object":"list","results":[comment("second")],"has_more":false,"next_cursor":null})))
        .expect(1).mount(&server).await;
    let api = api(&server);
    let first = api
        .list_comments(
            "block".parse::<BlockId>().unwrap(),
            Paging {
                page_size: Some(1),
                ..Paging::default()
            },
        )
        .await
        .unwrap();
    assert!(first.has_more);
    let item = &first.results[0];
    assert_eq!(item.discussion_id.to_string(), "discussion");
    assert_eq!(item.created_by.as_ref().unwrap().id.to_string(), "author");
    assert_eq!(
        item.display_name.as_ref().unwrap().resolved_name,
        "Integration"
    );
    assert_eq!(
        item.rich_text[0]["text"]["link"]["url"],
        "https://example.com/"
    );
    assert_eq!(item.attachments.len(), 1);
    assert!(item.has_pending_attachments);
    assert_eq!(item.additional["future_field"]["keep"], true);
    let second = api
        .list_comments(
            "block".parse::<BlockId>().unwrap(),
            Paging {
                page_size: Some(100),
                start_cursor: first.next_cursor,
            },
        )
        .await
        .unwrap();
    assert_eq!(second.results[0].id.to_string(), "second");
    assert!(!second.has_more);
}

#[tokio::test]
async fn retrieve_comment_retains_unknown_rich_text_and_parent_variants() {
    let server = MockServer::start().await;
    let mut body = comment("comment");
    body["parent"] = json!({"type":"page_id","page_id":"page"});
    body["rich_text"] = json!([
        {"type":"mention","plain_text":"@Person","mention":{"type":"user","user":{"id":"user"}}},
        {"type":"equation","plain_text":"x","equation":{"expression":"x"}},
        {"type":"future","plain_text":"future","future":{"keep":true}}
    ]);
    body["property_id"] = json!("prop");
    endpoint("GET", "/comments/comment")
        .respond_with(ResponseTemplate::new(200).set_body_json(body.clone()))
        .expect(1)
        .mount(&server)
        .await;
    let item = api(&server)
        .get_comment("comment".parse::<CommentId>().unwrap())
        .await
        .unwrap();
    assert!(matches!(item.parent, Some(CommentParent::Page { .. })));
    assert_eq!(
        item.rich_text,
        body["rich_text"].as_array().unwrap().clone()
    );
    assert_eq!(item.property_id.unwrap().to_string(), "prop");
}

#[tokio::test]
async fn create_comments_serializes_exactly_one_target_and_handles_partial_responses() {
    let text = || vec![RichTextInput::from("你好\nComment")];
    let cases = [
        (
            CommentCreateRequest::on_block("block".parse::<BlockId>().unwrap(), text()),
            json!({"parent":{"block_id":"block"}}),
        ),
        (
            CommentCreateRequest::on_page("page".parse::<PageId>().unwrap(), text()),
            json!({"parent":{"page_id":"page"}}),
        ),
        (
            CommentCreateRequest::reply("discussion".parse::<DiscussionId>().unwrap(), text()),
            json!({"discussion_id":"discussion"}),
        ),
    ];
    for (request, mut expected) in cases {
        let server = MockServer::start().await;
        expected["rich_text"] = json!([{"type":"text","text":{"content":"你好\nComment"}}]);
        endpoint("POST", "/comments")
            .and(body_json(expected))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"object":"comment","id":"created"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let response = api(&server).create_comment(request).await.unwrap();
        assert_eq!(response.id.to_string(), "created");
        assert_eq!(response.additional["object"], "comment");
    }
}

#[tokio::test]
async fn update_and_delete_comments_use_typed_ids_and_retain_full_write_responses() {
    let server = MockServer::start().await;
    endpoint("PATCH", "/comments/comment")
        .and(body_json(json!({"rich_text":[{"type":"text","text":{"content":"Updated","link":{"url":"https://example.com/"}},"annotations":{"bold":true,"code":true}}]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(comment("comment")))
        .expect(1).mount(&server).await;
    endpoint("DELETE", "/comments/comment")
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"object":"comment","id":"comment"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut text = RichTextInput::from("Updated");
    text.content = crate::models::requests::RichTextContent::Text {
        text: crate::models::requests::TextInput {
            content: "Updated".into(),
            link: Some(crate::models::text::Link {
                url: "https://example.com/".into(),
            }),
        },
    };
    text.annotations = Some(serde_json::from_value(json!({"bold":true,"code":true})).unwrap());
    let api = api(&server);
    let updated = api
        .update_comment(
            "comment".parse::<CommentId>().unwrap(),
            CommentUpdateRequest {
                rich_text: vec![text],
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.additional["discussion_id"], "discussion");
    let deleted = api.delete_comment(&updated).await.unwrap();
    assert_eq!(deleted.id.to_string(), "comment");
}

#[tokio::test]
async fn comment_errors_use_shared_types_and_writes_are_not_retried() {
    for status in [401, 403, 404, 429, 500] {
        let server = MockServer::start().await;
        let code = match status {
            401 => "unauthorized",
            403 => "restricted_resource",
            404 => "object_not_found",
            429 => "rate_limited",
            _ => "internal_server_error",
        };
        endpoint("POST", "/comments")
            .respond_with(ResponseTemplate::new(status).insert_header("Retry-After", "7")
                .set_body_json(json!({"object":"error","status":status,"code":code,"message":"Unavailable"})))
            .expect(1).mount(&server).await;
        let error = api(&server)
            .create_comment(CommentCreateRequest::on_block(
                "block".parse::<BlockId>().unwrap(),
                vec!["draft".into()],
            ))
            .await
            .unwrap_err();
        match error {
            Error::RateLimited { retry_after_secs } => assert_eq!(retry_after_secs, 7),
            Error::ApiError { error } => assert_eq!(error.status.code(), status),
            error => panic!("Unexpected error: {:?}", error),
        }
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn malformed_comment_responses_are_rejected() {
    let server = MockServer::start().await;
    endpoint("GET", "/comments/comment")
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"incomplete"})))
        .expect(1)
        .mount(&server)
        .await;
    endpoint("POST", "/comments")
        .respond_with(ResponseTemplate::new(200).set_body_string("invalid json"))
        .expect(1)
        .mount(&server)
        .await;
    assert!(matches!(
        api(&server)
            .get_comment("comment".parse::<CommentId>().unwrap())
            .await,
        Err(Error::JsonParseError { .. })
    ));
    let request = CommentCreateRequest {
        target: CommentCreateTarget::Parent {
            parent: CommentParent::Block {
                block_id: "block".parse().unwrap(),
            },
        },
        rich_text: vec!["draft".into()],
    };
    assert!(matches!(
        api(&server).create_comment(request).await,
        Err(Error::JsonParseError { .. })
    ));
}
