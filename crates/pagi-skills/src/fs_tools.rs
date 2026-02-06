//! Filesystem tools for bare-metal introspection.
//!
//! Primary entrypoint: [`analyze_workspace()`](crates/pagi-skills/src/fs_tools.rs:74)
//!
//! This module implements the `fs_workspace_analyzer` discovery skill, allowing the
//! orchestrator to scan the local Rust workspace and report crate structure.
//! When a store is provided, scan results are stored in **KB_OIKOS** (Context / "The World").

use pagi_core::{AgentSkill, KbRecord, KbType, KnowledgeStore, TenantContext};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::VecDeque;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

const SKILL_NAME: &str = "fs_workspace_analyzer";

const SANDBOX_WRITE_SKILL_NAME: &str = "write_sandbox_file";

/// Arguments accepted by the `fs_workspace_analyzer` skill.
#[derive(Debug, Clone, Default, Deserialize)]
struct FsWorkspaceAnalyzerArgs {
    /// Path to analyze. If omitted, defaults to the process current directory.
    #[serde(default)]
    path: Option<String>,
    /// Maximum directory depth to traverse (0 = just the root).
    #[serde(default)]
    depth: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct CrateInfo {
    /// Best-effort crate/package name derived from Cargo.toml.
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// Path to crate directory, relative to the analyzed root (when possible).
    path: String,
    /// Path to the manifest, relative to the analyzed root (when possible).
    manifest: String,
    /// Whether `src/` exists under the crate directory.
    has_src: bool,
    /// Whether this manifest looks like a workspace root.
    is_workspace: bool,
}

/// Key under KB_OIKOS where the latest workspace scan is stored.
pub const OIKOS_WORKSPACE_SCAN_KEY: &str = "workspace_scan/latest";

/// Agent skill wrapper that exposes [`analyze_workspace()`](crates/pagi-skills/src/fs_tools.rs:74)
/// via the orchestrator skill registry. When constructed with `new_with_store`, scan output
/// is stored in **KB_OIKOS** (Context) for the cognitive map.
pub struct FsWorkspaceAnalyzer {
    store: Option<Arc<KnowledgeStore>>,
}

impl FsWorkspaceAnalyzer {
    /// No store: scan results are returned only (no KB write).
    pub fn new() -> Self {
        Self { store: None }
    }

    /// With store: scan results are also written to KB_OIKOS (workspace_scan/latest).
    pub fn new_with_store(store: Arc<KnowledgeStore>) -> Self {
        Self { store: Some(store) }
    }
}

impl Default for FsWorkspaceAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Recursively scans a Rust workspace root and returns a JSON summary of detected crates.
///
/// It identifies:
/// - `Cargo.toml` files
/// - `src/` directories under crate roots
/// - `add-ons/` folder (if present)
///
/// The scan intentionally skips common heavy directories (e.g. `target/`, `.git/`, `node_modules/`, `data/`).
pub fn analyze_workspace(path: &Path) -> serde_json::Value {
    let depth_limit = 25usize;

    let root = path.to_path_buf();

    let mut add_ons_found = false;
    let mut add_ons_path: Option<String> = None;
    let mut cargo_manifests: Vec<CrateInfo> = Vec::new();

    let skip_dir_names = [
        ".git",
        "target",
        "node_modules",
        // this repo includes large sled/db fixtures under add-ons/pagi-gateway/data
        "data",
        "db",
        "blobs",
    ];

    let mut q: VecDeque<(PathBuf, usize)> = VecDeque::new();
    q.push_back((root.clone(), 0));

    while let Some((dir, depth)) = q.pop_front() {
        // Depth guard
        if depth > depth_limit {
            continue;
        }

        // Detect add-ons folder
        if dir.file_name().and_then(|s| s.to_str()) == Some("add-ons") {
            add_ons_found = true;
            add_ons_path = Some(rel_or_abs(&root, &dir));
        }

        // Record Cargo.toml manifests at this level.
        let manifest_path = dir.join("Cargo.toml");
        if manifest_path.is_file() {
            let (pkg_name, is_workspace) = read_manifest_metadata(&manifest_path);
            let has_src = dir.join("src").is_dir();
            cargo_manifests.push(CrateInfo {
                name: pkg_name,
                path: rel_or_abs(&root, &dir),
                manifest: rel_or_abs(&root, &manifest_path),
                has_src,
                is_workspace,
            });
        }

        // Walk children directories.
        let read_dir = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in read_dir.flatten() {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }
            let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if skip_dir_names.iter().any(|&s| s.eq_ignore_ascii_case(name)) {
                continue;
            }
            q.push_back((p, depth + 1));
        }
    }

    // Sort deterministically by manifest path.
    cargo_manifests.sort_by(|a, b| a.manifest.cmp(&b.manifest));

    let crate_count = cargo_manifests.len();
    let workspace_roots: Vec<&CrateInfo> = cargo_manifests.iter().filter(|c| c.is_workspace).collect();

    let summary = format!(
        "Workspace scan root: {}\nCrates/manifests found: {}\nWorkspace manifests: {}\nadd-ons present: {}",
        rel_or_abs(&root, &root),
        crate_count,
        workspace_roots.len(),
        add_ons_found
    );

    serde_json::json!({
        "status": "ok",
        "skill": SKILL_NAME,
        "root": rel_or_abs(&root, &root),
        "crate_count": crate_count,
        "crates": cargo_manifests,
        "workspace_manifest_count": workspace_roots.len(),
        "add_ons_found": add_ons_found,
        "add_ons_path": add_ons_path,
        "summary": summary,
    })
}

fn rel_or_abs(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .ok()
        .map(|rp| {
            let s = rp.to_string_lossy().to_string();
            if s.is_empty() { ".".to_string() } else { s }
        })
        .unwrap_or_else(|| p.to_string_lossy().to_string())
        .replace('\\', "/")
}

fn read_manifest_metadata(manifest_path: &Path) -> (Option<String>, bool) {
    let Ok(text) = fs::read_to_string(manifest_path) else {
        return (None, false);
    };

    let is_workspace = text.lines().any(|l| l.trim() == "[workspace]");
    let mut in_package = false;
    let mut name: Option<String> = None;

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_package = line == "[package]";
            // Stop scanning package name once we hit another section after [package].
            if name.is_some() && !in_package {
                break;
            }
            continue;
        }
        if !in_package {
            continue;
        }

        // Very small TOML subset parse: `name = "..."`.
        let Some(rest) = line.strip_prefix("name") else { continue };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let rest = rest.trim();
        if let Some(stripped) = rest.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
            name = Some(stripped.to_string());
            break;
        }
    }

    (name, is_workspace)
}

fn canonicalize_within_base(base: &Path, candidate: &Path) -> Result<PathBuf, String> {
    let base = base
        .canonicalize()
        .map_err(|e| format!("failed to canonicalize base path: {}", e))?;
    let candidate = candidate
        .canonicalize()
        .map_err(|e| format!("failed to canonicalize candidate path: {}", e))?;
    if !candidate.starts_with(&base) {
        return Err("path is outside the workspace base directory".to_string());
    }
    Ok(candidate)
}

#[async_trait::async_trait]
impl AgentSkill for FsWorkspaceAnalyzer {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let args: FsWorkspaceAnalyzerArgs = match payload {
            Some(v) => serde_json::from_value(v).unwrap_or_default(),
            None => FsWorkspaceAnalyzerArgs::default(),
        };

        let base = std::env::current_dir()?;
        let requested = args
            .path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| base.clone());

        // Safety: restrict scanning to within the current working directory.
        let root = canonicalize_within_base(&base, &requested).map_err(std::io::Error::other)?;

        let mut out = analyze_workspace(&root);
        if let Some(d) = args.depth {
            out["requested_depth"] = serde_json::json!(d);
        }
        out["requested_path"] = serde_json::json!(requested.to_string_lossy().to_string());
        out["canonical_root"] = serde_json::json!(root.to_string_lossy().to_string());

        // Breadcrumbs: store in KB_OIKOS (Context / "The World") when store is available
        if let Some(ref store) = self.store {
            let slot_id = KbType::Oikos.slot_id();
            let content = serde_json::to_string(&out).unwrap_or_else(|_| "{}".to_string());
            let record = KbRecord::with_metadata(
                content,
                serde_json::json!({
                    "type": "workspace_scan",
                    "skill": SKILL_NAME,
                    "crate_count": out.get("crate_count").and_then(|v| v.as_u64()).unwrap_or(0),
                    "tags": ["oikos", "workspace", "context"]
                }),
            );
            if store.insert_record(slot_id, OIKOS_WORKSPACE_SCAN_KEY, &record).is_ok() {
                tracing::info!(
                    target: "pagi::fs_tools",
                    slot = slot_id,
                    key = OIKOS_WORKSPACE_SCAN_KEY,
                    "Workspace scan stored in KB_OIKOS (Context)"
                );
            }
        }

        Ok(out)
    }
}

/// Arguments accepted by the `write_sandbox_file` skill.
#[derive(Debug, Clone, Deserialize)]
struct WriteSandboxFileArgs {
    /// Target file path **within** `research_sandbox/`.
    ///
    /// Accepts either `report.md` or `research_sandbox/report.md`.
    path: String,
    /// Content to write.
    content: String,
    /// If true, appends to file instead of truncating.
    #[serde(default)]
    append: bool,
}

/// Agent skill: write a file within `research_sandbox/` only.
///
/// Safety properties:
/// - Rejects absolute paths and any `..` segments
/// - Enforces a canonicalized prefix check against the canonical sandbox root
pub struct WriteSandboxFile;

impl WriteSandboxFile {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WriteSandboxFile {
    fn default() -> Self {
        Self::new()
    }
}

fn sanitize_sandbox_rel_path(input: &str) -> Result<PathBuf, String> {
    let raw = input.trim().replace('\\', "/");
    if raw.is_empty() {
        return Err("path is required".to_string());
    }

    // Allow callers to include the folder prefix; normalize to a relative path under sandbox.
    let raw = raw
        .strip_prefix("research_sandbox/")
        .unwrap_or(raw.as_str());

    let p = Path::new(raw);
    if p.is_absolute() {
        return Err("absolute paths are forbidden".to_string());
    }

    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::Normal(seg) => out.push(seg),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err("path traversal is forbidden".to_string());
            }
        }
    }

    if out.as_os_str().is_empty() {
        return Err("path resolves to empty".to_string());
    }

    Ok(out)
}

fn canonical_sandbox_target(base: &Path, rel: &Path) -> Result<(PathBuf, PathBuf), String> {
    // Ensure sandbox exists, then canonicalize.
    let sandbox_root = base.join("research_sandbox");
    fs::create_dir_all(&sandbox_root)
        .map_err(|e| format!("failed to create research_sandbox directory: {e}"))?;
    let sandbox_root_canon = sandbox_root
        .canonicalize()
        .map_err(|e| format!("failed to canonicalize sandbox root: {e}"))?;

    let target = sandbox_root.join(rel);
    let parent = target
        .parent()
        .ok_or_else(|| "invalid path: missing parent".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create parent directories: {e}"))?;
    let parent_canon = parent
        .canonicalize()
        .map_err(|e| format!("failed to canonicalize parent directory: {e}"))?;

    if !parent_canon.starts_with(&sandbox_root_canon) {
        return Err("path is outside research_sandbox".to_string());
    }

    let file_name = target
        .file_name()
        .ok_or_else(|| "invalid path: missing filename".to_string())?;
    let target_canon = parent_canon.join(file_name);
    Ok((sandbox_root_canon, target_canon))
}

#[async_trait::async_trait]
impl AgentSkill for WriteSandboxFile {
    fn name(&self) -> &str {
        SANDBOX_WRITE_SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let args: WriteSandboxFileArgs = match payload {
            Some(v) => serde_json::from_value(v)
                .map_err(|e| std::io::Error::other(format!("invalid payload: {e}")))?,
            None => {
                return Err(std::io::Error::other(
                    "missing payload: expected { path, content, append? }",
                ))?
            }
        };

        let base = std::env::current_dir()?;
        let rel = sanitize_sandbox_rel_path(&args.path).map_err(std::io::Error::other)?;
        let (sandbox_root_canon, target_canon) =
            canonical_sandbox_target(&base, &rel).map_err(std::io::Error::other)?;

        // Final guard: ensure the final file path still prefixes the sandbox root.
        if !target_canon.starts_with(&sandbox_root_canon) {
            return Err(std::io::Error::other("path is outside research_sandbox"))?;
        }

        let bytes = if args.append {
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&target_canon)?;
            f.write_all(args.content.as_bytes())?;
            args.content.as_bytes().len()
        } else {
            fs::write(&target_canon, args.content.as_bytes())?;
            args.content.as_bytes().len()
        };

        // Provide a stable, workspace-relative-ish path for the caller.
        let relative_from_base = rel_or_abs(&base, &target_canon);

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SANDBOX_WRITE_SKILL_NAME,
            "path": relative_from_base,
            "bytes_written": bytes,
            "append": args.append,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_workspace_finds_cargo_toml() {
        // Analyze the current repo root; this is intentionally a real-world smoke test.
        let root = std::env::current_dir().unwrap();
        let v = analyze_workspace(&root);
        assert_eq!(v.get("status").and_then(|s| s.as_str()), Some("ok"));
        assert!(v.get("crate_count").and_then(|n| n.as_u64()).unwrap_or(0) >= 1);
    }

    #[test]
    fn write_sandbox_file_rejects_traversal() {
        assert!(sanitize_sandbox_rel_path("../evil.md").is_err());
        assert!(sanitize_sandbox_rel_path("research_sandbox/../evil.md").is_err());
    }

    #[test]
    fn write_sandbox_file_rejects_absolute() {
        // Windows absolute path via prefix component.
        assert!(sanitize_sandbox_rel_path("C:\\Windows\\win.ini").is_err());
    }

    #[test]
    fn write_sandbox_file_allows_prefix_and_normalizes() {
        let p = sanitize_sandbox_rel_path("research_sandbox/report_01.md").unwrap();
        assert_eq!(p.to_string_lossy().replace('\\', "/"), "report_01.md");
    }

    #[test]
    fn write_sandbox_file_happy_path_writes() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let tmp_root = std::env::temp_dir().join(format!(
            "pagi_sandbox_writer_test_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&tmp_root).unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&tmp_root).unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let res = rt
            .block_on(WriteSandboxFile::new().execute(
                &TenantContext {
                    tenant_id: "t".to_string(),
                    correlation_id: None,
                    agent_id: None,
                },
                Some(serde_json::json!({
                    "path": "report.md",
                    "content": "hello",
                    "append": false
                })),
            ))
            .unwrap();

        assert_eq!(res.get("status").and_then(|v| v.as_str()), Some("ok"));
        let wrote = fs::read_to_string(tmp_root.join("research_sandbox").join("report.md")).unwrap();
        assert_eq!(wrote, "hello");

        std::env::set_current_dir(prev).unwrap();
        let _ = fs::remove_dir_all(tmp_root);
    }
}

