use alfred_workflow_rs::{Icon, Item, ItemText};
use anyhow::Result;
use htmlize::unescape;
use url::Url;

use crate::models::SearchResult;

const BREADCRUMB_MAX_LENGTH: usize = 75;
const ELLIPSIS: &str = "...";

/// Builds the placeholder shown before the user enters a search query.
pub fn placeholder_item() -> Item {
    Item::new("Search the Vue.js docs...").set_icon(Icon::new("icon.png"))
}

/// Converts Vue.js search results into Alfred items in the historical grouping order.
pub fn items_from_results(results: &[SearchResult]) -> Result<Vec<Item>> {
    let groups = group_results(results);
    groups
        .into_iter()
        .flat_map(|group| group.subgroups)
        .flat_map(|subgroup| subgroup.results)
        .map(item_from_result)
        .collect()
}

/// Builds the Google fallback shown when Algolia returns no hits.
pub fn google_fallback_item(query: &str) -> Result<Item> {
    let url = Url::parse_with_params(
        "https://www.google.com/search",
        [("q", format!("Vue.js {query}"))],
    )?;

    Ok(Item::builder("No matching answers found")
        .subtitle("Shall I try and search Google?")
        .arg(url.as_str())
        .text(ItemText::new(url.as_str()))
        .quick_look_url(url.as_str())
        .icon(Icon::new("google.png"))
        .valid(true)
        .build()?)
}

struct ResultGroup<'a> {
    name: &'a str,
    subgroups: Vec<SubtitleGroup<'a>>,
}

struct SubtitleGroup<'a> {
    subtitle: String,
    results: Vec<&'a SearchResult>,
}

fn group_results(results: &[SearchResult]) -> Vec<ResultGroup<'_>> {
    let mut groups: Vec<ResultGroup<'_>> = Vec::new();

    for result in results {
        let title = result.hierarchy.last();
        let subtitle = decoded_breadcrumb(result, title);
        let group_name = result.hierarchy.first();

        let group_index = groups.iter().position(|group| group.name == group_name);
        let group_index = match group_index {
            Some(index) => index,
            None => {
                groups.push(ResultGroup {
                    name: group_name,
                    subgroups: Vec::new(),
                });
                groups.len() - 1
            }
        };

        let group = &mut groups[group_index];
        if let Some(subgroup) = group
            .subgroups
            .iter_mut()
            .find(|subgroup| subgroup.subtitle == subtitle)
        {
            subgroup.results.push(result);
        } else {
            group.subgroups.push(SubtitleGroup {
                subtitle,
                results: vec![result],
            });
        }
    }

    groups
}

fn item_from_result(result: &SearchResult) -> Result<Item> {
    let title = result.hierarchy.last();
    let decoded_title = decode_html_text(title);
    Ok(Item::builder(&decoded_title)
        .uid(&result.object_id)
        .subtitle(breadcrumb(result, title))
        .arg(&result.url)
        .text(ItemText::new(&result.url).with_large_type(&decoded_title))
        .quick_look_url(&result.url)
        .icon(Icon::new("icon.png"))
        .valid(true)
        .build()?)
}

fn breadcrumb(result: &SearchResult, title: &str) -> String {
    truncate(&decoded_breadcrumb(result, title), BREADCRUMB_MAX_LENGTH)
}

fn decoded_breadcrumb(result: &SearchResult, title: &str) -> String {
    let raw = result
        .hierarchy
        .values()
        .filter(|value| *value != title)
        .collect::<Vec<_>>()
        .join(" > ");
    decode_html_text(&raw)
}

fn truncate(value: &str, max_length: usize) -> String {
    if value.chars().count() <= max_length {
        return value.to_owned();
    }

    let keep = max_length.saturating_sub(ELLIPSIS.chars().count());
    value.chars().take(keep).chain(ELLIPSIS.chars()).collect()
}

fn decode_html_text(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut segment_start = 0;
    let mut cursor = 0;

    while let Some(relative_start) = text[cursor..].find("&#") {
        let start = cursor + relative_start;
        let Some((end, character)) = legacy_numeric_reference(text, start) else {
            cursor = start + 2;
            continue;
        };

        decoded.push_str(unescape(&text[segment_start..start]).as_ref());
        if let Some(character) = character {
            decoded.push(character);
        } else {
            decoded.push_str(&text[start..end]);
        }
        cursor = end;
        segment_start = end;
    }

    decoded.push_str(unescape(&text[segment_start..]).as_ref());
    decoded
}

fn legacy_numeric_reference(text: &str, start: usize) -> Option<(usize, Option<char>)> {
    let bytes = text.as_bytes();
    let mut cursor = start + 2;
    let (radix, uppercase_x) = match bytes.get(cursor) {
        Some(b'x') => {
            cursor += 1;
            (16, false)
        }
        Some(b'X') => {
            cursor += 1;
            (16, true)
        }
        _ => (10, false),
    };
    let digits_start = cursor;
    while let Some(byte) = bytes.get(cursor) {
        let is_digit = if radix == 16 {
            byte.is_ascii_hexdigit()
        } else {
            byte.is_ascii_digit()
        };
        if !is_digit {
            break;
        }
        cursor += 1;
    }

    if cursor == digits_start {
        return None;
    }

    let terminated = bytes.get(cursor) == Some(&b';');
    let end = if terminated { cursor + 1 } else { cursor };
    if !terminated || uppercase_x {
        return Some((end, None));
    }

    let digits = &text[digits_start..cursor];
    let character = u32::from_str_radix(digits, radix)
        .ok()
        .and_then(char::from_u32);
    Some((end, character))
}

#[cfg(test)]
#[path = "tests/app.rs"]
mod tests;
