use super::*;
use serde_json::json;
use wiremock::{
    matchers::{body_json, header, method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn saved_view_lifecycle_keeps_configuration_and_pagination() {
    let server = MockServer::start().await;
    let api = NotionApi::with_base_url("test".into(), server.uri()).unwrap();
    let database: DatabaseId = "database".parse().unwrap();
    let view: ViewId = "view".parse().unwrap();
    Mock::given(method("GET"))
        .and(path("/views"))
        .and(header("Notion-Version", crate::NOTION_API_VERSION))
        .and(header("Authorization", "Bearer test"))
        .and(query_param("database_id", "database"))
        .and(query_param("page_size", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"results":[{"object":"view","id":"view"}],"has_more":true,"next_cursor":"next"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let list = api
        .list_views(
            database,
            Paging {
                page_size: Some(50),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(list.has_more);
    assert_eq!(list.results[0].id, view);
    Mock::given(method("GET")).and(path("/views/view"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"view","name":"Tasks","type":"board","data_source_id":"source","configuration":{"type":"board","properties":[{"property_id":"title","visible":true}],"group_by":{"type":"formula","property_id":"f","group_by":{"type":"number","range_size":10},"sort":{"type":"ascending"}},"future":true},"future_view_field":42}))).expect(1).mount(&server).await;
    let saved = api.get_view(&view).await.unwrap();
    assert_eq!(saved.additional["future_view_field"], 42);
    let config = saved.configuration.unwrap();
    assert_eq!(config.additional["future"], true);
    assert_eq!(config.group_by.unwrap().group_by.unwrap()["range_size"], 10);
    Mock::given(method("POST")).and(path("/views/view/queries")).and(body_json(json!({"page_size":50})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"query","view_id":"view","total_count":2,"results":[{"id":"one"}],"has_more":true,"next_cursor":"next"}))).expect(1).mount(&server).await;
    let query = api.create_view_query(&view, Some(50)).await.unwrap();
    assert!(query.results[0].properties.is_none());
    Mock::given(method("GET")).and(path("/views/view/queries/query"))
        .and(query_param("start_cursor","next"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"results":[{"id":"two","properties":{"Related":{"type":"relation","has_more":true,"relation":[]}}}],"has_more":false,"next_cursor":null}))).expect(1).mount(&server).await;
    let next = api
        .get_view_query_results(
            &view,
            &query.id,
            Paging {
                start_cursor: query.next_cursor,
                page_size: Some(50),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        next.results[0].properties.as_ref().unwrap()["Related"]["has_more"],
        true
    );
    Mock::given(method("DELETE"))
        .and(path("/views/view/queries/query"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id":"query","deleted":true})),
        )
        .expect(1)
        .mount(&server)
        .await;
    assert!(
        api.delete_view_query(&view, query.id)
            .await
            .unwrap()
            .deleted
    );
}

#[test]
fn unknown_layouts_and_all_group_shapes_round_trip() {
    for kind in [
        "select",
        "multi_select",
        "status",
        "person",
        "created_by",
        "last_edited_by",
        "relation",
        "date",
        "created_time",
        "last_edited_time",
        "text",
        "title",
        "url",
        "email",
        "phone_number",
        "number",
        "checkbox",
        "formula",
        "future",
    ] {
        let group: GroupBy =
            serde_json::from_value(json!({"type":kind,"property_id":"p","future":1})).unwrap();
        assert_eq!(group.property_type, kind);
        assert_eq!(serde_json::to_value(group).unwrap()["future"], 1);
    }
    let view: View = serde_json::from_value(json!({"id":"v","type":"future"})).unwrap();
    assert_eq!(view.view_type, "future");
}

#[tokio::test]
async fn view_errors_use_the_existing_error_model() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/views/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_json(
            json!({"object":"error","status":404,"code":"object_not_found","message":"Not shared"}),
        ))
        .mount(&server)
        .await;
    let api = NotionApi::with_base_url("test".into(), server.uri()).unwrap();
    assert!(matches!(
        api.get_view("missing".parse::<ViewId>().unwrap()).await,
        Err(Error::ApiError { .. })
    ));
}

#[tokio::test]
async fn partial_user_profiles_can_be_resolved_by_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/u"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"object":"user","id":"u","name":"Alice","type":"person","person":{}}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let api = NotionApi::with_base_url("test".into(), server.uri()).unwrap();
    let user = api
        .get_user("u".parse::<crate::ids::UserId>().unwrap())
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(user).unwrap()["name"], "Alice");
}
