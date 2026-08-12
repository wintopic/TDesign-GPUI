use crate::root;
use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const REPORT_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpstreamCommand {
    Check,
    Sync,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Options {
    pub(crate) apply: bool,
    pub(crate) refs: BTreeMap<String, String>,
    pub(crate) json_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpstreamLock {
    schema: u32,
    updated_at: String,
    source: Vec<Source>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Source {
    name: String,
    repository: String,
    branch: String,
    commit: String,
}

#[derive(Debug, Clone, Serialize)]
struct UpstreamReport {
    schema: u32,
    generated_at: String,
    command: String,
    apply_requested: bool,
    overall: OverallStatus,
    summary: ReportSummary,
    sources: Vec<SourceReport>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum OverallStatus {
    Current,
    SyncSafe,
    ReviewRequired,
}

#[derive(Debug, Clone, Default, Serialize)]
struct ReportSummary {
    sources_checked: usize,
    sources_changed: usize,
    files_changed: usize,
    sync_safe_files: usize,
    review_required_files: usize,
    ignored_files: usize,
    applied_sources: usize,
    token_changes: usize,
    local_icon_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct SourceReport {
    name: String,
    repository: String,
    branch: String,
    locked_commit: String,
    target_ref: String,
    target_commit: String,
    status: SourceStatus,
    changes: Vec<FileChangeReport>,
    token_changes: Vec<TokenChange>,
    applied: bool,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum SourceStatus {
    Current,
    SyncSafe,
    ReviewRequired,
}

#[derive(Debug, Clone, Serialize)]
struct FileChangeReport {
    kind: ChangeKind,
    path: String,
    old_path: Option<String>,
    disposition: Disposition,
    category: String,
    reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Disposition {
    SyncSafe,
    ReviewRequired,
    Ignored,
}

#[derive(Debug, Clone, Serialize)]
struct TokenChange {
    name: String,
    mode: String,
    old_value: Option<String>,
    new_value: Option<String>,
    kind: TokenChangeKind,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum TokenChangeKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone)]
struct RawChange {
    kind: ChangeKind,
    old_path: Option<String>,
    path: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ThemeToken {
    name: String,
    value: String,
    mode: String,
}

pub(crate) fn run(command: UpstreamCommand, options: Options) -> Result<()> {
    let mut lock = read_lock()?;
    validate_ref_overrides(&lock, &options)?;
    let cache_root = root().join("upstream-cache");
    fs::create_dir_all(&cache_root).with_context(|| format!("create {}", cache_root.display()))?;

    if options.apply {
        ensure_generated_baselines(&lock, &cache_root)?;
    }

    let mut report = UpstreamReport {
        schema: REPORT_SCHEMA,
        generated_at: Utc::now().to_rfc3339(),
        command: match command {
            UpstreamCommand::Check => "check".to_owned(),
            UpstreamCommand::Sync => "sync".to_owned(),
        },
        apply_requested: options.apply,
        overall: OverallStatus::Current,
        summary: ReportSummary::default(),
        sources: Vec::new(),
    };

    for source in lock.source.clone() {
        validate_commit(&source.name, &source.commit)?;
        let target_ref = options
            .refs
            .get(&source.name)
            .cloned()
            .unwrap_or_else(|| source.branch.clone());
        let target_commit = resolve_ref(&source, &target_ref)?;
        let mut source_report = inspect_source(&source, &target_ref, &target_commit, &cache_root)?;
        update_summary(&mut report.summary, &source_report);

        if options.apply && source_report.status == SourceStatus::SyncSafe {
            apply_source(&source, &target_commit, &source_report, &cache_root)?;
            let locked = lock
                .source
                .iter_mut()
                .find(|item| item.name == source.name)
                .context("source disappeared from lock")?;
            locked.commit.clone_from(&target_commit);
            source_report.applied = true;
            report.summary.applied_sources += 1;
        }
        report.sources.push(source_report);
    }

    report.summary.local_icon_count = count_local_icons()?;
    report.overall = if report
        .sources
        .iter()
        .any(|source| source.status == SourceStatus::ReviewRequired)
    {
        OverallStatus::ReviewRequired
    } else if report
        .sources
        .iter()
        .any(|source| source.status == SourceStatus::SyncSafe)
    {
        OverallStatus::SyncSafe
    } else {
        OverallStatus::Current
    };

    if options.apply && report.summary.applied_sources > 0 {
        lock.updated_at = Utc::now().to_rfc3339();
        write_lock(&lock)?;
    }

    let report_path = options
        .json_path
        .clone()
        .unwrap_or_else(|| root().join("artifacts/upstream-report.json"));
    write_json_report(&report, &report_path)?;
    if options.apply && report.overall == OverallStatus::ReviewRequired {
        write_review_report(&report)?;
    }
    print_report(&report, &report_path);
    Ok(())
}

fn read_lock() -> Result<UpstreamLock> {
    let path = root().join("upstream.lock.toml");
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let lock: UpstreamLock = toml::from_str(&text).context("parse upstream.lock.toml")?;
    if lock.schema != 1 {
        bail!("unsupported upstream lock schema {}", lock.schema);
    }
    if lock.source.is_empty() {
        bail!("upstream.lock.toml has no sources");
    }
    Ok(lock)
}

fn write_lock(lock: &UpstreamLock) -> Result<()> {
    let path = root().join("upstream.lock.toml");
    let text = toml::to_string_pretty(lock).context("serialize upstream.lock.toml")?;
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))
}

fn validate_ref_overrides(lock: &UpstreamLock, options: &Options) -> Result<()> {
    let known = lock
        .source
        .iter()
        .map(|source| source.name.as_str())
        .collect::<BTreeSet<_>>();
    for name in options.refs.keys() {
        if !known.contains(name.as_str()) {
            bail!("unknown upstream source {name:?}");
        }
    }
    Ok(())
}

fn validate_commit(source: &str, commit: &str) -> Result<()> {
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("{source} has invalid locked commit {commit:?}");
    }
    Ok(())
}

fn resolve_ref(source: &Source, reference: &str) -> Result<String> {
    if reference.len() == 40 && reference.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(reference.to_owned());
    }
    let output = match git_output(None, ["ls-remote", source.repository.as_str(), reference]) {
        Ok(output) => output,
        Err(network_error) => {
            // A checked-in cache makes `cargo xtask upstream check` useful in
            // offline CI and on developer machines with transient TLS/DNS
            // failures. We still prefer the live remote whenever it works.
            if let Some(commit) = cached_ref(source, reference)? {
                return Ok(commit);
            }
            return Err(network_error).with_context(|| {
                format!(
                    "resolve {reference:?} for {} (no usable cached ref)",
                    source.name
                )
            });
        }
    };
    let lines = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let commit = lines
        .iter()
        .find(|line| line.ends_with("^{}"))
        .or_else(|| lines.first())
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or_default();
    validate_commit(&source.name, commit).with_context(|| {
        format!(
            "unable to resolve ref {reference:?} from {}",
            source.repository
        )
    })?;
    Ok(commit.to_owned())
}

fn cache_path_for_source(source: &Source) -> PathBuf {
    let safe_name = source
        .name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    root()
        .join("upstream-cache")
        .join(format!("{safe_name}.git"))
}

fn cached_ref(source: &Source, reference: &str) -> Result<Option<String>> {
    let repository = cache_path_for_source(source);
    if !repository.exists() {
        return Ok(None);
    }
    let ref_name = format!("refs/remotes/origin/{reference}");
    let output = git_output(
        Some(&repository),
        ["rev-parse", "--verify", ref_name.as_str()],
    );
    let Ok(output) = output else {
        return Ok(None);
    };
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(Some(commit))
    } else {
        Ok(None)
    }
}

fn inspect_source(
    source: &Source,
    target_ref: &str,
    target_commit: &str,
    cache_root: &Path,
) -> Result<SourceReport> {
    let mut report = SourceReport {
        name: source.name.clone(),
        repository: source.repository.clone(),
        branch: source.branch.clone(),
        locked_commit: source.commit.clone(),
        target_ref: target_ref.to_owned(),
        target_commit: target_commit.to_owned(),
        status: SourceStatus::Current,
        changes: Vec::new(),
        token_changes: Vec::new(),
        applied: false,
        notes: Vec::new(),
    };
    if source.commit == target_commit {
        report
            .notes
            .push("locked commit already matches the requested ref".to_owned());
        return Ok(report);
    }

    let repository = ensure_cache(source, cache_root)?;
    fetch_history(&repository, source, target_ref, target_commit)?;
    ensure_object(&repository, &source.commit, source)?;
    ensure_object(&repository, target_commit, source)?;
    let changes = diff_changes(&repository, &source.commit, target_commit)?;
    let (classified, token_changes) =
        classify_changes(source, &repository, &changes, &source.commit, target_commit)?;
    report.changes = classified;
    report.token_changes = token_changes;
    if report
        .changes
        .iter()
        .any(|change| change.disposition == Disposition::ReviewRequired)
    {
        report.status = SourceStatus::ReviewRequired;
        report
            .notes
            .push("one or more consumed upstream changes require review".to_owned());
    } else {
        report.status = SourceStatus::SyncSafe;
        report
            .notes
            .push("all consumed changes are safe generated updates or documentation".to_owned());
    }
    Ok(report)
}

fn ensure_cache(source: &Source, cache_root: &Path) -> Result<PathBuf> {
    let safe_name = source
        .name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let path = cache_root.join(format!("{safe_name}.git"));
    if !path.exists() {
        fs::create_dir_all(&path)
            .with_context(|| format!("create upstream cache {}", path.display()))?;
        git_output(None, ["init", "--bare", path.to_string_lossy().as_ref()])?;
        git_output(
            Some(&path),
            ["remote", "add", "origin", source.repository.as_str()],
        )?;
    } else {
        let output = git_output(Some(&path), ["remote", "get-url", "origin"])?;
        let current = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if current != source.repository {
            git_output(
                Some(&path),
                ["remote", "set-url", "origin", source.repository.as_str()],
            )?;
        }
    }
    Ok(path)
}

fn fetch_history(
    repository: &Path,
    source: &Source,
    target_ref: &str,
    target_commit: &str,
) -> Result<()> {
    fetch_branch(repository, &source.branch)?;
    if target_ref != source.branch
        && !(target_ref.len() == 40 && target_ref.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        let _ = fetch_branch(repository, target_ref);
    }
    for commit in [&source.commit, target_commit] {
        if !object_exists(repository, commit)? {
            let output = Command::new("git")
                .current_dir(repository)
                .args([
                    "fetch",
                    "--force",
                    "--filter=blob:none",
                    "--no-tags",
                    "origin",
                    commit,
                ])
                .output()
                .with_context(|| format!("fetch {commit} for {}", source.name))?;
            if !output.status.success() {
                bail!(
                    "{}: unable to fetch commit {commit}: {}",
                    source.name,
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
        }
    }
    Ok(())
}

fn fetch_branch(repository: &Path, branch: &str) -> Result<()> {
    let refspec = format!("+refs/heads/{branch}:refs/remotes/origin/{branch}");
    match git_output(
        Some(repository),
        [
            "fetch",
            "--force",
            "--filter=blob:none",
            "--no-tags",
            "origin",
            refspec.as_str(),
        ],
    ) {
        Ok(_) => Ok(()),
        Err(error) => {
            let cached = format!("refs/remotes/origin/{branch}");
            if git_output(Some(repository), ["show-ref", "--verify", cached.as_str()]).is_ok() {
                Ok(())
            } else {
                Err(error).with_context(|| format!("fetch upstream branch {branch}"))
            }
        }
    }
}

fn ensure_object(repository: &Path, commit: &str, source: &Source) -> Result<()> {
    if !object_exists(repository, commit)? {
        bail!(
            "{}: commit {commit} is unavailable after fetch",
            source.name
        );
    }
    Ok(())
}

fn object_exists(repository: &Path, commit: &str) -> Result<bool> {
    let status = Command::new("git")
        .current_dir(repository)
        .args(["cat-file", "-e", &format!("{commit}^{{commit}}")])
        .status()
        .context("execute git cat-file")?;
    Ok(status.success())
}

fn diff_changes(repository: &Path, old: &str, new: &str) -> Result<Vec<RawChange>> {
    let output = git_output(
        Some(repository),
        [
            "diff",
            "--name-status",
            "--find-renames=50%",
            old,
            new,
            "--",
        ],
    )?;
    let mut changes = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let fields = line.split('\t').collect::<Vec<_>>();
        let status = fields
            .first()
            .and_then(|value| value.chars().next())
            .unwrap_or('?');
        let (kind, old_path, path) = match status {
            'A' => (
                ChangeKind::Added,
                None,
                fields.get(1).copied().unwrap_or_default(),
            ),
            'M' => (
                ChangeKind::Modified,
                None,
                fields.get(1).copied().unwrap_or_default(),
            ),
            'D' => (
                ChangeKind::Deleted,
                None,
                fields.get(1).copied().unwrap_or_default(),
            ),
            'R' => (
                ChangeKind::Renamed,
                fields.get(1).map(|value| (*value).to_owned()),
                fields.get(2).copied().unwrap_or_default(),
            ),
            'C' => (
                ChangeKind::Copied,
                fields.get(1).map(|value| (*value).to_owned()),
                fields.get(2).copied().unwrap_or_default(),
            ),
            'T' => (
                ChangeKind::TypeChanged,
                None,
                fields.get(1).copied().unwrap_or_default(),
            ),
            _ => (
                ChangeKind::Unknown,
                None,
                fields.get(1).copied().unwrap_or_default(),
            ),
        };
        if !path.is_empty() {
            changes.push(RawChange {
                kind,
                old_path,
                path: path.to_owned(),
            });
        }
    }
    Ok(changes)
}

fn classify_changes(
    source: &Source,
    repository: &Path,
    changes: &[RawChange],
    old_commit: &str,
    target_commit: &str,
) -> Result<(Vec<FileChangeReport>, Vec<TokenChange>)> {
    let mut reports = Vec::new();
    let mut all_token_changes = Vec::new();
    for change in changes {
        let (mut disposition, mut category, mut reason) =
            classify_file(source.name.as_str(), change);

        if source.name == "tdesign-common" && is_theme_source(&change.path) {
            if matches!(change.kind, ChangeKind::Deleted | ChangeKind::Renamed) {
                disposition = Disposition::ReviewRequired;
                category = "theme-token-removal".to_owned();
                reason = "theme token file deletion or rename can break Rust overrides".to_owned();
            } else {
                match token_diff(
                    repository,
                    old_commit,
                    target_commit,
                    &change.path,
                    change.kind,
                ) {
                    Ok(token_changes) if token_changes.is_empty() => {
                        if path_has_theme_declarations(
                            repository,
                            old_commit,
                            target_commit,
                            &change.path,
                        )? {
                            disposition = Disposition::Ignored;
                            category = "theme-token-format".to_owned();
                            reason = "theme file changed without semantic token changes".to_owned();
                        } else {
                            disposition = Disposition::ReviewRequired;
                            category = "theme-import".to_owned();
                            reason = "theme file has no parseable token declarations".to_owned();
                        }
                    }
                    Ok(token_changes) => {
                        if token_changes
                            .iter()
                            .any(|token| token.kind == TokenChangeKind::Deleted)
                        {
                            disposition = Disposition::ReviewRequired;
                            category = "theme-token-removal".to_owned();
                            reason = "one or more public theme tokens were removed".to_owned();
                        }
                        all_token_changes.extend(token_changes);
                    }
                    Err(error) => {
                        disposition = Disposition::ReviewRequired;
                        category = "theme-token-parse".to_owned();
                        reason = format!("unable to parse theme token change: {error:#}");
                    }
                }
            }
        }

        if source.name == "tdesign-icons"
            && matches!(change.kind, ChangeKind::Added | ChangeKind::Modified)
            && change.path.starts_with("svg/")
            && change.path.ends_with(".svg")
        {
            match git_show(repository, &format!("{target_commit}:{}", change.path))
                .and_then(|source| validate_and_sanitize_svg(&source))
            {
                Ok(_) => {}
                Err(error) => {
                    disposition = Disposition::ReviewRequired;
                    category = "icon-sanitization".to_owned();
                    reason = format!("SVG failed sanitization: {error:#}");
                }
            }
        }

        reports.push(FileChangeReport {
            kind: change.kind,
            path: change.path.clone(),
            old_path: change.old_path.clone(),
            disposition,
            category,
            reason,
        });
    }
    Ok((reports, all_token_changes))
}

fn classify_file(source: &str, change: &RawChange) -> (Disposition, String, String) {
    let path = change.path.as_str();
    if is_documentation(path) {
        return (
            Disposition::Ignored,
            "documentation".to_owned(),
            "documentation is tracked but has no generated Rust side effect".to_owned(),
        );
    }
    let result = match source {
        "tdesign-common" => {
            if is_theme_source(path)
                && !matches!(change.kind, ChangeKind::Deleted | ChangeKind::Renamed)
            {
                (
                    Disposition::SyncSafe,
                    "theme-token",
                    "parseable theme token addition or replacement",
                )
            } else if path.starts_with("style/") {
                (
                    Disposition::ReviewRequired,
                    "component-style",
                    "component styling changed and may alter visual parity",
                )
            } else {
                (
                    Disposition::ReviewRequired,
                    "upstream-source",
                    "unmapped common source changed",
                )
            }
        }
        "tdesign-icons" => {
            if path.starts_with("svg/") && path.ends_with(".svg") {
                if matches!(change.kind, ChangeKind::Added | ChangeKind::Modified) {
                    (
                        Disposition::SyncSafe,
                        "icon",
                        "SVG addition or content replacement",
                    )
                } else {
                    (
                        Disposition::ReviewRequired,
                        "icon-removal",
                        "icon deletion or rename is a breaking asset change",
                    )
                }
            } else {
                (
                    Disposition::Ignored,
                    "unconsumed-upstream",
                    "file is outside the consumed SVG tree",
                )
            }
        }
        "tdesign-api" => {
            if path == "packages/scripts/api.json" {
                (
                    Disposition::ReviewRequired,
                    "api",
                    "component props, events, or defaults changed",
                )
            } else {
                (
                    Disposition::Ignored,
                    "unconsumed-upstream",
                    "file is not the consumed API snapshot",
                )
            }
        }
        "tdesign-react" => (
            Disposition::ReviewRequired,
            "behavior-api",
            "React implementation, tests, or examples changed",
        ),
        "zed" => {
            if is_gpui_path(path) {
                (
                    Disposition::ReviewRequired,
                    "gpui",
                    "GPUI source or toolchain changed; canary validation is required",
                )
            } else {
                (
                    Disposition::Ignored,
                    "unconsumed-upstream",
                    "Zed file is outside the consumed GPUI crates",
                )
            }
        }
        _ => (
            Disposition::ReviewRequired,
            "unknown-source",
            "source is not covered by sync rules",
        ),
    };
    (result.0, result.1.to_owned(), result.2.to_owned())
}

fn is_documentation(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("docs/")
        || lower.starts_with("doc/")
        || lower.ends_with(".md")
        || lower.ends_with(".mdx")
        || lower.ends_with(".txt")
        || lower.contains("/changelog")
        || lower.starts_with("changelog")
}

fn is_theme_source(path: &str) -> bool {
    path == "style/web/_variables.less"
        || (path.starts_with("style/web/theme/") && path.ends_with(".less"))
}

fn is_gpui_path(path: &str) -> bool {
    path.starts_with("crates/gpui/")
        || path.starts_with("crates/gpui_macros/")
        || path.starts_with("crates/gpui_platform/")
        || path == "Cargo.toml"
        || path == "rust-toolchain.toml"
}

fn token_diff(
    repository: &Path,
    old_commit: &str,
    target_commit: &str,
    path: &str,
    kind: ChangeKind,
) -> Result<Vec<TokenChange>> {
    let old_tokens = if kind == ChangeKind::Added {
        BTreeMap::new()
    } else {
        let source = git_show(repository, &format!("{old_commit}:{path}"))?;
        parse_theme_tokens(path, &source)
    };
    let new_tokens = if kind == ChangeKind::Deleted {
        BTreeMap::new()
    } else {
        let source = git_show(repository, &format!("{target_commit}:{path}"))?;
        parse_theme_tokens(path, &source)
    };
    let names = old_tokens
        .keys()
        .chain(new_tokens.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut changes = Vec::new();
    for key in names {
        let old = old_tokens.get(&key);
        let new = new_tokens.get(&key);
        if old == new {
            continue;
        }
        let exemplar = new.or(old).context("token key disappeared")?;
        let kind = match (old, new) {
            (None, Some(_)) => TokenChangeKind::Added,
            (Some(_), None) => TokenChangeKind::Deleted,
            (Some(_), Some(_)) => TokenChangeKind::Modified,
            (None, None) => continue,
        };
        changes.push(TokenChange {
            name: exemplar.name.clone(),
            mode: exemplar.mode.clone(),
            old_value: old.map(|token| token.value.clone()),
            new_value: new.map(|token| token.value.clone()),
            kind,
        });
    }
    Ok(changes)
}

fn path_has_theme_declarations(
    repository: &Path,
    old_commit: &str,
    target_commit: &str,
    path: &str,
) -> Result<bool> {
    for commit in [old_commit, target_commit] {
        if let Ok(source) = git_show(repository, &format!("{commit}:{path}")) {
            if !parse_theme_tokens(path, &source).is_empty() {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn parse_theme_tokens(path: &str, content: &str) -> BTreeMap<String, ThemeToken> {
    let mode = if path.ends_with("_dark.less") {
        "dark"
    } else if path.ends_with("_light.less") {
        "light"
    } else if path == "style/web/_variables.less" {
        "alias"
    } else {
        "common"
    };
    let mut tokens = BTreeMap::new();
    for raw_line in content.lines() {
        let line = raw_line.split("//").next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        let parsed = if let Some(rest) = line.strip_prefix('@') {
            rest.split_once(':')
                .map(|(name, value)| (format!("@{}", name.trim()), value))
        } else if let Some(rest) = line.strip_prefix("--td-") {
            rest.split_once(':')
                .map(|(name, value)| (format!("--td-{}", name.trim()), value))
        } else {
            None
        };
        let Some((name, value)) = parsed else {
            continue;
        };
        let value = value.trim().trim_end_matches(';').trim();
        if value.is_empty() {
            continue;
        }
        let token = ThemeToken {
            name: name.clone(),
            value: value.to_owned(),
            mode: mode.to_owned(),
        };
        tokens.insert(format!("{mode}:{name}"), token);
    }
    tokens
}

fn ensure_generated_baselines(lock: &UpstreamLock, cache_root: &Path) -> Result<()> {
    let token_path = root().join("crates/tdesign-gpui/src/generated_theme_tokens.rs");
    if token_path.exists() {
        return Ok(());
    }
    let source = lock
        .source
        .iter()
        .find(|source| source.name == "tdesign-common")
        .context("tdesign-common source missing from lock")?;
    let repository = ensure_cache(source, cache_root)?;
    fetch_history(&repository, source, &source.branch, &source.commit)?;
    apply_theme_tokens(&repository, &source.commit)?;
    Ok(())
}

fn apply_source(
    source: &Source,
    target_commit: &str,
    report: &SourceReport,
    cache_root: &Path,
) -> Result<()> {
    let repository = ensure_cache(source, cache_root)?;
    match source.name.as_str() {
        "tdesign-icons" => apply_icons(&repository, target_commit, report),
        "tdesign-common" => apply_theme_tokens(&repository, target_commit),
        _ => Ok(()),
    }
}

fn apply_icons(repository: &Path, target_commit: &str, report: &SourceReport) -> Result<()> {
    for change in &report.changes {
        if change.disposition != Disposition::SyncSafe
            || !change.path.starts_with("svg/")
            || !change.path.ends_with(".svg")
        {
            continue;
        }
        let relative = change.path.strip_prefix("svg/").unwrap_or(&change.path);
        let destination = root()
            .join("crates/tdesign-gpui-assets/assets/icons")
            .join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let source = git_show(repository, &format!("{target_commit}:{}", change.path))?;
        let sanitized = validate_and_sanitize_svg(&source)?;
        fs::write(&destination, sanitized)
            .with_context(|| format!("write {}", destination.display()))?;
    }
    Ok(())
}

fn validate_and_sanitize_svg(source: &str) -> Result<String> {
    let mut normalized = source.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let lowered = normalized.to_ascii_lowercase();
    for forbidden in [
        "<script",
        "javascript:",
        "<foreignobject",
        "xlink:href=\"http",
        "href=\"http",
        "xlink:href='http",
        "href='http",
    ] {
        if lowered.contains(forbidden) {
            bail!("unsafe SVG construct {forbidden:?}");
        }
    }
    if !lowered.contains("<svg") {
        bail!("asset does not contain an <svg> root");
    }
    if normalized.starts_with("<?xml") {
        if let Some(end) = normalized.find("?>") {
            normalized = normalized[(end + 2)..].trim_start().to_owned();
        }
    }
    while let Some(start) = normalized.find("<!--") {
        let Some(relative_end) = normalized[(start + 4)..].find("-->") else {
            bail!("unterminated SVG comment");
        };
        let end = start + 4 + relative_end + 3;
        normalized.replace_range(start..end, "");
    }
    normalized = normalized.trim().to_owned();
    normalized.push('\n');
    Ok(normalized)
}

fn apply_theme_tokens(repository: &Path, target_commit: &str) -> Result<()> {
    let mut paths = vec!["style/web/_variables.less".to_owned()];
    let output = git_output(
        Some(repository),
        [
            "ls-tree",
            "-r",
            "--name-only",
            target_commit,
            "--",
            "style/web/theme",
        ],
    )?;
    paths.extend(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|path| path.ends_with(".less"))
            .map(str::to_owned),
    );
    paths.sort();
    paths.dedup();

    let mut tokens = BTreeMap::<String, ThemeToken>::new();
    for path in paths {
        let source = git_show(repository, &format!("{target_commit}:{path}"))?;
        tokens.extend(parse_theme_tokens(&path, &source));
    }
    if tokens.is_empty() {
        bail!("no parseable theme tokens found in tdesign-common");
    }

    let destination = root().join("crates/tdesign-gpui/src/generated_theme_tokens.rs");
    let mut output = String::from("// @generated by cargo xtask upstream sync; do not edit.\n");
    output.push_str("#[allow(missing_docs)]\n");
    output.push_str("#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n");
    output.push_str(
        "pub struct UpstreamThemeToken {\n    pub name: &'static str,\n    pub value: &'static str,\n    pub mode: &'static str,\n}\n\n",
    );
    output.push_str("#[allow(missing_docs)]\n");
    output.push_str("pub static TDESIGN_WEB_TOKENS: &[UpstreamThemeToken] = &[\n");
    for token in tokens.values() {
        output.push_str("    UpstreamThemeToken { name: ");
        output.push_str(&format!("{:?}", token.name));
        output.push_str(", value: ");
        output.push_str(&format!("{:?}", token.value));
        output.push_str(", mode: ");
        output.push_str(&format!("{:?}", token.mode));
        output.push_str(" },\n");
    }
    output.push_str("];\n");
    fs::write(&destination, output).with_context(|| format!("write {}", destination.display()))?;
    Ok(())
}

fn git_show(repository: &Path, object: &str) -> Result<String> {
    let output = git_output(Some(repository), ["show", object])?;
    String::from_utf8(output.stdout).context("git show returned non-UTF-8 data")
}

fn update_summary(summary: &mut ReportSummary, source: &SourceReport) {
    summary.sources_checked += 1;
    if source.status != SourceStatus::Current {
        summary.sources_changed += 1;
    }
    summary.files_changed += source.changes.len();
    summary.sync_safe_files += source
        .changes
        .iter()
        .filter(|change| change.disposition == Disposition::SyncSafe)
        .count();
    summary.review_required_files += source
        .changes
        .iter()
        .filter(|change| change.disposition == Disposition::ReviewRequired)
        .count();
    summary.ignored_files += source
        .changes
        .iter()
        .filter(|change| change.disposition == Disposition::Ignored)
        .count();
    summary.token_changes += source.token_changes.len();
}

fn count_local_icons() -> Result<usize> {
    let directory = root().join("crates/tdesign-gpui-assets/assets/icons");
    let entries =
        fs::read_dir(&directory).with_context(|| format!("read {}", directory.display()))?;
    Ok(entries
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().extension().and_then(OsStr::to_str) == Some("svg"))
        .count())
}

fn write_json_report(report: &UpstreamReport, path: &Path) -> Result<()> {
    if path == Path::new("-") {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(report).context("serialize upstream report")?;
    fs::write(path, format!("{json}\n")).with_context(|| format!("write {}", path.display()))
}

fn write_review_report(report: &UpstreamReport) -> Result<()> {
    let directory = root().join("upstream");
    fs::create_dir_all(&directory)?;
    let json = serde_json::to_string_pretty(report)?;
    fs::write(directory.join("review-report.json"), format!("{json}\n"))?;

    let mut markdown = String::from("# Upstream review required\n\n");
    markdown.push_str(&format!("Generated: `{}`\n\n", report.generated_at));
    markdown.push_str("This report is generated by `cargo xtask upstream sync --apply`.\n\n");
    for source in &report.sources {
        if source.status != SourceStatus::ReviewRequired {
            continue;
        }
        markdown.push_str(&format!(
            "## {}\n\n`{}` → `{}`\n\n",
            source.name, source.locked_commit, source.target_commit
        ));
        for change in &source.changes {
            if change.disposition != Disposition::ReviewRequired {
                continue;
            }
            let old_path = change
                .old_path
                .as_deref()
                .map(|old| format!(" (from `{old}`)"))
                .unwrap_or_default();
            markdown.push_str(&format!(
                "- **{}** `{}`{} — {}\n",
                format_change_kind(change.kind),
                change.path,
                old_path,
                change.reason
            ));
        }
        markdown.push('\n');
    }
    fs::write(directory.join("review-report.md"), markdown)?;
    Ok(())
}

fn format_change_kind(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "add",
        ChangeKind::Modified => "modify",
        ChangeKind::Deleted => "delete",
        ChangeKind::Renamed => "rename",
        ChangeKind::Copied => "copy",
        ChangeKind::TypeChanged => "type-change",
        ChangeKind::Unknown => "change",
    }
}

fn print_report(report: &UpstreamReport, path: &Path) {
    if path == Path::new("-") {
        return;
    }
    println!(
        "upstream: {:?}; {} source(s), {} changed file(s), {} review-required file(s)",
        report.overall,
        report.summary.sources_checked,
        report.summary.files_changed,
        report.summary.review_required_files
    );
    for source in &report.sources {
        if source.status != SourceStatus::Current {
            println!(
                "  {}: {:?} {} -> {}",
                source.name, source.status, source.locked_commit, source.target_commit
            );
        }
    }
    println!("upstream: JSON report written to {}", path.display());
    if report.apply_requested && report.overall == OverallStatus::ReviewRequired {
        println!("upstream: review report written to upstream/review-report.json and .md");
    }
}

fn git_output<I, S>(directory: Option<&Path>, args: I) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new("git");
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    let args = args.into_iter().collect::<Vec<_>>();
    let output = command
        .args(args.iter().map(AsRef::as_ref))
        .output()
        .context("execute git")?;
    if !output.status.success() {
        bail!(
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_icon_addition_as_safe() {
        let change = RawChange {
            kind: ChangeKind::Added,
            old_path: None,
            path: "svg/foo.svg".into(),
        };
        assert_eq!(
            classify_file("tdesign-icons", &change).0,
            Disposition::SyncSafe
        );
    }

    #[test]
    fn classifies_icon_rename_as_review() {
        let change = RawChange {
            kind: ChangeKind::Renamed,
            old_path: Some("svg/foo.svg".into()),
            path: "svg/bar.svg".into(),
        };
        assert_eq!(
            classify_file("tdesign-icons", &change).0,
            Disposition::ReviewRequired
        );
    }

    #[test]
    fn classifies_api_as_review() {
        let change = RawChange {
            kind: ChangeKind::Modified,
            old_path: None,
            path: "packages/scripts/api.json".into(),
        };
        assert_eq!(
            classify_file("tdesign-api", &change).0,
            Disposition::ReviewRequired
        );
    }

    #[test]
    fn parses_less_and_css_tokens() {
        let aliases = parse_theme_tokens(
            "style/web/_variables.less",
            "@brand-color: var(--td-brand-color);\n",
        );
        assert_eq!(aliases["alias:@brand-color"].value, "var(--td-brand-color)");
        let light = parse_theme_tokens(
            "style/web/theme/_light.less",
            "  --td-brand-color: #0052d9;\n",
        );
        assert_eq!(light["light:--td-brand-color"].value, "#0052d9");
    }

    #[test]
    fn sanitizer_rejects_external_resources() {
        assert!(validate_and_sanitize_svg("<svg><script>alert(1)</script></svg>").is_err());
        assert!(validate_and_sanitize_svg("<svg href=\"http://evil\"></svg>").is_err());
    }
}
