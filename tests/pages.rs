use test_log::test;
mod common;
use common::test_client;
use rusticnotion::{
    ids::BlockId,
    models::{
        search::{FilterProperty, FilterValue, NotionSearch},
        Object,
    },
};

#[test(tokio::test)]
async fn get_block_children() -> Result<(), Box<dyn std::error::Error>> {
    let api = test_client();

    let search_response = api
        .search(NotionSearch::Filter {
            value: FilterValue::Page,
            property: FilterProperty::Object,
        })
        .await?;

    assert!(
        !search_response.results.is_empty(),
        "Test expected to find at least one page in notion"
    );

    for object in search_response.results {
        match object {
            Object::Page { page } => api.get_block_children(BlockId::from(page.id), None).await?,
            _ => panic!("Should not have received anything but pages!"),
        };
    }

    Ok(())
}
