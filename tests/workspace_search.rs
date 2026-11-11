use test_log::test;
mod common;
use common::test_client;
use rusticnotion::models::search::{FilterProperty, FilterValue, NotionSearch};
use rusticnotion::models::Object;

#[test(tokio::test)]
async fn list_data_sources() -> Result<(), Box<dyn std::error::Error>> {
    let api = test_client();

    let response = api.list_data_sources().await?;
    assert!(!response.results.is_empty());

    Ok(())
}

#[test(tokio::test)]
async fn search_data_sources() -> Result<(), Box<dyn std::error::Error>> {
    let api = test_client();

    let response = api
        .search(NotionSearch::Filter {
            property: FilterProperty::Object,
            value: FilterValue::DataSource,
        })
        .await?;

    assert!(!response.results.is_empty());
    assert!(response
        .results
        .iter()
        .all(|object| matches!(object, Object::DataSource { .. })));

    Ok(())
}

#[test(tokio::test)]
async fn search_pages() -> Result<(), Box<dyn std::error::Error>> {
    let api = test_client();

    let response = api
        .search(NotionSearch::Filter {
            property: FilterProperty::Object,
            value: FilterValue::Page,
        })
        .await?;

    assert!(!response.results.is_empty());
    assert!(response
        .results
        .iter()
        .all(|object| matches!(object, Object::Page { .. })));

    Ok(())
}
