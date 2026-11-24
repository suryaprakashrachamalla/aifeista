use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn default_config_root() -> PathBuf {
    if let Ok(custom) = std::env::var("AI_CARNIVAL_CONFIG_DIR") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config/ai-carnival")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecipeKind {
    Layout,
    Mod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeRecord {
    pub id: String,
    pub name: String,
    pub kind: RecipeKind,
    pub builtin: bool,
    pub base_layout: String,
    pub description: String,
    pub filter_status: Option<String>,
    pub accent_hex: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub stars: u32,
    #[serde(default)]
    pub trending: bool,
    #[serde(default)]
    pub source_url: String,
    pub css: String,
    pub recipe_md: String,
    pub path: String,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveStudioSelection {
    pub active_layout: String,
    pub enabled_mods: Vec<String>,
    pub updated_at_ms: i64,
}

impl Default for ActiveStudioSelection {
    fn default() -> Self {
        Self {
            active_layout: "kanso".to_string(),
            enabled_mods: vec!["mod-telemetry".to_string()],
            updated_at_ms: now_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub snapshot_id: String,
    pub created_at_ms: i64,
    pub note: String,
    pub selection: ActiveStudioSelection,
    pub created_recipe_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioRecipesState {
    pub config_root: String,
    pub active_layout: String,
    pub enabled_mods: Vec<String>,
    pub layouts: Vec<RecipeRecord>,
    pub mods: Vec<RecipeRecord>,
    pub has_rollback: bool,
    pub last_snapshot_note: Option<String>,
    pub revision: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveRecipeRequest {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub kind: RecipeKind,
    #[serde(default)]
    pub base_layout: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub filter_status: Option<String>,
    #[serde(default)]
    pub accent_hex: Option<String>,
    #[serde(default)]
    pub css: Option<String>,
    #[serde(default)]
    pub recipe_md: Option<String>,
    #[serde(default)]
    pub activate: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct StudioRecipes {
    root: PathBuf,
    lock: Arc<Mutex<()>>,
}

const OBSOLETE_LAYOUT_IDS: &[&str] = &[
    "kanban",
    "mission",
    "village",
    "departure",
    "threads",
    "radar",
    "custom-carnival-noir",
];

impl StudioRecipes {
    pub fn default_location() -> Self {
        Self::new(default_config_root())
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root_path = root.into();
        let mgr = Self {
            root: root_path,
            lock: Arc::new(Mutex::new(())),
        };
        let _ = mgr.ensure_seeded();
        mgr
    }

    fn layouts_dir(&self) -> PathBuf {
        self.root.join("layouts")
    }

    fn mods_dir(&self) -> PathBuf {
        self.root.join("mods")
    }

    fn history_dir(&self) -> PathBuf {
        self.root.join("history")
    }

    fn active_file(&self) -> PathBuf {
        self.root.join("active.json")
    }

    fn ensure_seeded(&self) -> Result<(), String> {
        fs::create_dir_all(self.layouts_dir()).map_err(|e| e.to_string())?;
        fs::create_dir_all(self.mods_dir()).map_err(|e| e.to_string())?;
        fs::create_dir_all(self.history_dir()).map_err(|e| e.to_string())?;

        // Seed Carnival AGENTS.md recipe instructions on disk (~/.config/ai-carnival/AGENTS.md)
        let agents_md_path = self.root.join("AGENTS.md");
        if !agents_md_path.exists() {
            let agents_doc = concat!(
                "# Carnival Studio Recipes (AGENTS.md)\n\n",
                "Each directory in `layouts/<id>/` contains:\n",
                "- `manifest.json`: `{ id, name, base_layout, author, stars, trending, source_url, accent_hex, description }`\n",
                "- `style.css`: Custom CSS layered over the Carnival smoked jet-black glass shell.\n",
                "- `RECIPE.md`: Human-readable installation & customization recipe.\n\n",
                "Editing `manifest.json` or `style.css` triggers automatic hot-reload in Agent Manager.\n"
            );
            let _ = fs::write(agents_md_path, agents_doc);
        }

        // Remove old placeholder recipes so Gallery shows only the curated Carnival layout catalog + custom user recipes
        for old_id in OBSOLETE_LAYOUT_IDS {
            let old_dir = self.layouts_dir().join(old_id);
            if old_dir.exists() {
                let _ = fs::remove_dir_all(old_dir);
            }
        }

        // Curated Carnival community & studio layouts, ordered by Trending stars
        let curated_layouts: [(&str, &str, &str, u32, bool, &str, &str); 9] = [
            (
                "pixel-hq",
                "Pixel HQ",
                "bnhan2710",
                60,
                true,
                "https://github.com/bnhan2710/pixel-hq/tree/main",
                "Your coding agents as a small engineering company: dark office pixel pets at desks with MacBooks, job-title nameplates, and split live panes.",
            ),
            (
                "cockpit",
                "Cockpit",
                "carnival",
                57,
                true,
                "",
                "High-density telemetry & mission control cockpit with clean status strips, agent panes, and keyboard navigation.",
            ),
            (
                "world-of-codecraft",
                "World of Codecraft",
                "kevinpardo",
                28,
                true,
                "https://github.com/kevinpardo/world-of-codecraft/tree/main",
                "High-fantasy isometric pixel village above a live agent terminal — spaces are heraldic districts and every agent is a roaming hero.",
            ),
            (
                "kanso",
                "Kanso",
                "jlopezlira",
                27,
                true,
                "",
                "Minimalist monochrome side-by-side agent workspace. The one waiting on you comes first; less babysitting, more flow.",
            ),
            (
                "omarchy",
                "Omarchy",
                "shafiqimtiaz",
                19,
                false,
                "",
                "Tiling flat-tile workspace inspired by Omarchy: crisp bordered panes, left spaces sidebar, live quota header, and right Git Changes inspector.",
            ),
            (
                "islands",
                "Islands",
                "CodeyPacker",
                14,
                false,
                "",
                "Every panel is its own floating rounded card with breathing room around every surface — monochrome smoked glass Islands style.",
            ),
            (
                "brutal-metrics",
                "Brutal Metrics",
                "coldtatooine",
                10,
                false,
                "https://github.com/coldtatooine/brutal-metrics/tree/main",
                "Brutalist telemetry rail showing live tokens, cache hits, and context % beside each agent; turns warm rust-cream when an agent needs input.",
            ),
            (
                "middle-earth",
                "Middle-earth",
                "joaovitorwoliveira",
                7,
                false,
                "",
                "Your agents become the Fellowship: animated pixel sprites in the Shire/Mordor synced beside live agent controls.",
            ),
            (
                "officetime",
                "Officetime",
                "Skaanning",
                4,
                false,
                "https://github.com/Skaanning/officetime/tree/master",
                "Top-down pixel office floorplan: busy coworkers type at desks, blocked agents raise a hand, finished agents celebrate; click any coworker to inspect.",
            ),
        ];

        for (id, name, author, stars, trending, source_url, desc) in curated_layouts {
            let dir = self.layouts_dir().join(id);
            if !dir.join("manifest.json").exists() {
                let rec = RecipeRecord {
                    id: id.to_string(),
                    name: name.to_string(),
                    kind: RecipeKind::Layout,
                    builtin: true,
                    base_layout: id.to_string(),
                    description: desc.to_string(),
                    filter_status: None,
                    accent_hex: "#ede6d6".to_string(),
                    author: author.to_string(),
                    stars,
                    trending,
                    source_url: source_url.to_string(),
                    css: String::new(),
                    recipe_md: format!(
                        "# {name}\n\nBy **{author}** · ★ {stars}\nSource: {source_url}\n\n{desc}\n\n> Prompt: `Read ~/.config/ai-carnival/AGENTS.md, then customize {name}`\n"
                    ),
                    path: dir.display().to_string(),
                    updated_at_ms: now_ms(),
                };
                self.write_recipe_dir(&dir, &rec)?;
            }
        }

        let builtin_mods = [
            (
                "mod-telemetry",
                "Token & Cache Meter",
                "Live Prefix-Cache hit rate, SkillTree pruning count, Cordis PID/memory, and active agent summary.",
            ),
            (
                "mod-git-diff",
                "Git & Diff Inspector",
                "Live git status and unified diff viewer for the focused agent workspace.",
            ),
            (
                "mod-agenttrail",
                "AgentTrail Map",
                "Chronological tool, approval, and status timeline across focused agents.",
            ),
            (
                "mod-shortcut-hud",
                "Shortcut & Prefix HUD",
                "Interactive Ctrl+B prefix bar and ⌘⇧A / ⌘⇧L keyboard shortcut reference.",
            ),
            (
                "mod-broadcast",
                "Quick Broadcast Bar",
                "Send a single coordinated directive across all running agents simultaneously.",
            ),
        ];

        for (id, name, desc) in builtin_mods {
            let dir = self.mods_dir().join(id);
            if !dir.join("manifest.json").exists() {
                let rec = RecipeRecord {
                    id: id.to_string(),
                    name: name.to_string(),
                    kind: RecipeKind::Mod,
                    builtin: true,
                    base_layout: "panel".to_string(),
                    description: desc.to_string(),
                    filter_status: None,
                    accent_hex: "#ede6d6".to_string(),
                    author: "Carnival".to_string(),
                    stars: 0,
                    trending: false,
                    source_url: String::new(),
                    css: String::new(),
                    recipe_md: format!("# {name}\n\n{desc}\n"),
                    path: dir.display().to_string(),
                    updated_at_ms: now_ms(),
                };
                self.write_recipe_dir(&dir, &rec)?;
            }
        }

        // Migrate stale selection if pointing to removed placeholder layout
        let mut sel = if self.active_file().exists() {
            self.load_selection()
        } else {
            ActiveStudioSelection::default()
        };
        if OBSOLETE_LAYOUT_IDS.contains(&sel.active_layout.as_str())
            || !self.layouts_dir().join(&sel.active_layout).join("manifest.json").exists()
        {
            sel.active_layout = "kanso".to_string();
        }
        self.save_selection(&sel)?;

        Ok(())
    }

    fn write_recipe_dir(&self, dir: &Path, rec: &RecipeRecord) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        fs::write(
            dir.join("manifest.json"),
            serde_json::to_string_pretty(rec).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::write(dir.join("style.css"), &rec.css).map_err(|e| e.to_string())?;
        fs::write(dir.join("RECIPE.md"), &rec.recipe_md).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_selection(&self) -> ActiveStudioSelection {
        fs::read_to_string(self.active_file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_selection(&self, sel: &ActiveStudioSelection) -> Result<(), String> {
        fs::write(
            self.active_file(),
            serde_json::to_string_pretty(sel).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    fn scan_recipes(&self, dir: &Path, default_kind: RecipeKind) -> Vec<RecipeRecord> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(dir) else {
            return out;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }
            let manifest_path = p.join("manifest.json");
            if let Ok(raw) = fs::read_to_string(&manifest_path) {
                if let Ok(mut rec) = serde_json::from_str::<RecipeRecord>(&raw) {
                    rec.kind = default_kind.clone();
                    rec.path = p.display().to_string();
                    if let Ok(disk_css) = fs::read_to_string(p.join("style.css")) {
                        if !disk_css.trim().is_empty() || !rec.css.is_empty() {
                            rec.css = disk_css;
                        }
                    }
                    if let Ok(disk_md) = fs::read_to_string(p.join("RECIPE.md")) {
                        rec.recipe_md = disk_md;
                    }
                    out.push(rec);
                }
            }
        }
        // Sort by highest stars descending (Trending first), then custom/alphabetical
        out.sort_by(|a, b| b.stars.cmp(&a.stars).then_with(|| a.name.cmp(&b.name)));
        out
    }

    fn latest_snapshot(&self) -> Option<SnapshotEntry> {
        let mut snaps: Vec<(PathBuf, SnapshotEntry)> = Vec::new();
        let entries = fs::read_dir(self.history_dir()).ok()?;
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("json") {
                if let Ok(raw) = fs::read_to_string(&p) {
                    if let Ok(snap) = serde_json::from_str::<SnapshotEntry>(&raw) {
                        snaps.push((p, snap));
                    }
                }
            }
        }
        snaps.sort_by(|a, b| b.1.created_at_ms.cmp(&a.1.created_at_ms));
        snaps.into_iter().next().map(|(_, s)| s)
    }

    fn write_snapshot(&self, note: &str, created_recipe_dir: Option<String>) -> Result<SnapshotEntry, String> {
        let snap = SnapshotEntry {
            snapshot_id: format!("snap-{}", &Uuid::new_v4().to_string()[..8]),
            created_at_ms: now_ms(),
            note: note.to_string(),
            selection: self.load_selection(),
            created_recipe_dir,
        };
        let path = self.history_dir().join(format!("{}-{}.json", snap.created_at_ms, snap.snapshot_id));
        fs::write(
            path,
            serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(snap)
    }

    pub fn list_state(&self) -> StudioRecipesState {
        let _guard = self.lock.lock();
        let sel = self.load_selection();
        let layouts = self.scan_recipes(&self.layouts_dir(), RecipeKind::Layout);
        let mods = self.scan_recipes(&self.mods_dir(), RecipeKind::Mod);
        let last_snap = self.latest_snapshot();

        let mut hasher = DefaultHasher::new();
        sel.active_layout.hash(&mut hasher);
        sel.enabled_mods.hash(&mut hasher);
        for r in layouts.iter().chain(mods.iter()) {
            r.id.hash(&mut hasher);
            r.css.hash(&mut hasher);
            r.base_layout.hash(&mut hasher);
            r.filter_status.hash(&mut hasher);
        }

        StudioRecipesState {
            config_root: self.root.display().to_string(),
            active_layout: sel.active_layout,
            enabled_mods: sel.enabled_mods,
            layouts,
            mods,
            has_rollback: last_snap.is_some(),
            last_snapshot_note: last_snap.map(|s| s.note),
            revision: hasher.finish(),
        }
    }

    pub fn set_active(&self, layout_id: Option<String>, enabled_mods: Option<Vec<String>>) -> Result<StudioRecipesState, String> {
        {
            let _guard = self.lock.lock().map_err(|e| e.to_string())?;
            let mut sel = self.load_selection();
            if let Some(lid) = layout_id {
                if !lid.trim().is_empty() {
                    sel.active_layout = lid.trim().to_string();
                }
            }
            if let Some(mods) = enabled_mods {
                sel.enabled_mods = mods;
            }
            sel.updated_at_ms = now_ms();
            self.save_selection(&sel)?;
        }
        Ok(self.list_state())
    }

    pub fn save_recipe(&self, req: SaveRecipeRequest) -> Result<(RecipeRecord, StudioRecipesState), String> {
        let slug = req
            .id
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(slugify)
            .unwrap_or_else(|| format!("custom-{}", slugify(&req.name)));

        let rec = {
            let _guard = self.lock.lock().map_err(|e| e.to_string())?;
            let dir = match req.kind {
                RecipeKind::Layout => self.layouts_dir().join(&slug),
                RecipeKind::Mod => self.mods_dir().join(&slug),
            };
            let _ = self.write_snapshot(
                &format!("Before saving {}", req.name),
                Some(dir.display().to_string()),
            );

            let rec = RecipeRecord {
                id: slug.clone(),
                name: req.name.clone(),
                kind: req.kind.clone(),
                builtin: false,
                base_layout: req.base_layout.unwrap_or_else(|| "kanso".to_string()),
                description: req
                    .description
                    .unwrap_or_else(|| format!("Custom Studio recipe crafted for {}", req.name)),
                filter_status: req.filter_status,
                accent_hex: req.accent_hex.unwrap_or_else(|| "#ede6d6".to_string()),
                author: "You".to_string(),
                stars: 0,
                trending: false,
                source_url: String::new(),
                css: req.css.unwrap_or_default(),
                recipe_md: req.recipe_md.unwrap_or_else(|| format!("# {}\n\nCustom Carnival Studio recipe.", req.name)),
                path: dir.display().to_string(),
                updated_at_ms: now_ms(),
            };
            self.write_recipe_dir(&dir, &rec)?;

            if req.activate.unwrap_or(true) {
                let mut sel = self.load_selection();
                match rec.kind {
                    RecipeKind::Layout => sel.active_layout = rec.id.clone(),
                    RecipeKind::Mod => {
                        if !sel.enabled_mods.contains(&rec.id) {
                            sel.enabled_mods.push(rec.id.clone());
                        }
                    }
                }
                sel.updated_at_ms = now_ms();
                self.save_selection(&sel)?;
            }
            rec
        };

        Ok((rec, self.list_state()))
    }

    /// "Shape This UI" prompt-to-recipe synthesizer: generates a real disk-backed recipe + snapshots for rollback.
    pub fn shape_ui(&self, prompt: &str) -> Result<(RecipeRecord, SnapshotEntry, StudioRecipesState), String> {
        let lower = prompt.trim().to_lowercase();
        if lower.is_empty() {
            return Err("Prompt cannot be empty".to_string());
        }

        // Support pasting Carnival layout URLs or keywords directly!
        let base_layout = if lower.contains("pixel-hq") || lower.contains("pixel hq") || lower.contains("pet") {
            "pixel-hq"
        } else if lower.contains("codecraft") || lower.contains("fantasy") || lower.contains("warcraft") {
            "world-of-codecraft"
        } else if lower.contains("officetime") || lower.contains("office") || lower.contains("floorplan") {
            "officetime"
        } else if lower.contains("middle-earth") || lower.contains("middle earth") || lower.contains("fellowship") || lower.contains("shire") {
            "middle-earth"
        } else if lower.contains("brutal") || lower.contains("metrics") {
            "brutal-metrics"
        } else if lower.contains("omarchy") || lower.contains("tile") {
            "omarchy"
        } else if lower.contains("islands") || lower.contains("floating card") {
            "islands"
        } else if lower.contains("cockpit") {
            "cockpit"
        } else {
            "kanso"
        };

        let filter_status = if lower.contains("blocked") || lower.contains("needs you") || lower.contains("approval") || lower.contains("urgent") {
            Some("needs_you".to_string())
        } else if lower.contains("only working") || lower.contains("active only") {
            Some("working".to_string())
        } else if lower.contains("done only") {
            Some("done".to_string())
        } else {
            None
        };

        let (theme_title, theme_css) = if lower.contains("warm") || lower.contains("cream") || lower.contains("editorial") {
            (
                "Warm Smoked Studio",
                ".am-shell { background: rgba(16, 13, 11, 0.88) !important; }\n.surface-card { border-color: rgba(237, 230, 214, 0.22) !important; }\n",
            )
        } else if lower.contains("jet") || lower.contains("black") || lower.contains("minimal") || lower.contains("kanso") {
            (
                "Jet Black Kanso",
                ".am-shell { background: rgba(4, 4, 6, 0.92) !important; }\n",
            )
        } else {
            (
                "Shaped Smoked Studio",
                ".surface-card { border-color: rgba(237, 230, 214, 0.24) !important; }\n",
            )
        };

        let slug = format!("custom-{}", slugify(&format!("{theme_title}-{}", &Uuid::new_v4().to_string()[..4])));
        let dir = self.layouts_dir().join(&slug);

        let (snap, rec) = {
            let _guard = self.lock.lock().map_err(|e| e.to_string())?;
            let snap = self.write_snapshot(
                &format!("Shape UI: {}", prompt.trim()),
                Some(dir.display().to_string()),
            )?;

            let rec = RecipeRecord {
                id: slug.clone(),
                name: theme_title.to_string(),
                kind: RecipeKind::Layout,
                builtin: false,
                base_layout: base_layout.to_string(),
                description: format!("Shaped from prompt: \"{}\"", prompt.trim()),
                filter_status: filter_status.clone(),
                accent_hex: "#ede6d6".to_string(),
                author: "You".to_string(),
                stars: 0,
                trending: false,
                source_url: String::new(),
                css: theme_css.to_string(),
                recipe_md: format!(
                    "# {theme_title}\n\nGenerated via Carnival **Shape This UI** prompt:\n> {}\n\n- Base geometry: `{base_layout}`\n- Filter: `{}`\n",
                    prompt.trim(),
                    filter_status.as_deref().unwrap_or("none"),
                ),
                path: dir.display().to_string(),
                updated_at_ms: now_ms(),
            };
            self.write_recipe_dir(&dir, &rec)?;

            let mut sel = self.load_selection();
            sel.active_layout = rec.id.clone();
            if lower.contains("diff") && !sel.enabled_mods.contains(&"mod-git-diff".to_string()) {
                sel.enabled_mods.push("mod-git-diff".to_string());
            }
            if (lower.contains("trail") || lower.contains("history"))
                && !sel.enabled_mods.contains(&"mod-agenttrail".to_string())
            {
                sel.enabled_mods.push("mod-agenttrail".to_string());
            }
            sel.updated_at_ms = now_ms();
            self.save_selection(&sel)?;
            (snap, rec)
        };

        Ok((rec, snap, self.list_state()))
    }

    pub fn rollback(&self) -> Result<(SnapshotEntry, StudioRecipesState), String> {
        let snap = {
            let _guard = self.lock.lock().map_err(|e| e.to_string())?;
            let mut snaps: Vec<(PathBuf, SnapshotEntry)> = Vec::new();
            if let Ok(entries) = fs::read_dir(self.history_dir()) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.extension().and_then(|x| x.to_str()) == Some("json") {
                        if let Ok(raw) = fs::read_to_string(&p) {
                            if let Ok(snap) = serde_json::from_str::<SnapshotEntry>(&raw) {
                                snaps.push((p, snap));
                            }
                        }
                    }
                }
            }
            snaps.sort_by(|a, b| b.1.created_at_ms.cmp(&a.1.created_at_ms));
            let (snap_path, snap) = snaps
                .into_iter()
                .next()
                .ok_or_else(|| "No UI snapshot available to roll back".to_string())?;
            self.save_selection(&snap.selection)?;
            let _ = fs::remove_file(snap_path);
            snap
        };

        Ok((snap, self.list_state()))
    }
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "recipe".to_string()
    } else {
        trimmed.chars().take(42).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_curated_carnival_gallery_and_shapes_with_rollback() {
        let tmp = tempfile::tempdir().unwrap();
        let recipes = StudioRecipes::new(tmp.path());

        let initial = recipes.list_state();
        assert_eq!(initial.active_layout, "kanso");
        assert_eq!(initial.layouts.len(), 9);
        assert_eq!(initial.layouts[0].id, "pixel-hq");
        assert_eq!(initial.layouts[0].stars, 60);
        assert!(initial.layouts[0].trending);
        assert_eq!(initial.mods.len(), 5);

        // Edit custom recipe CSS directly on disk -> hot reload bumps revision
        let target_css = Path::new(&initial.layouts[0].path).join("style.css");
        fs::write(&target_css, "/* edited live */").unwrap();
        let reloaded = recipes.list_state();
        assert_ne!(initial.revision, reloaded.revision);

        // Shape UI from natural language prompt (including Pixel HQ + blocked filter)
        let (shaped, _snap, after_shape) = recipes
            .shape_ui("Use pixel hq layout showing only blocked agents and git diffs")
            .unwrap();
        assert_eq!(after_shape.active_layout, shaped.id);
        assert_eq!(shaped.base_layout, "pixel-hq");
        assert_eq!(shaped.filter_status.as_deref(), Some("needs_you"));
        assert!(after_shape.has_rollback);
        assert!(after_shape.enabled_mods.contains(&"mod-git-diff".to_string()));

        // Rollback restores previous layout ("kanso")
        let (_restored_snap, after_rb) = recipes.rollback().unwrap();
        assert_eq!(after_rb.active_layout, "kanso");
    }
}
