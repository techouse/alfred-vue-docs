use super::*;

fn hierarchy() -> SearchResultHierarchy {
    SearchResultHierarchy {
        lvl0: "Guide".to_owned(),
        lvl1: Some("Components".to_owned()),
        lvl2: None,
        lvl3: None,
        lvl4: None,
        lvl5: None,
        lvl6: None,
    }
}

#[test]
fn hierarchy_exposes_root_and_deepest_values() {
    let hierarchy = hierarchy();

    assert_eq!(hierarchy.first(), "Guide");
    assert_eq!(hierarchy.last(), "Components");
    assert_eq!(
        hierarchy.values().collect::<Vec<_>>(),
        vec!["Guide", "Components"]
    );
}

#[test]
fn hierarchy_level_returns_only_valid_levels() {
    let hierarchy = hierarchy();

    assert_eq!(hierarchy.level(0), Some("Guide"));
    assert_eq!(hierarchy.level(1), Some("Components"));
    assert_eq!(hierarchy.level(2), None);
    assert_eq!(hierarchy.level(7), None);
}

#[test]
fn search_result_deserializes_all_historical_fields() -> Result<()> {
    let result: SearchResult = serde_json::from_str(
        r#"{
          "objectID":"component",
          "type":"lvl2",
          "url":"https://vuejs.org/guide/component",
          "content":"content",
          "hierarchy":{
            "lvl0":"Guide",
            "lvl1":"Components",
            "lvl2":"Component Basics",
            "lvl3":null,
            "lvl4":null,
            "lvl5":null,
            "lvl6":null
          }
        }"#,
    )?;

    assert_eq!(
        (
            result.object_id,
            result.result_type,
            result.url,
            result.content,
            result.hierarchy.last(),
        ),
        (
            "component".to_owned(),
            "lvl2".to_owned(),
            "https://vuejs.org/guide/component".to_owned(),
            Some("content".to_owned()),
            "Component Basics",
        )
    );
    Ok(())
}
