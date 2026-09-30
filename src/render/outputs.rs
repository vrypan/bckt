use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::cache::{read_cached_string, store_cached_string};
use super::listing::{HomePageCache, legacy_homepage_outputs};
use super::utils::{normalize_path, remove_dir_if_empty, remove_file_if_exists};
use super::{
    FEED_CACHE_PREFIX, MONTH_ARCHIVE_PREFIX, POST_HASH_PREFIX, SEARCH_INDEX_KEY, SITEMAP_CACHE_KEY,
    TAG_CACHE_PREFIX, YEAR_ARCHIVE_PREFIX,
};

pub(super) const OUTPUTS_KEY: &str = "generated_outputs_v1";
pub(super) const PENDING_OUTPUTS_KEY: &str = "generated_outputs_pending_v1";
const MANIFEST_VERSION: u32 = 1;

/// Output producers in renderer write order: a later producer overwrites an
/// earlier one when both generate the same path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Producer {
    Posts,
    Indexes,
    Feeds,
    Search,
    Pages,
    Static,
}

pub(super) type Inventory = BTreeSet<String>;

/// Relative output paths grouped by producer. A producer missing from the map
/// has no recorded ownership; a present producer with an empty set owns nothing.
/// `refresh` lists producers whose cache digests cannot be trusted and that
/// must rewrite everything the next time they run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct OutputManifest {
    version: u32,
    producers: BTreeMap<Producer, Inventory>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    refresh: BTreeSet<Producer>,
}

impl OutputManifest {
    pub(super) fn new() -> Self {
        Self {
            version: MANIFEST_VERSION,
            producers: BTreeMap::new(),
            refresh: BTreeSet::new(),
        }
    }

    pub(super) fn load(db: &sled::Db, key: &str) -> Result<Option<Self>> {
        let Some(raw) = read_cached_string(db, key)? else {
            return Ok(None);
        };
        let manifest: Self = serde_json::from_str(&raw)
            .with_context(|| format!("output manifest {key} is malformed"))?;
        if manifest.version != MANIFEST_VERSION {
            bail!(
                "output manifest {key} has unsupported version {}",
                manifest.version
            );
        }
        for path in manifest.producers.values().flatten() {
            validate_output_path(path)
                .with_context(|| format!("output manifest {key} is invalid"))?;
        }
        Ok(Some(manifest))
    }

    pub(super) fn store(&self, db: &sled::Db, key: &str) -> Result<()> {
        let raw = serde_json::to_string(self).context("failed to serialize output manifest")?;
        store_cached_string(db, key, &raw)
    }

    pub(super) fn set(&mut self, producer: Producer, inventory: Inventory) {
        self.producers.insert(producer, inventory);
    }

    pub(super) fn get(&self, producer: Producer) -> Option<&Inventory> {
        self.producers.get(&producer)
    }

    pub(super) fn extend(&mut self, producer: Producer, paths: impl IntoIterator<Item = String>) {
        self.producers.entry(producer).or_default().extend(paths);
    }

    /// Per-producer union of both manifests.
    pub(super) fn union(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        for (producer, paths) in &other.producers {
            merged.extend(*producer, paths.iter().cloned());
        }
        merged.refresh.extend(other.refresh.iter().copied());
        merged
    }

    pub(super) fn owned_paths(&self) -> BTreeSet<&str> {
        self.producers
            .values()
            .flatten()
            .map(String::as_str)
            .collect()
    }

    fn owners(&self) -> BTreeMap<&str, BTreeSet<Producer>> {
        let mut owners: BTreeMap<&str, BTreeSet<Producer>> = BTreeMap::new();
        for (producer, paths) in &self.producers {
            for path in paths {
                owners.entry(path).or_default().insert(*producer);
            }
        }
        owners
    }
}

/// Output ownership for one render: the committed manifest (or a legacy seed),
/// any pending ledger left by a failed run, and this run's inventories.
pub(super) struct OutputRun {
    ledger: OutputManifest,
    next: OutputManifest,
    enabled: BTreeSet<Producer>,
    owed: BTreeSet<Producer>,
}

impl OutputRun {
    pub(super) fn plan(
        committed: OutputManifest,
        pending: Option<OutputManifest>,
        current: BTreeMap<Producer, Inventory>,
    ) -> Self {
        let before = match pending {
            Some(pending) => committed.union(&pending),
            None => committed,
        };
        let enabled: BTreeSet<Producer> = current.keys().copied().collect();
        let mut attempted = OutputManifest::new();
        for (producer, inventory) in &current {
            attempted.set(*producer, inventory.clone());
        }
        let ledger = before.union(&attempted);
        let next = next_manifest(&ledger, &current);
        let mut owed = before.refresh.clone();
        for owners in changed_shared_paths(&before, &next).into_values() {
            owed.extend(owners);
        }
        Self {
            ledger,
            next,
            enabled,
            owed,
        }
    }

    /// Whether an enabled, cache-gated producer must rewrite all its outputs.
    /// Pages render unconditionally, so they never need a forced refresh.
    pub(super) fn requires_refresh(&self) -> bool {
        self.owed
            .iter()
            .any(|producer| *producer != Producer::Pages && self.enabled.contains(producer))
    }

    /// Whether `producer` shares an output path with another producer, so an
    /// earlier stage may have overwritten its bytes during this run.
    pub(super) fn overlays_other_outputs(&self, producer: Producer) -> bool {
        self.next
            .owners()
            .values()
            .any(|owners| owners.len() > 1 && owners.contains(&producer))
    }

    /// Record everything this run may write before writing it, so a failed run
    /// leaves its outputs discoverable and forces a refresh on retry.
    pub(super) fn begin(&self, db: &sled::Db) -> Result<()> {
        let mut pending = self.ledger.clone();
        pending.refresh = self.owed.union(&self.enabled).copied().collect();
        pending.store(db, PENDING_OUTPUTS_KEY)?;
        db.flush()
            .context("failed to flush output ownership ledger")?;
        Ok(())
    }

    /// Remove obsolete outputs, then commit the new ownership. Call only after
    /// every enabled stage succeeded.
    pub(super) fn finish(mut self, db: &sled::Db, html_root: &Path) -> Result<usize> {
        let obsolete = obsolete_paths(&self.ledger, &self.next, &self.enabled);
        let removed = remove_obsolete_outputs(html_root, &obsolete)?;
        self.next.refresh = self.owed.difference(&self.enabled).copied().collect();
        self.next.store(db, OUTPUTS_KEY)?;
        db.remove(PENDING_OUTPUTS_KEY)
            .context("failed to clear output ownership ledger")?;
        Ok(removed)
    }
}

/// Seed ownership from cache keys written before ownership tracking existed.
/// Only exact files named by those keys are claimed; directories, attachments,
/// pages, and static files have no provenance and stay untouched.
pub(super) fn legacy_manifest(
    db: &sled::Db,
    html_root: &Path,
    homepage: &HomePageCache,
    search_path: &Path,
) -> Result<OutputManifest> {
    let mut manifest = OutputManifest::new();
    let posts = legacy_suffixes(db, POST_HASH_PREFIX)?
        .into_iter()
        .map(|permalink| Path::new(permalink.trim_start_matches('/')).join("index.html"));
    manifest.extend(Producer::Posts, owned(html_root, posts));

    let mut indexes = legacy_homepage_outputs(homepage, html_root)?;
    let tags = legacy_suffixes(db, TAG_CACHE_PREFIX)?
        .into_iter()
        .map(|slug| Path::new("tags").join(slug).join("index.html"));
    let years = legacy_suffixes(db, YEAR_ARCHIVE_PREFIX)?
        .into_iter()
        .map(|year| Path::new(&year).join("index.html"));
    let months = legacy_suffixes(db, MONTH_ARCHIVE_PREFIX)?
        .into_iter()
        .map(|month| Path::new(&month.replacen('-', "/", 1)).join("index.html"));
    indexes.extend(owned(html_root, tags.chain(years).chain(months)));
    manifest.extend(Producer::Indexes, indexes);

    let mut feeds: Vec<PathBuf> = legacy_suffixes(db, FEED_CACHE_PREFIX)?
        .into_iter()
        .map(|feed| PathBuf::from(feed.trim_start_matches('/')))
        .collect();
    if db.contains_key(SITEMAP_CACHE_KEY)? {
        feeds.push(PathBuf::from("sitemap.xml"));
    }
    manifest.extend(Producer::Feeds, owned(html_root, feeds));

    if db.contains_key(SEARCH_INDEX_KEY)? {
        manifest.extend(Producer::Search, relative_output(html_root, search_path));
    }
    Ok(manifest)
}

fn legacy_suffixes(db: &sled::Db, prefix: &str) -> Result<Vec<String>> {
    let mut suffixes = Vec::new();
    for entry in db.scan_prefix(prefix.as_bytes()) {
        let (key, _) = entry.context("failed to iterate legacy cache entries")?;
        if let Some(suffix) = std::str::from_utf8(&key)
            .ok()
            .and_then(|key| key.strip_prefix(prefix))
        {
            suffixes.push(suffix.to_string());
        }
    }
    Ok(suffixes)
}

fn owned(html_root: &Path, relatives: impl IntoIterator<Item = PathBuf>) -> Inventory {
    relatives
        .into_iter()
        .filter_map(|relative| relative_output(html_root, &html_root.join(relative)))
        .collect()
}

/// Ownership after a run: enabled producers take their current inventory,
/// skipped producers keep whatever the ledger recorded for them.
pub(super) fn next_manifest(
    ledger: &OutputManifest,
    current: &BTreeMap<Producer, Inventory>,
) -> OutputManifest {
    let mut next = ledger.clone();
    for (producer, inventory) in current {
        next.set(*producer, inventory.clone());
    }
    next
}

/// Paths previously owned by an enabled producer that no producer owns now.
pub(super) fn obsolete_paths(
    ledger: &OutputManifest,
    next: &OutputManifest,
    enabled: &BTreeSet<Producer>,
) -> BTreeSet<String> {
    let keep = next.owned_paths();
    enabled
        .iter()
        .filter_map(|producer| ledger.get(*producer))
        .flatten()
        .filter(|path| !keep.contains(path.as_str()))
        .cloned()
        .collect()
}

/// Paths whose owner set changed while more than one producer owns them before
/// or after. Such paths may hold an overlay's bytes and need a refresh.
pub(super) fn changed_shared_paths(
    previous: &OutputManifest,
    next: &OutputManifest,
) -> BTreeMap<String, BTreeSet<Producer>> {
    let before = previous.owners();
    let after = next.owners();
    let empty = BTreeSet::new();
    let mut changed = BTreeMap::new();
    for path in before.keys().chain(after.keys()) {
        let old = before.get(path).unwrap_or(&empty);
        let new = after.get(path).unwrap_or(&empty);
        if old != new && (old.len() > 1 || new.len() > 1) {
            changed.insert(path.to_string(), new.clone());
        }
    }
    changed
}

/// Convert an absolute output path under `html_root` into a manifest entry.
/// Paths outside `html_root` are not owned and yield `None`.
pub(super) fn relative_output(html_root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(html_root).ok()?;
    let normalized = normalize_path(relative);
    validate_output_path(&normalized).ok()?;
    Some(normalized)
}

pub(super) fn validate_output_path(path: &str) -> Result<PathBuf> {
    if path.is_empty() || path.contains('\\') {
        bail!("invalid output path '{path}'");
    }
    let candidate = PathBuf::from(path);
    let all_normal = candidate
        .components()
        .all(|component| matches!(component, Component::Normal(_)));
    if !all_normal || normalize_path(&candidate) != path {
        bail!("invalid output path '{path}'");
    }
    Ok(candidate)
}

/// Remove obsolete generated files and then any directories they leave empty.
/// Every path is validated before anything is deleted.
pub(super) fn remove_obsolete_outputs(html_root: &Path, paths: &BTreeSet<String>) -> Result<usize> {
    let mut targets = Vec::new();
    for path in paths {
        let relative = validate_output_path(path)?;
        if ancestors_present(html_root, &relative)? {
            targets.push(relative);
        }
    }

    for relative in &targets {
        remove_file_if_exists(&html_root.join(relative))?;
        let mut parent = relative.parent();
        while let Some(dir) = parent.filter(|dir| !dir.as_os_str().is_empty()) {
            remove_dir_if_empty(&html_root.join(dir))?;
            parent = dir.parent();
        }
    }
    Ok(targets.len())
}

/// Check the directories between `html_root` and the file. Returns false when
/// one is missing (the file is already gone) and refuses symlinked ancestors.
fn ancestors_present(html_root: &Path, relative: &Path) -> Result<bool> {
    let mut current = html_root.to_path_buf();
    let Some(parent) = relative.parent() else {
        return Ok(true);
    };
    for component in parent.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => bail!(
                "refusing to clean up output through symlinked directory {}",
                current.display()
            ),
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Ok(false),
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to inspect {}", current.display()));
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn inventory(paths: &[&str]) -> Inventory {
        paths.iter().map(|path| path.to_string()).collect()
    }

    fn manifest(entries: &[(Producer, &[&str])]) -> OutputManifest {
        let mut manifest = OutputManifest::new();
        for (producer, paths) in entries {
            manifest.set(*producer, inventory(paths));
        }
        manifest
    }

    fn touch(root: &Path, relative: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, relative).unwrap();
    }

    #[test]
    fn obsolete_paths_skip_files_still_owned_by_another_producer() {
        let ledger = manifest(&[
            (Producer::Indexes, &["index.html", "page/1/index.html"]),
            (Producer::Pages, &["index.html"]),
        ]);
        let mut current = BTreeMap::new();
        current.insert(Producer::Pages, Inventory::new());
        current.insert(Producer::Indexes, inventory(&["index.html"]));
        let next = next_manifest(&ledger, &current);
        let enabled = current.keys().copied().collect();

        let obsolete = obsolete_paths(&ledger, &next, &enabled);

        assert_eq!(obsolete, inventory(&["page/1/index.html"]));
    }

    #[test]
    fn skipped_producers_keep_their_ownership() {
        let ledger = manifest(&[
            (Producer::Posts, &["a/index.html"]),
            (Producer::Static, &["style.css"]),
        ]);
        let mut current = BTreeMap::new();
        current.insert(Producer::Posts, Inventory::new());
        let next = next_manifest(&ledger, &current);
        let enabled = current.keys().copied().collect();

        let obsolete = obsolete_paths(&ledger, &next, &enabled);

        assert_eq!(obsolete, inventory(&["a/index.html"]));
        assert_eq!(next.get(Producer::Static), Some(&inventory(&["style.css"])));
        assert_eq!(next.get(Producer::Posts), Some(&Inventory::new()));
        assert_eq!(next.get(Producer::Pages), None);
    }

    #[test]
    fn rejects_invalid_output_paths() {
        for path in [
            "",
            "/etc/passwd",
            "../outside",
            "a/../b",
            "./a",
            "a\\b",
            "a//b",
        ] {
            assert!(validate_output_path(path).is_err(), "accepted {path:?}");
        }
        assert!(validate_output_path("tags/rust/index.html").is_ok());
        assert_eq!(
            relative_output(Path::new("/site/html"), Path::new("/tmp/x")),
            None
        );
    }

    #[test]
    fn removal_keeps_unknown_files_and_tolerates_missing_ones() {
        let temp = TempDir::new().unwrap();
        let html = temp.path();
        touch(html, "page/2/index.html");
        touch(html, "page/2/notes.txt");
        touch(html, "page/3/index.html");

        let removed = remove_obsolete_outputs(
            html,
            &inventory(&["page/2/index.html", "page/3/index.html", "gone/index.html"]),
        )
        .unwrap();

        assert_eq!(removed, 2);
        assert!(html.join("page/2/notes.txt").exists());
        assert!(!html.join("page/2/index.html").exists());
        assert!(!html.join("page/3").exists());
        assert!(html.join("page").exists());
    }

    #[cfg(unix)]
    #[test]
    fn removal_refuses_symlinked_ancestors_before_deleting_anything() {
        let temp = TempDir::new().unwrap();
        let html = temp.path().join("html");
        let outside = temp.path().join("outside");
        touch(&outside, "index.html");
        touch(&html, "a/index.html");
        std::os::unix::fs::symlink(&outside, html.join("linked")).unwrap();

        let err =
            remove_obsolete_outputs(&html, &inventory(&["a/index.html", "linked/index.html"]))
                .unwrap_err();

        assert!(err.to_string().contains("symlinked"), "{err}");
        assert!(outside.join("index.html").exists());
        assert!(html.join("a/index.html").exists());
    }

    #[test]
    fn manifest_round_trips_and_rejects_unknown_versions() {
        let temp = TempDir::new().unwrap();
        let db = sled::open(temp.path().join("db")).unwrap();
        let stored = manifest(&[(Producer::Feeds, &["rss.xml"]), (Producer::Pages, &[])]);
        stored.store(&db, OUTPUTS_KEY).unwrap();
        assert_eq!(
            OutputManifest::load(&db, OUTPUTS_KEY).unwrap(),
            Some(stored)
        );

        store_cached_string(&db, OUTPUTS_KEY, r#"{"version":9,"producers":{}}"#).unwrap();
        assert!(OutputManifest::load(&db, OUTPUTS_KEY).is_err());
        store_cached_string(&db, OUTPUTS_KEY, "not json").unwrap();
        assert!(OutputManifest::load(&db, OUTPUTS_KEY).is_err());
        store_cached_string(
            &db,
            OUTPUTS_KEY,
            r#"{"version":1,"producers":{"posts":["../x"]}}"#,
        )
        .unwrap();
        assert!(OutputManifest::load(&db, OUTPUTS_KEY).is_err());
    }

    #[test]
    fn pending_ledger_retains_attempted_outputs_for_retry() {
        let previous = manifest(&[(Producer::Posts, &["old/index.html"])]);
        let attempted = manifest(&[(Producer::Posts, &["tried/index.html"])]);
        let ledger = previous.union(&attempted);
        let mut current = BTreeMap::new();
        current.insert(Producer::Posts, inventory(&["final/index.html"]));
        let next = next_manifest(&ledger, &current);
        let enabled = current.keys().copied().collect();

        let obsolete = obsolete_paths(&ledger, &next, &enabled);

        assert_eq!(obsolete, inventory(&["old/index.html", "tried/index.html"]));
    }

    #[test]
    fn changed_shared_paths_detects_removed_overlays() {
        let previous = manifest(&[
            (Producer::Indexes, &["index.html", "page/1/index.html"]),
            (Producer::Pages, &["index.html"]),
        ]);
        let next = manifest(&[(Producer::Indexes, &["index.html"]), (Producer::Pages, &[])]);

        let changed = changed_shared_paths(&previous, &next);

        assert_eq!(changed.len(), 1);
        assert_eq!(
            changed.get("index.html"),
            Some(&BTreeSet::from([Producer::Indexes]))
        );
    }
}
