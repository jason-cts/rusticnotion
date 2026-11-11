use test_log::test;
mod common;
use common::test_client;
use rusticnotion::models::{
    properties::PropertyConfiguration,
    search::{
        DataSourceQuery, FilterCondition, FilterProperty, FilterValue, NotionSearch,
        PropertyCondition, TextCondition,
    },
    Object,
};

#[test(tokio::test)]
async fn get_database() -> Result<(), Box<dyn std::error::Error>> {
    let api = test_client();

    let response = api
        .search(NotionSearch::Filter {
            value: FilterValue::DataSource,
            property: FilterProperty::Object,
        })
        .await?;

    let db = response
        .results()
        .iter()
        .filter_map(|o| match o {
            Object::DataSource { data_source } => Some(data_source),
            _ => None,
        })
        .next()
        .expect("Test expected to find at least one data source in notion")
        .clone();

    let source = api.get_data_source(&db.id).await?;
    assert_eq!(db, source);
    let database_id = match &source.parent {
        rusticnotion::models::Parent::Database { database_id } => database_id,
        _ => panic!("Test expected a data source with a database parent"),
    };
    let database = api.get_database(database_id).await?;
    assert!(database
        .data_sources
        .iter()
        .any(|entry| entry.id == source.id));

    Ok(())
}

#[test(tokio::test)]
async fn query_data_source() -> Result<(), Box<dyn std::error::Error>> {
    let api = test_client();

    let response = api
        .search(NotionSearch::Filter {
            value: FilterValue::DataSource,
            property: FilterProperty::Object,
        })
        .await?;

    let db = response
        .results()
        .iter()
        .filter_map(|o| match o {
            Object::DataSource { data_source } => Some(data_source),
            _ => None,
        })
        .next()
        .expect("Test expected to find at least one data source in notion")
        .clone();

    let title_property = db
        .properties
        .iter()
        .find_map(|(name, property)| match property {
            PropertyConfiguration::Title { .. } => Some(name.clone()),
            _ => None,
        })
        .expect("Test database must have a title property");

    let all_pages = api
        .query_data_source(&db.id, DataSourceQuery::default())
        .await?;
    let (page_id, title) = all_pages
        .results()
        .iter()
        .find_map(|page| {
            page.properties
                .title()
                .filter(|title| !title.is_empty())
                .map(|title| (page.id.clone(), title))
        })
        .expect("Test database must contain at least one page with a nonempty title");

    let pages = api
        .query_data_source(
            &db.id,
            DataSourceQuery {
                filter: Some(FilterCondition::Property {
                    property: title_property.clone(),
                    condition: PropertyCondition::RichText(TextCondition::Contains(title.clone())),
                }),
                ..Default::default()
            },
        )
        .await?;

    assert!(
        pages.results().iter().any(|page| page.id == page_id),
        "Title filter must include the known matching page"
    );
    for page in pages.results() {
        assert!(page.properties.title().unwrap().contains(&title));
    }

    let excluded_pages = api
        .query_data_source(
            &db.id,
            DataSourceQuery {
                filter: Some(FilterCondition::Property {
                    property: title_property,
                    condition: PropertyCondition::RichText(TextCondition::DoesNotContain(
                        title.clone(),
                    )),
                }),
                ..Default::default()
            },
        )
        .await?;

    for page in excluded_pages.results() {
        assert_ne!(page.id, page_id);
        assert!(!page.properties.title().unwrap().contains(&title));
    }

    Ok(())
}
