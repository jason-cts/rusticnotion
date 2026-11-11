use test_log::test;
mod common;
use common::test_client;
use rusticnotion::models::{
    properties::PropertyConfiguration,
    search::{
        DatabaseQuery, FilterCondition, FilterProperty, FilterValue, NotionSearch,
        PropertyCondition, TextCondition,
    },
    Object,
};

#[test(tokio::test)]
async fn get_database() -> Result<(), Box<dyn std::error::Error>> {
    let api = test_client();

    let response = api
        .search(NotionSearch::Filter {
            value: FilterValue::Database,
            property: FilterProperty::Object,
        })
        .await?;

    let db = response
        .results()
        .iter()
        .filter_map(|o| match o {
            Object::Database { database } => Some(database),
            _ => None,
        })
        .next()
        .expect("Test expected to find at least one database in notion")
        .clone();

    let db_result = api.get_database(&db.id).await?;

    assert_eq!(db, db_result);

    Ok(())
}

#[test(tokio::test)]
async fn query_database() -> Result<(), Box<dyn std::error::Error>> {
    let api = test_client();

    let response = api
        .search(NotionSearch::Filter {
            value: FilterValue::Database,
            property: FilterProperty::Object,
        })
        .await?;

    let db = response
        .results()
        .iter()
        .filter_map(|o| match o {
            Object::Database { database } => Some(database),
            _ => None,
        })
        .next()
        .expect("Test expected to find at least one database in notion")
        .clone();

    let title_property = db
        .properties
        .iter()
        .find_map(|(name, property)| match property {
            PropertyConfiguration::Title { .. } => Some(name.clone()),
            _ => None,
        })
        .expect("Test database must have a title property");

    let all_pages = api.query_database(&db.id, DatabaseQuery::default()).await?;
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
        .query_database(
            &db.id,
            DatabaseQuery {
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
        .query_database(
            &db.id,
            DatabaseQuery {
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
