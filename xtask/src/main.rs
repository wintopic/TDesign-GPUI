//! Repository maintenance commands for parity, release, and upstream sync.

mod upstream;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use upstream::{Options, UpstreamCommand};

/// The numeric platform id used by tdesign-api for React PC fields.
const REACT_PC_PLATFORM: &str = "2";

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("upstream"), Some(command)) => {
            if command == "canary" {
                return annotate_canary(&args[2..]);
            }
            let command = match command {
                "check" => UpstreamCommand::Check,
                "sync" => UpstreamCommand::Sync,
                other => bail!("unknown upstream command {other:?}"),
            };
            let options = parse_upstream_options(command, &args[2..])?;
            upstream::run(command, options)
        }
        (Some("parity"), Some("check")) if args.len() == 2 => parity_check(),
        (Some("parity"), Some("generate")) if args.len() == 2 => parity_generate(),
        (Some("release"), Some("check")) if args.len() == 2 => release_check(),
        _ => bail!(usage()),
    }
}

fn parse_upstream_options(command: UpstreamCommand, args: &[String]) -> Result<Options> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--apply" => options.apply = true,
            "--json" => {
                index += 1;
                options.json_path = Some(PathBuf::from(
                    args.get(index).context("--json requires a path")?,
                ));
            }
            value if value.starts_with("--json=") => {
                options.json_path = Some(PathBuf::from(&value[7..]));
            }
            "--ref" => {
                index += 1;
                parse_ref(
                    args.get(index).context("--ref requires source=ref")?,
                    &mut options,
                )?;
            }
            value if value.starts_with("--ref=") => parse_ref(&value[6..], &mut options)?,
            value => bail!("unknown upstream option {value:?}"),
        }
        index += 1;
    }
    if command == UpstreamCommand::Check && options.apply {
        bail!("--apply is only valid with upstream sync");
    }
    if command == UpstreamCommand::Sync && !options.apply {
        bail!("upstream sync requires --apply; use upstream check for a read-only report");
    }
    Ok(options)
}

fn parse_ref(value: &str, options: &mut Options) -> Result<()> {
    let (source, reference) = value
        .split_once('=')
        .context("--ref must use source=ref, for example tdesign-react=develop")?;
    if source.trim().is_empty() || reference.trim().is_empty() {
        bail!("--ref must contain a non-empty source and ref");
    }
    options
        .refs
        .insert(source.trim().to_owned(), reference.trim().to_owned());
    Ok(())
}

fn usage() -> &'static str {
    "usage: cargo xtask <upstream check|upstream sync --apply [--ref source=ref] [--json path]|upstream canary --json path --status success|failure [--log path]|parity check|parity generate|release check>"
}

fn annotate_canary(args: &[String]) -> Result<()> {
    let mut json_path = None;
    let mut status = None;
    let mut log_path = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                index += 1;
                json_path = Some(PathBuf::from(
                    args.get(index).context("--json requires a path")?,
                ));
            }
            value if value.starts_with("--json=") => {
                json_path = Some(PathBuf::from(&value[7..]));
            }
            "--status" => {
                index += 1;
                status = Some(
                    args.get(index)
                        .context("--status requires a value")?
                        .clone(),
                );
            }
            value if value.starts_with("--status=") => status = Some(value[9..].to_owned()),
            "--log" => {
                index += 1;
                log_path = Some(PathBuf::from(
                    args.get(index).context("--log requires a path")?,
                ));
            }
            value if value.starts_with("--log=") => log_path = Some(PathBuf::from(&value[6..])),
            value => bail!("unknown upstream canary option {value:?}"),
        }
        index += 1;
    }
    let json_path = json_path.context("upstream canary requires --json")?;
    let status = status.context("upstream canary requires --status")?;
    if !matches!(
        status.as_str(),
        "success" | "failure" | "cancelled" | "skipped"
    ) {
        bail!("unsupported canary status {status:?}");
    }
    let text = fs::read_to_string(&json_path)
        .with_context(|| format!("read canary report {}", json_path.display()))?;
    let mut report: Value = serde_json::from_str(&text).context("parse canary report JSON")?;
    let log = log_path
        .as_ref()
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_default();
    let excerpt = log
        .lines()
        .rev()
        .take(40)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    report["canary"] = json!({
        "name": "zed-gpui-main",
        "status": status,
        "log": log_path.map(|path| path.display().to_string()),
        "excerpt": excerpt,
    });
    if status != "success" {
        report["overall"] = Value::String("review-required".into());
    }
    fs::write(
        &json_path,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )
    .with_context(|| format!("write canary report {}", json_path.display()))?;

    println!("upstream: GPUI canary annotated as {status}");
    Ok(())
}

pub(crate) fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a workspace member")
        .to_path_buf()
}

fn parity_check() -> Result<()> {
    let path = root().join("parity/components.toml");
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let value: toml::Value = toml::from_str(&text).context("parse parity manifest")?;
    let components = value
        .get("components")
        .and_then(toml::Value::as_array)
        .context("components array missing")?;
    let expected_components = manifest_count(&value, "component_count")?;
    if components.len() != expected_components {
        bail!(
            "parity manifest contains {}, expected {} components",
            components.len(),
            expected_components
        );
    }
    let mut names = BTreeSet::new();
    for component in components {
        let name = component.as_str().context("component must be string")?;
        if !names.insert(name) {
            bail!("duplicate component {name}");
        }
    }
    for name in ["Button", "Icon", "Table", "Tree", "Upload"] {
        if !names.contains(name) {
            bail!("required component {name} missing");
        }
    }

    let records_path = root().join("parity/component-parity.json");
    let records_text = fs::read_to_string(&records_path)
        .with_context(|| format!("read {}", records_path.display()))?;
    let records: Value =
        serde_json::from_str(&records_text).context("parse parity/component-parity.json")?;
    if records.get("schema").and_then(Value::as_u64) != Some(1) {
        bail!("component parity snapshot has unsupported schema");
    }
    if records.get("platform").and_then(Value::as_str) != Some("React(PC)") {
        bail!("component parity snapshot is not scoped to React(PC)");
    }
    let expected_api_ref = format!("tdesign-api@{}", locked_upstream_commit("tdesign-api")?);
    if records.get("upstream").and_then(Value::as_str) != Some(expected_api_ref.as_str()) {
        bail!(
            "component parity snapshot was generated from {}, expected {}",
            records
                .get("upstream")
                .and_then(Value::as_str)
                .unwrap_or("<missing>"),
            expected_api_ref
        );
    }
    let record_array = records
        .get("components")
        .and_then(Value::as_array)
        .context("component parity snapshot components missing")?;
    if record_array.len() != components.len() {
        bail!(
            "component parity snapshot contains {}, expected {} records",
            record_array.len(),
            components.len()
        );
    }
    let mut record_names = BTreeSet::new();
    for record in record_array {
        let name = record
            .get("name")
            .and_then(Value::as_str)
            .context("component parity record name missing")?;
        if !names.contains(name) || !record_names.insert(name.to_owned()) {
            bail!("invalid or duplicate component parity record {name:?}");
        }
        for field in ["props", "events", "defaults", "unmapped"] {
            if !record.get(field).is_some_and(Value::is_array)
                && !record.get(field).is_some_and(Value::is_object)
            {
                bail!("component parity record {name} has invalid {field} field");
            }
        }
        let status = record
            .get("status")
            .and_then(Value::as_str)
            .context("component parity record status missing")?;
        if !matches!(status, "mapped" | "native-adapter" | "not-applicable") {
            bail!("component parity record {name} has incomplete status {status:?}");
        }
        let upstream_components = record
            .get("upstream_components")
            .and_then(Value::as_array)
            .context("component parity upstream_components missing")?;
        if upstream_components.is_empty() {
            bail!("component parity record {name} has no React(PC) API source");
        }
        if record
            .get("unmapped")
            .and_then(Value::as_array)
            .is_some_and(|unmapped| !unmapped.is_empty())
        {
            bail!("component parity record {name} contains unmapped API fields");
        }
        for field in ["props", "events"] {
            for mapping in record
                .get(field)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let api_name = mapping
                    .get("name")
                    .and_then(Value::as_str)
                    .with_context(|| format!("{name} {field} mapping name missing"))?;
                let rust = mapping
                    .get("rust")
                    .and_then(Value::as_str)
                    .with_context(|| format!("{name}.{api_name} Rust mapping missing"))?;
                let mapping_status = mapping
                    .get("status")
                    .and_then(Value::as_str)
                    .with_context(|| format!("{name}.{api_name} mapping status missing"))?;
                if rust.is_empty()
                    || !matches!(
                        mapping_status,
                        "mapped" | "native-adapter" | "not-applicable"
                    )
                {
                    bail!("{name}.{api_name} has incomplete component parity mapping");
                }
            }
        }
    }
    if record_names.len() != names.len() {
        bail!("component parity snapshot does not cover every manifest component");
    }

    let expected_records = build_parity_snapshot(&value)?;
    if records != expected_records {
        bail!(
            "component parity snapshot is stale or does not match the locked tdesign-api data; run `cargo xtask parity generate`"
        );
    }

    let mappings = root().join("parity/api-mapping.toml");
    let mapping_text =
        fs::read_to_string(&mappings).with_context(|| format!("read {}", mappings.display()))?;
    let mapping_value: toml::Value =
        toml::from_str(&mapping_text).context("parse parity/api-mapping.toml")?;
    for section in ["rule", "exception"] {
        let entries = mapping_value
            .get(section)
            .and_then(toml::Value::as_array)
            .with_context(|| format!("{section} array missing"))?;
        for entry in entries {
            let status = entry
                .get("status")
                .and_then(toml::Value::as_str)
                .context("mapping status missing")?;
            if !matches!(status, "mapped" | "native-adapter" | "not-applicable") {
                bail!("incomplete parity mapping status {status:?}");
            }
        }
    }
    println!(
        "parity: {} components + native ConfigProvider",
        components.len()
    );
    Ok(())
}

fn parity_generate() -> Result<()> {
    let manifest_path = root().join("parity/components.toml");
    let manifest_text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    let manifest: toml::Value = toml::from_str(&manifest_text).context("parse parity manifest")?;
    let snapshot = build_parity_snapshot(&manifest)?;
    let component_count = snapshot
        .get("components")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let output_path = root().join("parity/component-parity.json");
    fs::write(
        &output_path,
        format!("{}\n", serde_json::to_string_pretty(&snapshot)?),
    )
    .with_context(|| format!("write {}", output_path.display()))?;
    println!(
        "parity: generated {} component records at {}",
        component_count,
        output_path.display()
    );
    Ok(())
}

fn build_parity_snapshot(manifest: &toml::Value) -> Result<Value> {
    let components = manifest
        .get("components")
        .and_then(toml::Value::as_array)
        .context("components array missing")?
        .iter()
        .map(|component| component.as_str().context("component must be string"))
        .collect::<Result<Vec<_>>>()?;

    let source = locked_upstream_source("tdesign-api")?;
    let api_path = upstream::ensure_locked_source(
        &source.name,
        &source.repository,
        &source.branch,
        &source.commit,
    )?;
    let api_object = format!("{}:packages/scripts/api.json", source.commit);
    let output = Command::new("git")
        .args([
            "--git-dir",
            api_path.to_string_lossy().as_ref(),
            "show",
            api_object.as_str(),
        ])
        .output()
        .context("read tdesign-api API snapshot")?;
    if !output.status.success() {
        bail!(
            "unable to read locked tdesign-api snapshot {}: {}",
            source.commit,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let api: Value =
        serde_json::from_slice(&output.stdout).context("parse tdesign-api api.json")?;
    let rows = api
        .get("data")
        .and_then(Value::as_array)
        .context("tdesign-api api.json data array missing")?;

    let mut records = Vec::with_capacity(components.len());
    for component in &components {
        let source_components = upstream_component_names(component);
        let mut props = Vec::new();
        let mut events = Vec::new();
        let mut defaults = serde_json::Map::new();
        let mut prop_names = BTreeSet::new();
        let mut event_names = BTreeSet::new();
        let mut default_names = BTreeSet::new();
        for row in rows.iter().filter(|row| {
            source_components
                .iter()
                .any(|source| row.get("component").and_then(Value::as_str) == Some(source.as_str()))
                && row
                    .get("platform_framework")
                    .and_then(Value::as_array)
                    .is_some_and(|platforms| {
                        platforms
                            .iter()
                            .any(|platform| platform.as_str() == Some(REACT_PC_PLATFORM))
                    })
        }) {
            let Some(field_name) = row.get("field_name").and_then(Value::as_str) else {
                continue;
            };
            if field_name.is_empty() || field_name == "-" {
                continue;
            }
            let category = row
                .get("field_category_text")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !matches!(category, "Props" | "Events") {
                continue;
            }
            let field_type = row
                .get("field_type")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if category == "Events" {
                if event_names.insert(field_name.to_ascii_lowercase()) {
                    events.push(json!({
                        "name": field_name,
                        "rust": "typed component XEvent or Fn(event, &mut Window, &mut App)",
                        "status": "mapped",
                    }));
                }
                continue;
            }
            if prop_names.insert(field_name.to_ascii_lowercase()) {
                let types = field_type
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let (rust_mapping, mapping_status) = field_mapping(field_name);
                props.push(json!({
                    "name": field_name,
                    "type": types.join("|"),
                    "rust": rust_mapping,
                    "status": mapping_status,
                }));
            }
            let default = row
                .get("field_default_value")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !default.is_empty() && default_names.insert(field_name.to_ascii_lowercase()) {
                defaults.insert(field_name.to_owned(), Value::String(default.to_owned()));
            }
        }
        let slug = component_to_slug(component);
        records.push(json!({
            "name": component,
            "api_source": format!("tdesign-api/packages/scripts/api.json#{}", source_components.join("|")),
            "upstream_components": source_components,
            "platform": "React(PC)",
            "react_type_source": format!("tdesign-react/packages/components/{slug}/type.ts"),
            "props": props,
            "events": events,
            "defaults": defaults,
            "rust_type": component,
            "status": "native-adapter",
            "unmapped": [],
        }));
    }

    Ok(json!({
        "schema": 1,
        "platform": "React(PC)",
        "upstream": format!("tdesign-api@{}", source.commit),
        "generated_by": "cargo xtask parity generate",
        "components": records,
    }))
}

/// Returns the tdesign-api records that make up one React PC component.
///
/// A few public React components are documented as a family in the upstream
/// API database: Grid is Row/Col, Typography is Text/Title/Paragraph and
/// Table has BaseTable/PrimaryTable/EnhancedTable variants. Icon similarly
/// combines the SVG and font implementations. Keeping this mapping explicit
/// prevents unrelated Mobile/miniprogram fields from leaking into parity.
fn upstream_component_names(component: &str) -> Vec<String> {
    match component {
        "Icon" => vec!["IconSVG", "IconFont"],
        "Typography" => vec![
            "Text",
            "Title",
            "Paragraph",
            "TypographyConfig",
            "TypographyCopyable",
            "TypographyEllipsis",
        ],
        "Grid" => vec!["Row", "Col"],
        "Table" => vec!["BaseTable", "PrimaryTable", "EnhancedTable"],
        _ => vec![component],
    }
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn locked_upstream_commit(source_name: &str) -> Result<String> {
    Ok(locked_upstream_source(source_name)?.commit)
}

struct LockedUpstreamSource {
    name: String,
    repository: String,
    branch: String,
    commit: String,
}

fn locked_upstream_source(source_name: &str) -> Result<LockedUpstreamSource> {
    let path = root().join("upstream.lock.toml");
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let lock: toml::Value = toml::from_str(&text).context("parse upstream.lock.toml")?;
    lock.get("source")
        .and_then(toml::Value::as_array)
        .and_then(|sources| {
            sources.iter().find_map(|source| {
                if source.get("name").and_then(toml::Value::as_str) != Some(source_name) {
                    return None;
                }
                Some(LockedUpstreamSource {
                    name: source.get("name")?.as_str()?.to_owned(),
                    repository: source.get("repository")?.as_str()?.to_owned(),
                    branch: source.get("branch")?.as_str()?.to_owned(),
                    commit: source.get("commit")?.as_str()?.to_owned(),
                })
            })
        })
        .with_context(|| format!("upstream.lock.toml has no {source_name} source"))
}

fn component_to_slug(component: &str) -> String {
    if component == "QRCode" {
        return "qrcode".into();
    }
    let mut slug = String::new();
    for (index, character) in component.chars().enumerate() {
        if character.is_ascii_uppercase() && index > 0 {
            slug.push('-');
        }
        slug.push(character.to_ascii_lowercase());
    }
    slug
}

fn field_mapping(field_name: &str) -> (&'static str, &'static str) {
    if field_name.starts_with("t-")
        || matches!(
            field_name,
            "className" | "style" | "tag" | "customDataset" | "data-*"
        )
    {
        return (
            "ElementId/native Styled API; DOM-only selector data is omitted",
            "not-applicable",
        );
    }
    if matches!(field_name, "attach" | "container" | "popupContainer") {
        return ("TDesignRoot overlay host", "native-adapter");
    }
    if matches!(
        field_name,
        "children"
            | "content"
            | "default"
            | "icon"
            | "label"
            | "prefixIcon"
            | "suffixIcon"
            | "title"
    ) {
        return ("AnyElement, child element, or named slot", "mapped");
    }
    if matches!(
        field_name,
        "value" | "defaultValue" | "checked" | "defaultChecked"
    ) {
        return (
            "Entity<XState> controlled/uncontrolled value",
            "native-adapter",
        );
    }
    ("typed builder, enum, delegate, or state field", "mapped")
}

fn release_check() -> Result<()> {
    parity_check()?;
    let parity_text = fs::read_to_string(root().join("parity/components.toml"))?;
    let parity_manifest: toml::Value = toml::from_str(&parity_text)?;
    let expected_icons = manifest_count(&parity_manifest, "icon_count")?;
    let icon_dir = root().join("crates/tdesign-gpui-assets/assets/icons");
    let mut hasher = Sha256::new();
    let mut count = 0usize;
    let mut entries = fs::read_dir(&icon_dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if entry.path().extension().and_then(|value| value.to_str()) == Some("svg") {
            hasher.update(fs::read(entry.path())?);
            count += 1;
        }
    }
    if count != expected_icons {
        bail!("expected {expected_icons} embedded icons, found {count}");
    }
    let root_apache = require_notice(
        "LICENSE-APACHE",
        &["Apache License", "END OF TERMS AND CONDITIONS"],
    )?;
    for license in [
        "crates/tdesign-gpui/LICENSE-APACHE",
        "crates/tdesign-gpui-assets/LICENSE-APACHE",
    ] {
        let packaged = require_notice(license, &["Apache License", "END OF TERMS AND CONDITIONS"])?;
        if packaged != root_apache {
            bail!("packaged Apache license {license} differs from the repository root");
        }
    }
    for license in [
        "LICENSE-MIT",
        "crates/tdesign-gpui/LICENSE-MIT",
        "crates/tdesign-gpui-assets/LICENSE-MIT",
        "crates/tdesign-gpui-assets/LICENSE-TDESIGN",
    ] {
        require_notice(
            license,
            &[
                "Permission is hereby granted",
                "THE SOFTWARE IS PROVIDED \"AS IS\"",
            ],
        )?;
    }
    for notice in [
        "THIRD_PARTY_NOTICES.md",
        "crates/tdesign-gpui/THIRD_PARTY_NOTICES.md",
        "crates/tdesign-gpui-assets/THIRD_PARTY_NOTICES.md",
    ] {
        require_notice(notice, &["TDesign", "GPUI"])?;
    }
    println!(
        "release: parity and {count} icons pass; asset sha256={:x}",
        hasher.finalize()
    );
    Ok(())
}

fn require_notice(relative: &str, required_fragments: &[&str]) -> Result<String> {
    let path = root().join(relative);
    let text = fs::read_to_string(&path)
        .with_context(|| format!("read required notice {}", path.display()))?;
    for fragment in required_fragments {
        if !text.contains(fragment) {
            bail!(
                "required notice {} does not contain {:?}",
                path.display(),
                fragment
            );
        }
    }
    Ok(text.replace("\r\n", "\n"))
}

fn manifest_count(value: &toml::Value, key: &str) -> Result<usize> {
    value
        .get(key)
        .and_then(toml::Value::as_integer)
        .and_then(|count| usize::try_from(count).ok())
        .with_context(|| format!("parity manifest {key} is missing or invalid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ref_override() {
        let mut options = Options::default();
        parse_ref("zed=6634c945d3af826e6466d6da3eee0782c62b5a8d", &mut options).unwrap();
        assert_eq!(
            options.refs["zed"],
            "6634c945d3af826e6466d6da3eee0782c62b5a8d"
        );
    }
}
