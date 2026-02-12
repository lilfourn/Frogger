use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const DEFAULT_MAX_FILES: usize = 20_000;
pub const DEFAULT_CHUNK_SIZE: usize = 250;
const MAX_SNIPPET_CHARS: usize = 200;

const CANONICAL_FOLDERS: &[&str] = &[
    "projects",
    "documents",
    "spreadsheets",
    "images",
    "videos",
    "audio",
    "archives",
    "data",
    "design",
    "system",
    "other",
];

const FOLDER_ALIASES: &[(&str, &[&str])] = &[
    (
        "projects",
        &[
            "project",
            "projects",
            "code",
            "dev",
            "development",
            "repo",
            "repository",
            "workspace",
        ],
    ),
    (
        "documents",
        &[
            "docs",
            "doc",
            "documents",
            "notes",
            "text",
            "writing",
            "letters",
        ],
    ),
    (
        "spreadsheets",
        &[
            "spreadsheet",
            "spreadsheets",
            "finance",
            "accounting",
            "budget",
        ],
    ),
    (
        "images",
        &["image", "images", "photos", "pictures", "screenshots"],
    ),
    ("videos", &["video", "videos", "movies", "clips"]),
    ("audio", &["audio", "music", "sound", "podcasts"]),
    (
        "archives",
        &["archive", "archives", "compressed", "backup", "backups"],
    ),
    ("data", &["data", "datasets", "csv", "json", "logs"]),
    ("design", &["design", "graphics", "assets", "ui", "ux"]),
    ("system", &["system", "hidden", "config", "metadata"]),
    ("other", &["other", "misc", "miscellaneous", "unknown"]),
];

#[derive(Debug, Clone, Serialize)]
pub struct IndexedManifestFile {
    pub absolute_path: String,
    pub relative_path: String,
    pub parent_relative: String,
    pub depth: usize,
    pub extension: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub modified_at: Option<String>,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizeCategory {
    pub folder: String,
    pub description: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizePlan {
    pub categories: Vec<OrganizeCategory>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrganizePlanStats {
    pub total_files: usize,
    pub indexed_files: usize,
    pub skipped_hidden: usize,
    pub skipped_already_organized: usize,
    pub chunks: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrganizePlanDocument {
    pub taxonomy_version: String,
    pub categories: Vec<OrganizeCategory>,
    pub unclassified: Vec<String>,
    pub stats: OrganizePlanStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrganizeAction {
    pub tool: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrganizeActionBatch {
    pub actions: Vec<OrganizeAction>,
    pub warnings: Vec<String>,
}

pub fn canonical_folder(folder: &str) -> String {
    let normalized = folder
        .trim()
        .to_lowercase()
        .replace('&', " and ")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else if c == '/' || c == '\\' || c == ' ' || c == '-' || c == '_' {
                '_'
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>();
    let collapsed = normalized
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");

    if CANONICAL_FOLDERS.contains(&collapsed.as_str()) {
        return collapsed;
    }

    for (canonical, aliases) in FOLDER_ALIASES {
        if aliases.iter().any(|alias| *alias == collapsed) {
            return (*canonical).to_string();
        }
    }

    if collapsed.contains("project") || collapsed.contains("code") {
        return "projects".to_string();
    }
    if collapsed.contains("image") || collapsed.contains("photo") || collapsed.contains("picture") {
        return "images".to_string();
    }
    if collapsed.contains("video") || collapsed.contains("movie") {
        return "videos".to_string();
    }
    if collapsed.contains("audio") || collapsed.contains("music") {
        return "audio".to_string();
    }
    if collapsed.contains("archive") || collapsed.contains("backup") || collapsed.contains("zip") {
        return "archives".to_string();
    }
    if collapsed.contains("sheet") || collapsed.contains("finance") || collapsed.contains("budget")
    {
        return "spreadsheets".to_string();
    }
    if collapsed.contains("doc") || collapsed.contains("note") || collapsed.contains("text") {
        return "documents".to_string();
    }
    if collapsed.contains("design") || collapsed.contains("asset") {
        return "design".to_string();
    }
    if collapsed.contains("system") || collapsed.contains("hidden") || collapsed.starts_with('.') {
        return "system".to_string();
    }

    "other".to_string()
}

fn normalize_path(path: &str) -> String {
    let mut out = path.replace('\\', "/");
    while out.ends_with('/') && out.len() > 1 {
        out.pop();
    }
    out
}

fn relative_from_root(root: &str, absolute: &str) -> Option<String> {
    if root == "/" {
        return absolute.strip_prefix('/').map(|s| s.to_string());
    }
    absolute
        .strip_prefix(&(root.to_string() + "/"))
        .map(|s| s.to_string())
}

fn is_hidden_relative(relative: &str) -> bool {
    relative.split('/').any(|segment| segment.starts_with('.'))
}

fn first_segment(relative: &str) -> Option<&str> {
    relative.split('/').find(|segment| !segment.is_empty())
}

fn compact_snippet(raw: &str) -> Option<String> {
    let compact = raw
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if compact.is_empty() {
        return None;
    }
    Some(compact.chars().take(MAX_SNIPPET_CHARS).collect())
}

pub fn collect_indexed_manifest(
    conn: &Connection,
    root_path: &str,
    include_hidden: bool,
    max_files: usize,
) -> Result<(Vec<IndexedManifestFile>, usize, usize), AppError> {
    let root = normalize_path(root_path);
    let slash_pattern = format!("{root}/%");
    let root_backslash = root.replace('/', "\\");
    let backslash_pattern = format!("{root_backslash}\\%");

    let mut stmt = conn.prepare(
        "SELECT
            f.path,
            f.extension,
            f.mime_type,
            f.size_bytes,
            f.modified_at,
            o.text_content
         FROM files f
         LEFT JOIN ocr_text o ON o.file_path = f.path
         WHERE f.is_directory = 0
           AND (
             f.path = ?1
             OR f.path LIKE ?2
             OR f.path = ?3
             OR f.path LIKE ?4
           )
         ORDER BY length(f.path) ASC, f.path ASC",
    )?;

    let mut all = Vec::new();
    let mut skipped_hidden = 0usize;
    let mut skipped_already_organized = 0usize;

    let rows = stmt.query_map(
        params![root, slash_pattern, root_backslash, backslash_pattern],
        |row| {
            let absolute_path: String = row.get(0)?;
            let extension: Option<String> = row.get(1)?;
            let mime_type: Option<String> = row.get(2)?;
            let size_bytes: Option<i64> = row.get(3)?;
            let modified_at: Option<String> = row.get(4)?;
            let raw_snippet: Option<String> = row.get(5)?;
            Ok((
                absolute_path,
                extension,
                mime_type,
                size_bytes,
                modified_at,
                raw_snippet,
            ))
        },
    )?;

    for row in rows {
        let (absolute_path, extension, mime_type, size_bytes, modified_at, raw_snippet) = row?;
        let absolute_normalized = normalize_path(&absolute_path);
        let Some(relative_path) = relative_from_root(&root, &absolute_normalized) else {
            continue;
        };
        if relative_path.is_empty() {
            continue;
        }

        if !include_hidden && is_hidden_relative(&relative_path) {
            skipped_hidden += 1;
            continue;
        }

        if let Some(segment) = first_segment(&relative_path) {
            if CANONICAL_FOLDERS.contains(&segment) {
                skipped_already_organized += 1;
                continue;
            }
        }

        let parent_relative = Path::new(&relative_path)
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let depth = relative_path.split('/').count().saturating_sub(1);
        let snippet = raw_snippet.as_deref().and_then(compact_snippet);

        all.push(IndexedManifestFile {
            absolute_path,
            relative_path,
            parent_relative,
            depth,
            extension,
            mime_type,
            size_bytes,
            modified_at,
            snippet,
        });
    }

    all.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    all.truncate(max_files);

    Ok((all, skipped_hidden, skipped_already_organized))
}

pub fn chunk_manifest(
    files: &[IndexedManifestFile],
    chunk_size: usize,
) -> Vec<Vec<IndexedManifestFile>> {
    if files.is_empty() {
        return Vec::new();
    }
    let size = chunk_size.max(1);
    files.chunks(size).map(|chunk| chunk.to_vec()).collect()
}

pub fn build_tree_summary(files: &[IndexedManifestFile]) -> serde_json::Value {
    let mut top_level_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut extension_counts: BTreeMap<String, usize> = BTreeMap::new();

    for file in files {
        let top = first_segment(&file.relative_path)
            .unwrap_or("(root)")
            .to_string();
        *top_level_counts.entry(top).or_default() += 1;

        let ext = file
            .extension
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("(none)")
            .to_lowercase();
        *extension_counts.entry(ext).or_default() += 1;
    }

    serde_json::json!({
        "top_level_counts": top_level_counts,
        "extension_counts": extension_counts,
    })
}

pub fn build_chunk_prompt(
    root_path: &str,
    chunk_index: usize,
    chunk_total: usize,
    tree_summary: &serde_json::Value,
    chunk_files: &[IndexedManifestFile],
) -> Result<String, AppError> {
    let allowed_categories = CANONICAL_FOLDERS.join(", ");
    let manifest_payload = chunk_files
        .iter()
        .map(|file| {
            serde_json::json!({
                "relative_path": file.relative_path,
                "parent_relative": file.parent_relative,
                "depth": file.depth,
                "extension": file.extension,
                "mime_type": file.mime_type,
                "size_bytes": file.size_bytes,
                "modified_at": file.modified_at,
                "snippet": file.snippet,
            })
        })
        .collect::<Vec<_>>();
    let json_files = serde_json::to_string_pretty(&manifest_payload)?;
    let summary_json = serde_json::to_string_pretty(tree_summary)?;

    Ok(format!(
        "You are organizing a file tree using indexed file metadata and snippets.\n\
Root path: {root_path}\n\
Chunk: {chunk_index}/{chunk_total}\n\
\n\
Allowed folder categories (must only use these): {allowed_categories}\n\
\n\
Global tree summary JSON:\n\
{summary_json}\n\
\n\
Chunk file manifest JSON (all paths are relative to root):\n\
{json_files}\n\
\n\
Rules:\n\
1. Assign every file in this chunk to exactly one category.\n\
2. Prefer path and filename signals first, then extension/mime, then snippet.\n\
3. Keep sibling files and parent-folder cohorts together unless there is a strong reason not to.\n\
4. Use \"other\" only as a last resort when evidence is truly ambiguous.\n\
5. Hidden/system files should go to \"system\" only if present in manifest.\n\
6. Tie-breakers in priority order: path semantics > extension/mime > snippet text > recency/size.\n\
7. Return ONLY a JSON code block with this schema:\n\
```json\n\
{{\n\
  \"categories\": [\n\
    {{ \"folder\": \"projects\", \"description\": \"short reason\", \"files\": [\"relative/path.ext\"] }}\n\
  ]\n\
}}\n\
```\n\
No other text."
    ))
}

pub fn build_refinement_chunk_prompt(
    root_path: &str,
    chunk_index: usize,
    chunk_total: usize,
    tree_summary: &serde_json::Value,
    chunk_files: &[IndexedManifestFile],
    current_assignments: &HashMap<String, String>,
) -> Result<String, AppError> {
    let allowed_categories = CANONICAL_FOLDERS.join(", ");
    let manifest_payload = chunk_files
        .iter()
        .map(|file| {
            let current_category = current_assignments
                .get(&file.relative_path)
                .cloned()
                .unwrap_or_else(|| "other".to_string());
            serde_json::json!({
                "relative_path": file.relative_path,
                "parent_relative": file.parent_relative,
                "depth": file.depth,
                "extension": file.extension,
                "mime_type": file.mime_type,
                "size_bytes": file.size_bytes,
                "modified_at": file.modified_at,
                "snippet": file.snippet,
                "current_category": current_category,
            })
        })
        .collect::<Vec<_>>();
    let json_files = serde_json::to_string_pretty(&manifest_payload)?;
    let summary_json = serde_json::to_string_pretty(tree_summary)?;

    Ok(format!(
        "You are refining uncertain file classifications for a file tree.\n\
Root path: {root_path}\n\
Refinement chunk: {chunk_index}/{chunk_total}\n\
\n\
Allowed folder categories (must only use these): {allowed_categories}\n\
\n\
Global tree summary JSON:\n\
{summary_json}\n\
\n\
Refinement file manifest JSON (all paths are relative to root):\n\
{json_files}\n\
\n\
Rules:\n\
1. Assign every file to exactly one category.\n\
2. Move files out of \"other\" only when evidence is clear.\n\
3. If evidence is weak or conflicting, keep file in \"other\".\n\
4. Keep decisions consistent with folder structure and sibling files.\n\
5. Return ONLY a JSON code block with this schema:\n\
```json\n\
{{\n\
  \"categories\": [\n\
    {{ \"folder\": \"projects\", \"description\": \"short reason\", \"files\": [\"relative/path.ext\"] }}\n\
  ]\n\
}}\n\
```\n\
No other text."
    ))
}

pub fn extract_json_payload(text: &str) -> Option<String> {
    let trimmed = text.trim();

    if let Some(start) = trimmed.find("```json") {
        let rest = &trimmed[start + "```json".len()..];
        if let Some(end) = rest.find("```") {
            return Some(rest[..end].trim().to_string());
        }
    }

    if let Some(start) = trimmed.find("```") {
        let rest = &trimmed[start + 3..];
        if let Some(newline) = rest.find('\n') {
            let body = &rest[newline + 1..];
            if let Some(end) = body.find("```") {
                return Some(body[..end].trim().to_string());
            }
        }
    }

    let first = trimmed.find('{')?;
    let last = trimmed.rfind('}')?;
    if first <= last {
        return Some(trimmed[first..=last].to_string());
    }

    None
}

pub fn parse_chunk_plan(
    text: &str,
    chunk_rel_paths: &HashSet<String>,
) -> Result<HashMap<String, Vec<String>>, AppError> {
    let payload = extract_json_payload(text).ok_or_else(|| {
        AppError::General("Model response did not contain a valid JSON payload".to_string())
    })?;
    let parsed: OrganizePlan = serde_json::from_str(&payload)?;

    let mut per_category: HashMap<String, Vec<String>> = HashMap::new();
    let mut assigned: HashSet<String> = HashSet::new();

    for category in parsed.categories {
        let folder = canonical_folder(&category.folder);
        for file in category.files {
            let rel = normalize_path(file.trim());
            if rel.is_empty() || !chunk_rel_paths.contains(&rel) || assigned.contains(&rel) {
                continue;
            }
            per_category
                .entry(folder.clone())
                .or_default()
                .push(rel.clone());
            assigned.insert(rel);
        }
    }

    for rel in chunk_rel_paths {
        if !assigned.contains(rel) {
            per_category
                .entry("other".to_string())
                .or_default()
                .push(rel.clone());
        }
    }

    Ok(per_category)
}

fn assign_path(assignments: &mut HashMap<String, String>, rel_path: String, folder: String) {
    if rel_path.is_empty() {
        return;
    }

    match assignments.get(&rel_path) {
        Some(existing) if existing != "other" && folder == "other" => {}
        Some(existing) if existing != "other" && folder != "other" => {}
        Some(existing) if existing == "other" && folder == "other" => {}
        Some(_) => {
            assignments.insert(rel_path, folder);
        }
        None => {
            assignments.insert(rel_path, folder);
        }
    }
}

pub fn category_assignments(
    category_map: &HashMap<String, Vec<String>>,
) -> HashMap<String, String> {
    let mut assignments = HashMap::new();

    for canonical in CANONICAL_FOLDERS {
        if let Some(files) = category_map.get(*canonical) {
            for rel in files {
                let normalized = normalize_path(rel.trim());
                assign_path(&mut assignments, normalized, (*canonical).to_string());
            }
        }
    }

    for (folder, files) in category_map {
        let canonical = canonical_folder(folder);
        if CANONICAL_FOLDERS.contains(&folder.as_str()) {
            continue;
        }
        for rel in files {
            let normalized = normalize_path(rel.trim());
            assign_path(&mut assignments, normalized, canonical.clone());
        }
    }

    assignments
}

pub fn assignments_to_category_map(
    assignments: &HashMap<String, String>,
) -> HashMap<String, Vec<String>> {
    let mut merged: HashMap<String, Vec<String>> = HashMap::new();

    for (path, folder) in assignments {
        let normalized_path = normalize_path(path.trim());
        if normalized_path.is_empty() {
            continue;
        }
        let canonical = canonical_folder(folder);
        merged.entry(canonical).or_default().push(normalized_path);
    }

    for files in merged.values_mut() {
        files.sort();
        files.dedup();
    }

    merged
}

pub fn reconcile_with_refinement(
    initial: HashMap<String, Vec<String>>,
    refinement: HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<String>> {
    let mut assignments = category_assignments(&initial);
    let refinement_assignments = category_assignments(&refinement);

    for (path, folder) in refinement_assignments {
        if folder == "other" {
            assignments.entry(path).or_insert(folder);
            continue;
        }
        assignments.insert(path, folder);
    }

    assignments_to_category_map(&assignments)
}

pub fn build_plan_document(
    merged_categories: HashMap<String, Vec<String>>,
    stats: OrganizePlanStats,
) -> OrganizePlanDocument {
    let mut categories = Vec::new();

    for canonical in CANONICAL_FOLDERS {
        if let Some(mut files) = merged_categories.get(*canonical).cloned() {
            files.sort();
            files.dedup();
            if files.is_empty() {
                continue;
            }
            categories.push(OrganizeCategory {
                folder: (*canonical).to_string(),
                description: format!("{canonical} files"),
                files,
            });
        }
    }

    let unclassified = merged_categories
        .get("other")
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    OrganizePlanDocument {
        taxonomy_version: "v1".to_string(),
        categories,
        unclassified,
        stats,
    }
}

pub fn plan_to_json_block(plan: &OrganizePlanDocument) -> Result<String, AppError> {
    let pretty = serde_json::to_string_pretty(plan)?;
    Ok(format!("```json\n{pretty}\n```"))
}

fn join_normalized(base: &str, tail: &str) -> String {
    if base == "/" {
        format!("/{tail}")
    } else {
        format!("{base}/{tail}")
    }
}

fn unique_destination(
    destination: &str,
    planned_sources: &HashSet<String>,
    reserved_destinations: &HashSet<String>,
    existing_paths: &HashSet<String>,
) -> Option<String> {
    if destination.is_empty() {
        return None;
    }

    if reserved_destinations.contains(destination) {
        return None;
    }

    if existing_paths.contains(destination) && !planned_sources.contains(destination) {
        return None;
    }

    if Path::new(destination).exists() && !planned_sources.contains(destination) {
        return None;
    }

    Some(destination.to_string())
}

pub fn build_action_batch(
    root_path: &str,
    plan: &OrganizePlan,
    existing_absolute_paths: &HashSet<String>,
) -> OrganizeActionBatch {
    let root = normalize_path(root_path);
    let mut warnings = Vec::new();
    let mut create_dirs: BTreeSet<String> = BTreeSet::new();
    let mut moves_by_dest: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut reserved_destinations: HashSet<String> = HashSet::new();

    let mut planned_sources: HashSet<String> = HashSet::new();
    for category in &plan.categories {
        for rel in &category.files {
            let relative = normalize_path(rel.trim());
            if relative.is_empty() {
                continue;
            }
            planned_sources.insert(join_normalized(&root, &relative));
        }
    }

    for category in &plan.categories {
        let folder = canonical_folder(&category.folder);
        for rel in &category.files {
            let relative = normalize_path(rel.trim());
            if relative.is_empty() {
                continue;
            }

            let source_abs = join_normalized(&root, &relative);
            if !existing_absolute_paths.contains(&source_abs) {
                warnings.push(format!("Missing source in index, skipped: {source_abs}"));
                continue;
            }

            let file_name = Path::new(&relative)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if file_name.is_empty() {
                warnings.push(format!("Invalid source path, skipped: {source_abs}"));
                continue;
            }

            let parent_relative = Path::new(&relative)
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let dest_dir = if parent_relative.is_empty() {
                join_normalized(&root, &folder)
            } else {
                join_normalized(&root, &format!("{folder}/{parent_relative}"))
            };
            let destination_abs = join_normalized(&dest_dir, &file_name);

            if source_abs == destination_abs {
                continue;
            }

            let Some(_safe_destination) = unique_destination(
                &destination_abs,
                &planned_sources,
                &reserved_destinations,
                existing_absolute_paths,
            ) else {
                warnings.push(format!(
                    "Destination collision, skipped move: {source_abs} -> {destination_abs}"
                ));
                continue;
            };

            create_dirs.insert(dest_dir.clone());
            moves_by_dest.entry(dest_dir).or_default().push(source_abs);
            reserved_destinations.insert(destination_abs);
        }
    }

    let mut actions = Vec::new();

    for path in create_dirs {
        actions.push(OrganizeAction {
            tool: "create_directory".to_string(),
            args: serde_json::json!({ "path": path }),
        });
    }

    for (dest_dir, mut sources) in moves_by_dest {
        sources.sort();
        sources.dedup();
        if sources.is_empty() {
            continue;
        }
        actions.push(OrganizeAction {
            tool: "move_files".to_string(),
            args: serde_json::json!({ "sources": sources, "dest_dir": dest_dir }),
        });
    }

    OrganizeActionBatch { actions, warnings }
}

pub fn actions_to_blocks(batch: &OrganizeActionBatch) -> Result<String, AppError> {
    if batch.actions.is_empty() {
        let warning_text = if batch.warnings.is_empty() {
            "No safe actions were generated.".to_string()
        } else {
            batch.warnings.join("\n")
        };
        return Err(AppError::General(warning_text));
    }

    let mut out = String::new();

    if !batch.warnings.is_empty() {
        out.push_str("Warnings:\n");
        for warning in &batch.warnings {
            out.push_str("- ");
            out.push_str(warning);
            out.push('\n');
        }
        out.push('\n');
    }

    for action in &batch.actions {
        let payload = serde_json::json!({
            "tool": action.tool,
            "args": action.args,
        });
        out.push_str("```action\n");
        out.push_str(&serde_json::to_string(&payload)?);
        out.push_str("\n```\n");
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_folder_maps_aliases() {
        assert_eq!(canonical_folder("Docs"), "documents");
        assert_eq!(canonical_folder("project files"), "projects");
        assert_eq!(canonical_folder("Finance"), "spreadsheets");
        assert_eq!(canonical_folder("weird_bucket"), "other");
    }

    #[test]
    fn extract_json_payload_handles_codeblock() {
        let text = "hello\n```json\n{\"categories\": []}\n```\n";
        let payload = extract_json_payload(text).unwrap();
        assert_eq!(payload, "{\"categories\": []}");
    }

    #[test]
    fn parse_chunk_plan_assigns_unclassified_to_other() {
        let text = "```json\n{\"categories\":[{\"folder\":\"projects\",\"description\":\"x\",\"files\":[\"a/b.txt\"]}]}\n```";
        let chunk: HashSet<String> = ["a/b.txt".to_string(), "c.txt".to_string()]
            .into_iter()
            .collect();
        let parsed = parse_chunk_plan(text, &chunk).unwrap();
        assert_eq!(
            parsed.get("projects").unwrap(),
            &vec!["a/b.txt".to_string()]
        );
        assert_eq!(parsed.get("other").unwrap(), &vec!["c.txt".to_string()]);
    }

    #[test]
    fn build_action_batch_preserves_relative_tree() {
        let plan = OrganizePlan {
            categories: vec![OrganizeCategory {
                folder: "projects".to_string(),
                description: "x".to_string(),
                files: vec!["foo/bar.txt".to_string()],
            }],
        };
        let mut existing = HashSet::new();
        existing.insert("/root/foo/bar.txt".to_string());

        let batch = build_action_batch("/root", &plan, &existing);
        assert_eq!(batch.actions.len(), 2);
        assert_eq!(batch.actions[0].tool, "create_directory");
        assert_eq!(batch.actions[1].tool, "move_files");
    }

    #[test]
    fn category_assignments_prefers_non_other() {
        let mut map = HashMap::new();
        map.insert(
            "other".to_string(),
            vec!["src/main.rs".to_string(), "docs/readme.md".to_string()],
        );
        map.insert(
            "projects".to_string(),
            vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
        );

        let assignments = category_assignments(&map);

        assert_eq!(assignments.get("src/main.rs").unwrap(), "projects");
        assert_eq!(assignments.get("src/lib.rs").unwrap(), "projects");
        assert_eq!(assignments.get("docs/readme.md").unwrap(), "other");
    }

    #[test]
    fn reconcile_with_refinement_promotes_other_files() {
        let mut initial = HashMap::new();
        initial.insert(
            "other".to_string(),
            vec!["a.txt".to_string(), "b.txt".to_string()],
        );
        initial.insert("projects".to_string(), vec!["c.rs".to_string()]);

        let mut refinement = HashMap::new();
        refinement.insert("documents".to_string(), vec!["a.txt".to_string()]);
        refinement.insert("other".to_string(), vec!["b.txt".to_string()]);

        let reconciled = reconcile_with_refinement(initial, refinement);

        assert_eq!(
            reconciled.get("documents").unwrap(),
            &vec!["a.txt".to_string()]
        );
        assert_eq!(reconciled.get("other").unwrap(), &vec!["b.txt".to_string()]);
        assert_eq!(
            reconciled.get("projects").unwrap(),
            &vec!["c.rs".to_string()]
        );
    }

    #[test]
    fn build_refinement_chunk_prompt_includes_current_category() {
        let files = vec![IndexedManifestFile {
            absolute_path: "/root/a.txt".to_string(),
            relative_path: "a.txt".to_string(),
            parent_relative: "".to_string(),
            depth: 0,
            extension: Some("txt".to_string()),
            mime_type: Some("text/plain".to_string()),
            size_bytes: Some(3),
            modified_at: None,
            snippet: Some("abc".to_string()),
        }];
        let mut assignments = HashMap::new();
        assignments.insert("a.txt".to_string(), "other".to_string());

        let prompt = build_refinement_chunk_prompt(
            "/root",
            1,
            1,
            &serde_json::json!({"top_level_counts": {}, "extension_counts": {}}),
            &files,
            &assignments,
        )
        .unwrap();

        assert!(prompt.contains("\"current_category\": \"other\""));
        assert!(prompt.contains("Refinement chunk: 1/1"));
    }
}
