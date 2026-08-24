#![forbid(unsafe_code)]

mod cli;
mod config;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use alfred_vue_docs::app::{google_fallback_item, items_from_results, placeholder_item};
use alfred_vue_docs::models::SearchResult;
use alfred_vue_docs::services::AlgoliaSearch;
use alfred_workflow_rs::{Icon, Item, Items, Updater, UserConfiguration, Workflow};
use anyhow::{Result, anyhow};

use crate::cli::Cli;

const GITHUB_REPOSITORY_URL: &str = "https://github.com/techouse/alfred-vue-docs";
const UPDATE_INTERVAL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkflowSettings {
    vue_version: String,
    use_alfred_cache: bool,
    use_file_cache: bool,
    cache_ttl: Option<u64>,
    file_cache_max_entries: Option<usize>,
}

fn main() -> ExitCode {
    let mut workflow = Workflow::new();
    workflow.set_disable_alfred_smart_result_ordering(true);

    let cli = match Cli::parse(std::env::args().skip(1)) {
        Ok(cli) => cli,
        Err(error) => return render_error(&mut workflow, &error, 2, false),
    };

    if cli.update {
        return update_workflow();
    }

    let (update_available, exit_code) = match populate_workflow(&mut workflow, &cli) {
        Ok(()) => (update_available(&cli), ExitCode::SUCCESS),
        Err(error) => {
            if cli.verbose {
                eprintln!("{error:#}");
            }
            if let Err(add_error) = replace_items_with_runtime_error(&mut workflow, &error) {
                eprintln!("failed to render workflow error: {add_error}");
                return ExitCode::from(1);
            }
            (update_available(&cli), ExitCode::from(1))
        }
    };

    if let Err(error) = write_workflow_stdout(&workflow, update_available) {
        eprintln!("failed to write Script Filter JSON: {error}");
        return ExitCode::from(1);
    }

    exit_code
}

fn replace_items_with_runtime_error(
    workflow: &mut Workflow,
    error: &anyhow::Error,
) -> alfred_workflow_rs::Result<()> {
    if let Err(clear_error) = workflow.clear_items() {
        eprintln!("failed to clear workflow items: {clear_error}");
    }
    workflow.clear_cache_key();
    workflow.set_use_automatic_cache(false);
    workflow.add_item(Item::new(error.to_string()))
}

fn render_error(
    workflow: &mut Workflow,
    error: &anyhow::Error,
    exit_code: u8,
    verbose: bool,
) -> ExitCode {
    if verbose {
        eprintln!("{error:#}");
    }
    if let Err(add_error) = workflow.add_item(Item::new(error.to_string())) {
        eprintln!("failed to render workflow error: {add_error}");
        return ExitCode::from(1);
    }
    if let Err(write_error) = write_workflow_stdout(workflow, false) {
        eprintln!("failed to write Script Filter JSON: {write_error}");
        return ExitCode::from(1);
    }

    ExitCode::from(exit_code)
}

fn populate_workflow(workflow: &mut Workflow, cli: &Cli) -> Result<()> {
    let settings = read_workflow_settings(workflow, "info.plist", "prefs.plist")?;
    populate_workflow_with(workflow, cli, &settings, |query, version| {
        let search = AlgoliaSearch::new(config::algolia_search_config()?)?;
        search.query(query, version)
    })
}

fn populate_workflow_with<S>(
    workflow: &mut Workflow,
    cli: &Cli,
    settings: &WorkflowSettings,
    search: S,
) -> Result<()>
where
    S: FnOnce(&str, &str) -> Result<Vec<SearchResult>>,
{
    let query = cli.normalized_query(&settings.vue_version);
    if cli.verbose {
        eprintln!("Query: \"{query}\"");
    }

    configure_cache(workflow, &query, settings);
    if query.is_empty() {
        workflow.add_item(placeholder_item())?;
        return Ok(());
    }
    if !workflow.get_items()?.is_empty() {
        return Ok(());
    }

    let results = search(&query, &settings.vue_version)?;
    if results.is_empty() {
        workflow.add_item(google_fallback_item(&query)?)?;
    } else {
        workflow.add_items(items_from_results(&results)?)?;
    }

    Ok(())
}

fn configure_cache(workflow: &mut Workflow, query: &str, settings: &WorkflowSettings) {
    if settings.use_alfred_cache {
        workflow.set_use_automatic_cache(true);
    } else if settings.use_file_cache && !query.is_empty() {
        workflow.set_cache_key(Some(file_cache_key(query, &settings.vue_version)));
        workflow.set_max_cache_entries(settings.file_cache_max_entries);
    }
    workflow.set_cache_time_to_live(settings.cache_ttl);
}

fn file_cache_key(query: &str, vue_version: &str) -> String {
    format!("{query}_{vue_version}")
}

fn read_workflow_settings(
    workflow: &Workflow,
    info_path: impl AsRef<Path>,
    prefs_path: impl AsRef<Path>,
) -> Result<WorkflowSettings> {
    let defaults = workflow.get_user_defaults(info_path, prefs_path)?;
    workflow_settings_from_defaults(&defaults)
}

fn workflow_settings_from_defaults(
    defaults: &BTreeMap<String, UserConfiguration>,
) -> Result<WorkflowSettings> {
    let vue_version =
        select_value(defaults, "vue_version").ok_or_else(|| anyhow!("vue_version not set!"))?;

    Ok(WorkflowSettings {
        vue_version: vue_version.to_owned(),
        use_alfred_cache: checkbox_value(defaults, "use_alfred_cache").unwrap_or(false),
        use_file_cache: checkbox_value(defaults, "use_file_cache").unwrap_or(false),
        cache_ttl: slider_value(defaults, "cache_ttl").and_then(|value| u64::try_from(value).ok()),
        file_cache_max_entries: slider_value(defaults, "file_cache_max_entries")
            .and_then(|value| usize::try_from(value).ok()),
    })
}

fn select_value<'a>(
    defaults: &'a BTreeMap<String, UserConfiguration>,
    variable: &str,
) -> Option<&'a str> {
    match defaults.get(variable) {
        Some(UserConfiguration::Select(configuration)) => Some(&configuration.config.value),
        _ => None,
    }
}

fn checkbox_value(defaults: &BTreeMap<String, UserConfiguration>, variable: &str) -> Option<bool> {
    match defaults.get(variable) {
        Some(UserConfiguration::CheckBox(configuration)) => Some(configuration.config.value),
        _ => None,
    }
}

fn slider_value(defaults: &BTreeMap<String, UserConfiguration>, variable: &str) -> Option<i64> {
    match defaults.get(variable) {
        Some(UserConfiguration::NumberSlider(configuration)) => Some(configuration.config.value),
        _ => None,
    }
}

fn update_available(cli: &Cli) -> bool {
    update_available_with(cli, || {
        updater()?.update_available().map_err(anyhow::Error::from)
    })
}

fn update_available_with<F>(cli: &Cli, check: F) -> bool
where
    F: FnOnce() -> Result<bool>,
{
    match check() {
        Ok(value) => value,
        Err(error) => {
            if cli.verbose {
                eprintln!("could not check for updates: {error}");
            }
            false
        }
    }
}

fn write_workflow_stdout(workflow: &Workflow, update_available: bool) -> Result<()> {
    let json = render_workflow_json(workflow, update_available)?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(json.as_bytes())?;
    Ok(())
}

fn render_workflow_json(workflow: &Workflow, update_available: bool) -> Result<String> {
    let mut items: Items = workflow.get_items()?;
    // alfred_workflow_rs omits UIDs when exact-order serialization is enabled. Keep the
    // workflow's disabled smart-ordering setting, but serialize this deterministic item
    // sequence with UIDs so Alfred receives the documented result identity as well.
    items.set_exact_order(false);
    items.set_skip_knowledge(workflow.skip_knowledge());
    items.set_cache(workflow.automatic_cache());
    if update_available {
        items.insert(0, update_item());
    }
    Ok(serde_json::to_string(&items)?)
}

fn update_workflow() -> ExitCode {
    println!("Updating workflow...");
    match updater().and_then(|updater| updater.update().map_err(Into::into)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn updater() -> Result<Updater> {
    Ok(
        Updater::builder(GITHUB_REPOSITORY_URL.parse()?, env!("CARGO_PKG_VERSION"))?
            .update_interval(UPDATE_INTERVAL)
            .build()?,
    )
}

fn update_item() -> Item {
    Item::with_arg("Auto-Update available!", "update:workflow")
        .set_subtitle("Press <enter> to auto-update to a new version of this workflow.")
        .set_match_text(
            "Auto-Update available! Press <enter> to auto-update to a new version of this workflow.",
        )
        .set_icon(Icon::new("alfredhatcog.png"))
        .set_valid(true)
}

#[cfg(test)]
#[path = "tests/main.rs"]
mod tests;
