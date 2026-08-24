use super::*;
use crate::models::SearchResultHierarchy;
use alfred_workflow_rs::Item;

fn result(id: &str, levels: &[&str], result_type: &str) -> SearchResult {
    let mut values = levels.iter().copied();
    let lvl0 = values.next().unwrap_or_default().to_owned();
    let mut optional = values.map(str::to_owned);
    SearchResult {
        object_id: id.to_owned(),
        result_type: result_type.to_owned(),
        url: format!("https://vuejs.org/guide/{id}"),
        hierarchy: SearchResultHierarchy {
            lvl0,
            lvl1: optional.next(),
            lvl2: optional.next(),
            lvl3: optional.next(),
            lvl4: optional.next(),
            lvl5: optional.next(),
            lvl6: optional.next(),
        },
        content: Some("content".to_owned()),
    }
}

#[test]
fn results_use_deepest_hierarchy_value_even_when_type_differs() -> Result<()> {
    let item =
        items_from_results(&[result("deep", &["Guide", "Components", "Slots"], "lvl1")])?.remove(0);

    assert_eq!(item.title(), "Slots");
    assert_eq!(item.subtitle(), Some("Guide > Components"));
    Ok(())
}

#[test]
fn results_are_grouped_by_first_seen_root_and_subtitle() -> Result<()> {
    let results = [
        result("root-b-first", &["B", "One"], "lvl1"),
        result("root-a", &["A", "One"], "lvl1"),
        result("root-b-second", &["B", "One"], "lvl1"),
        result("root-b-other", &["B", "Two"], "lvl1"),
    ];
    let items = items_from_results(&results)?;

    assert_eq!(
        items.iter().map(Item::uid).collect::<Vec<_>>(),
        vec![
            Some("root-b-first"),
            Some("root-b-second"),
            Some("root-b-other"),
            Some("root-a"),
        ]
    );
    Ok(())
}

#[test]
fn long_breadcrumbs_are_grouped_before_display_truncation() -> Result<()> {
    let shared_prefix = "a".repeat(72);
    let first_breadcrumb = format!("{shared_prefix}x");
    let second_breadcrumb = format!("{shared_prefix}y");
    let results = [
        result("first", &["Guide", &first_breadcrumb, "First"], "lvl2"),
        result("second", &["Guide", &second_breadcrumb, "Second"], "lvl2"),
    ];

    let groups = group_results(&results);

    assert_eq!(groups[0].subgroups.len(), 2);
    let items = items_from_results(&results)?;
    assert_eq!(items[0].subtitle(), items[1].subtitle());
    Ok(())
}

#[test]
fn level_zero_results_keep_an_empty_subtitle() -> Result<()> {
    let item = items_from_results(&[result("root", &["Guide"], "content")])?.remove(0);

    assert_eq!(item.title(), "Guide");
    assert_eq!(item.subtitle(), Some(""));
    Ok(())
}

#[test]
fn duplicate_raw_title_values_are_excluded_from_breadcrumbs() -> Result<()> {
    let item =
        items_from_results(&[result("duplicate", &["Guide", "Same", "Same"], "lvl2")])?.remove(0);

    assert_eq!(item.subtitle(), Some("Guide"));
    Ok(())
}

#[test]
fn titles_and_breadcrumbs_decode_dart_compatible_entities() -> Result<()> {
    let item = items_from_results(&[result(
        "entities",
        &["Guide &copy", "A &#38 and &#X26;", "C &#38; &#X26;"],
        "lvl2",
    )])?
    .remove(0);

    assert_eq!(item.title(), "C & &#X26;");
    assert_eq!(item.subtitle(), Some("Guide © > A &#38 and &#X26;"));
    assert_eq!(
        item.text().and_then(|text| text.large_type()),
        Some("C & &#X26;")
    );
    Ok(())
}

#[test]
fn breadcrumbs_truncate_to_75_unicode_scalars() -> Result<()> {
    let item = items_from_results(&[result(
        "long",
        &[&"A".repeat(50), &"🦀".repeat(50), "Title"],
        "lvl2",
    )])?
    .remove(0);
    let subtitle = item.subtitle().expect("subtitle must be present");

    assert_eq!(subtitle.chars().count(), 75);
    assert!(subtitle.ends_with("..."));
    Ok(())
}

#[test]
fn items_preserve_metadata_and_decoded_copy_title() -> Result<()> {
    let item =
        items_from_results(&[result("metadata", &["Guide", "A &amp; title"], "lvl6")])?.remove(0);

    assert_eq!(
        (
            item.uid(),
            item.arg(),
            item.quick_look_url(),
            item.icon().map(|icon| icon.path()),
            item.text().map(|text| text.copy()),
            item.text().and_then(|text| text.large_type()),
            item.valid(),
        ),
        (
            Some("metadata"),
            Some("https://vuejs.org/guide/metadata"),
            Some("https://vuejs.org/guide/metadata"),
            Some("icon.png"),
            Some("https://vuejs.org/guide/metadata"),
            Some("A & title"),
            true,
        )
    );
    Ok(())
}

#[test]
fn google_fallback_uses_vue_query_and_is_selectable() -> Result<()> {
    let item = google_fallback_item("composition api")?;

    assert_eq!(
        item.arg(),
        Some("https://www.google.com/search?q=Vue.js+composition+api")
    );
    assert_eq!(item.quick_look_url(), item.arg());
    assert_eq!(item.text().map(|text| text.copy()), item.arg());
    assert_eq!(item.icon().map(|icon| icon.path()), Some("google.png"));
    assert!(item.valid());
    Ok(())
}

#[test]
fn placeholder_is_not_selectable() {
    let item = placeholder_item();

    assert_eq!(item.title(), "Search the Vue.js docs...");
    assert_eq!(item.icon().map(|icon| icon.path()), Some("icon.png"));
    assert!(!item.valid());
}
