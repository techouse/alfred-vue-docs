use std::cell::Cell;

use alfred_workflow_rs::{FileCache, Item, UserConfiguration, Workflow};

use super::*;

fn settings(version: &str) -> WorkflowSettings {
    WorkflowSettings {
        vue_version: version.to_owned(),
        use_alfred_cache: false,
        use_file_cache: false,
        cache_ttl: Some(86_400),
        file_cache_max_entries: Some(1_280),
    }
}

#[test]
fn plist_defaults_map_to_runtime_settings() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let info_path = directory.path().join("info.plist");
    let prefs_path = directory.path().join("missing-prefs.plist");
    std::fs::write(&info_path, include_str!("../../info.plist"))?;
    let workflow = Workflow::new();

    let defaults = workflow.get_user_defaults(&info_path, &prefs_path)?;
    let Some(UserConfiguration::Select(version)) = defaults.get("vue_version") else {
        panic!("vue_version must be a select configuration");
    };
    assert_eq!(version.config.default_value, "v3");
    assert_eq!(
        version
            .config
            .pairs
            .iter()
            .take(2)
            .map(|pair| (pair.label.as_str(), pair.value.as_str()))
            .collect::<Vec<_>>(),
        vec![("3", "v3"), ("2", "v2")]
    );

    let actual = read_workflow_settings(&workflow, info_path, prefs_path)?;

    assert_eq!(
        actual,
        WorkflowSettings {
            vue_version: "v3".to_owned(),
            use_alfred_cache: true,
            use_file_cache: false,
            cache_ttl: Some(86_400),
            file_cache_max_entries: Some(1_280),
        }
    );
    Ok(())
}

#[test]
fn missing_or_mistyped_optional_defaults_use_safe_values() -> Result<()> {
    let mut defaults = BTreeMap::new();
    defaults.insert(
        "vue_version".to_owned(),
        UserConfiguration::Select(alfred_workflow_rs::SelectUserConfiguration {
            variable: "vue_version".to_owned(),
            description: None,
            label: None,
            config: alfred_workflow_rs::SelectConfiguration {
                default_value: "v3".to_owned(),
                value: "v3".to_owned(),
                pairs: Vec::new(),
            },
        }),
    );
    defaults.insert(
        "cache_ttl".to_owned(),
        UserConfiguration::TextField(alfred_workflow_rs::TextFieldUserConfiguration {
            variable: "cache_ttl".to_owned(),
            description: None,
            label: None,
            config: alfred_workflow_rs::TextFieldConfiguration {
                default_value: "".to_owned(),
                value: "".to_owned(),
                placeholder: None,
                required: false,
                trim: false,
            },
        }),
    );

    assert_eq!(
        workflow_settings_from_defaults(&defaults)?,
        WorkflowSettings {
            vue_version: "v3".to_owned(),
            use_alfred_cache: false,
            use_file_cache: false,
            cache_ttl: None,
            file_cache_max_entries: None,
        }
    );
    Ok(())
}

#[test]
fn missing_required_version_reports_exact_error() {
    let error = workflow_settings_from_defaults(&BTreeMap::new())
        .expect_err("Vue version must be required");

    assert_eq!(error.to_string(), "vue_version not set!");
}

#[test]
fn negative_slider_values_are_rejected_by_checked_conversion() -> Result<()> {
    let mut defaults = BTreeMap::new();
    defaults.insert(
        "vue_version".to_owned(),
        UserConfiguration::Select(alfred_workflow_rs::SelectUserConfiguration {
            variable: "vue_version".to_owned(),
            description: None,
            label: None,
            config: alfred_workflow_rs::SelectConfiguration {
                default_value: "v3".to_owned(),
                value: "v3".to_owned(),
                pairs: Vec::new(),
            },
        }),
    );
    defaults.insert(
        "cache_ttl".to_owned(),
        UserConfiguration::NumberSlider(alfred_workflow_rs::NumberSliderUserConfiguration {
            variable: "cache_ttl".to_owned(),
            description: None,
            label: None,
            config: alfred_workflow_rs::NumberSliderConfiguration {
                default_value: -1,
                value: -1,
                min: -10,
                max: 10,
                show_markers: false,
                only_stop_on_markers: false,
                marker_count: None,
            },
        }),
    );

    assert_eq!(workflow_settings_from_defaults(&defaults)?.cache_ttl, None);
    Ok(())
}

#[test]
fn automatic_cache_wins_when_both_modes_are_enabled() {
    let mut workflow = Workflow::new();
    let mut settings = settings("v3");
    settings.use_alfred_cache = true;
    settings.use_file_cache = true;

    configure_cache(&mut workflow, "request", &settings);

    assert_eq!(
        (workflow.use_automatic_cache(), workflow.cache_key()),
        (true, None)
    );
    assert_eq!(workflow.cache_time_to_live(), Some(86_400));
}

#[test]
fn file_cache_key_includes_normalized_query_and_version() {
    assert_eq!(file_cache_key("request body", "v3"), "request body_v3");
}

#[test]
fn empty_query_shows_placeholder_without_searching() -> Result<()> {
    let search_calls = Cell::new(0);
    let cli = Cli::default();
    let mut workflow = Workflow::new();

    populate_workflow_with(&mut workflow, &cli, &settings("v3"), |_, _| {
        search_calls.set(search_calls.get() + 1);
        Ok(Vec::new())
    })?;

    assert_eq!(
        (search_calls.get(), workflow.get_items()?.items()[0].title()),
        (0, "Search the Vue.js docs...")
    );
    Ok(())
}

#[test]
fn empty_query_does_not_enter_file_cache() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut settings = settings("v3");
    settings.use_file_cache = true;
    let search_calls = Cell::new(0);
    let cli = Cli::default();
    let mut first = Workflow::with_file_cache(FileCache::with_path(directory.path()));

    populate_workflow_with(&mut first, &cli, &settings, |_, _| {
        search_calls.set(search_calls.get() + 1);
        Ok(Vec::new())
    })?;

    assert_eq!(
        (
            search_calls.get(),
            first.cache_key(),
            first.get_items()?.len(),
            directory.path().read_dir()?.next().is_none(),
        ),
        (0, None, 1, true)
    );
    Ok(())
}

#[test]
fn runtime_error_does_not_enter_file_cache() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut settings = settings("v3");
    settings.use_file_cache = true;
    let cli = Cli {
        query: "request".to_owned(),
        ..Cli::default()
    };
    let mut first = Workflow::with_file_cache(FileCache::with_path(directory.path()));

    let error = populate_workflow_with(&mut first, &cli, &settings, |_, _| {
        Err(anyhow::anyhow!("transient search failure"))
    })
    .expect_err("the first search must fail");
    replace_items_with_runtime_error(&mut first, &error)?;

    assert!(first.cache_key().is_none());
    assert_eq!(
        first.get_items()?.items()[0].title(),
        "transient search failure"
    );

    let search_calls = Cell::new(0);
    let mut second = Workflow::with_file_cache(FileCache::with_path(directory.path()));
    populate_workflow_with(&mut second, &cli, &settings, |_, _| {
        search_calls.set(search_calls.get() + 1);
        Ok(Vec::new())
    })?;

    assert_eq!(search_calls.get(), 1);
    assert_eq!(
        second.get_items()?.items()[0].title(),
        "No matching answers found"
    );
    Ok(())
}

#[test]
fn runtime_error_does_not_emit_automatic_cache_metadata() -> Result<()> {
    let mut workflow = Workflow::new();
    workflow.set_use_automatic_cache(true);
    let error = anyhow::anyhow!("transient search failure");

    replace_items_with_runtime_error(&mut workflow, &error)?;

    let rendered: serde_json::Value = serde_json::from_str(&workflow.to_json_string()?)?;
    assert!(rendered.get("cache").is_none());
    Ok(())
}

#[test]
fn file_cache_hit_bypasses_algolia() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut cached = Workflow::with_file_cache(FileCache::with_path(directory.path()));
    cached.set_cache_key(Some("request_v3"));
    let cached_item = google_fallback_item("request")?;
    cached.add_item(cached_item.clone())?;

    let mut workflow = Workflow::with_file_cache(FileCache::with_path(directory.path()));
    let mut settings = settings("v3");
    settings.use_file_cache = true;
    let search_calls = Cell::new(0);
    let cli = Cli {
        query: "request".to_owned(),
        ..Cli::default()
    };

    populate_workflow_with(&mut workflow, &cli, &settings, |_, _| {
        search_calls.set(search_calls.get() + 1);
        Ok(Vec::new())
    })?;

    assert_eq!(search_calls.get(), 0);
    assert_eq!(workflow.get_items()?.items(), &[cached_item]);
    Ok(())
}

#[test]
fn file_cache_entries_are_version_specific() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut cached = Workflow::with_file_cache(FileCache::with_path(directory.path()));
    cached.set_cache_key(Some("request_v2"));
    cached.add_item(Item::new("v2 result"))?;

    let mut workflow = Workflow::with_file_cache(FileCache::with_path(directory.path()));
    let mut settings = settings("v3");
    settings.use_file_cache = true;
    let search_calls = Cell::new(0);
    let cli = Cli {
        query: "request".to_owned(),
        ..Cli::default()
    };

    populate_workflow_with(&mut workflow, &cli, &settings, |_, _| {
        search_calls.set(search_calls.get() + 1);
        Ok(Vec::new())
    })?;

    assert_eq!(search_calls.get(), 1);
    Ok(())
}

#[test]
fn update_item_is_rendered_without_entering_file_cache() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut workflow = Workflow::with_file_cache(FileCache::with_path(directory.path()));
    workflow.set_cache_key(Some("request_v3"));
    workflow.add_item(Item::new("search result"))?;
    let rendered: serde_json::Value = serde_json::from_str(&render_workflow_json(
        &workflow,
        update_available_with(&Cli::default(), || Ok(true)),
    )?)?;
    let cached = workflow.get_items()?;

    assert_eq!(
        (
            rendered["items"].as_array().map(Vec::len),
            rendered["items"][0]["title"].as_str(),
            cached.len(),
            cached.items()[0].title()
        ),
        (Some(2), Some("Auto-Update available!"), 1, "search result")
    );
    Ok(())
}

#[test]
fn rendered_items_keep_uids_and_skip_alfred_knowledge() -> Result<()> {
    let mut workflow = Workflow::new();
    workflow.set_disable_alfred_smart_result_ordering(true);
    workflow.add_item(
        Item::builder("result")
            .uid("algolia-object")
            .valid(true)
            .build()?,
    )?;

    let rendered: serde_json::Value =
        serde_json::from_str(&render_workflow_json(&workflow, false)?)?;

    assert_eq!(
        (
            rendered["skipknowledge"].as_bool(),
            rendered["items"][0]["uid"].as_str(),
        ),
        (Some(true), Some("algolia-object"))
    );
    Ok(())
}
