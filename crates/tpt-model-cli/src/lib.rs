//! Squash/compact analysis for `tpt-model` projects.
//!
//! Given a minimum supported version `N`, the analysis finds:
//!
//! * versioned structs and enums (`#[derive(Model)]`) whose version is below `N`;
//! * `impl Migration` blocks whose source version is below `N`, together with the
//!   marker type they are implemented on, when that type is declared in the scanned sources.
//!
//! It refuses, with the reason and location for each conflict, when code that stays still
//! refers to something that would be deleted. Otherwise it produces a plan, a patch in
//! unified-diff form, and on request applies the removals.
//!
//! Only top-level items (including those in inline modules) are removable. Items are removed by
//! line span, so attributes and doc comments on a removed item go with it.

pub mod driver;
pub mod inspect;

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::path::PathBuf;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Item, Token};

/// A source file to scan, with its contents.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub text: String,
}

/// A versioned model found in source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub ident: String,
    pub name: String,
    pub version: u32,
}

/// A migration found in source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    /// The type the trait is implemented on.
    pub marker: String,
    pub from: String,
    pub to: String,
}

/// A top-level item, with its span and the identifiers it mentions.
#[derive(Debug, Clone)]
struct Span {
    label: String,
    file: PathBuf,
    start: usize,
    end: usize,
    idents: BTreeSet<String>,
    /// The item's declared name, for struct and enum items.
    declares: Option<String>,
    model: Option<Model>,
    migration: Option<Migration>,
}

/// Everything found in the scanned sources.
#[derive(Debug, Default)]
pub struct Scan {
    items: Vec<Span>,
}

/// Why the squash cannot proceed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The requested minimum is newer than the newest version of some type.
    MinAboveLatest { name: String, min: u32, latest: u32 },
    /// Code that stays refers to something that would be deleted.
    StillReferenced {
        target: String,
        by: String,
        file: PathBuf,
        line: usize,
    },
    /// A removal deletes whole lines, and a kept item shares one of them.
    SharesLines {
        target: String,
        by: String,
        file: PathBuf,
        line: usize,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::MinAboveLatest { name, min, latest } => write!(
                f,
                "refusing: minimum supported v{min} is newer than the latest version v{latest} of {name}"
            ),
            Refusal::StillReferenced { target, by, file, line } => write!(
                f,
                "refusing: `{target}` would be deleted but is still used by {by} at {}:{line}",
                file.display()
            ),
            Refusal::SharesLines {
                target,
                by,
                file,
                line,
            } => write!(
                f,
                "refusing: removing `{target}` deletes whole lines, and {by} shares line {line} of {}",
                file.display()
            ),
        }
    }
}

/// A line range to delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    pub what: String,
    pub file: PathBuf,
    pub start: usize,
    pub end: usize,
}

/// The result of a successful analysis.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub min_supported: u32,
    pub removals: Vec<Removal>,
    pub kept: Vec<String>,
    pub warnings: Vec<String>,
}

impl Plan {
    /// Human-readable plan.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "squash plan (minimum supported: v{})",
            self.min_supported
        );
        if self.removals.is_empty() {
            out.push_str("nothing to remove\n");
        }
        for r in &self.removals {
            let _ = writeln!(
                out,
                "  delete {} ({}:{}-{})",
                r.what,
                r.file.display(),
                r.start,
                r.end
            );
        }
        let files: BTreeSet<String> = self
            .removals
            .iter()
            .map(|r| r.file.display().to_string())
            .collect();
        if !files.is_empty() {
            let _ = writeln!(
                out,
                "affected files: {}",
                files.into_iter().collect::<Vec<_>>().join(", ")
            );
        }
        for k in &self.kept {
            let _ = writeln!(out, "  keep {k}");
        }
        for w in &self.warnings {
            let _ = writeln!(out, "warning: {w}");
        }
        out
    }

    /// The plan as one line of JSON:
    /// `{"min_supported":N,"removals":[{"what","file","start","end"}],"kept":[],"warnings":[]}`.
    pub fn to_json(&self) -> String {
        let removals: Vec<String> = self
            .removals
            .iter()
            .map(|r| {
                format!(
                    "{{\"what\":{},\"file\":{},\"start\":{},\"end\":{}}}",
                    json_str(&r.what),
                    json_str(&r.file.display().to_string().replace('\\', "/")),
                    r.start,
                    r.end
                )
            })
            .collect();
        let kept: Vec<String> = self.kept.iter().map(|k| json_str(k)).collect();
        let warnings: Vec<String> = self.warnings.iter().map(|w| json_str(w)).collect();
        format!(
            "{{\"min_supported\":{},\"removals\":[{}],\"kept\":[{}],\"warnings\":[{}]}}\n",
            self.min_supported,
            removals.join(","),
            kept.join(","),
            warnings.join(",")
        )
    }

    /// Unified-diff patch that deletes the planned lines, with three lines of context.
    ///
    /// Removals whose context overlaps share one hunk, as `diff -U3` would produce. Hunks
    /// without context are not used, because `git apply` anchors those at the top of the file.
    pub fn patch(&self, files: &[SourceFile]) -> String {
        const CONTEXT: usize = 3;
        let mut out = String::new();
        let mut by_file: BTreeMap<&PathBuf, Vec<&Removal>> = BTreeMap::new();
        for r in &self.removals {
            by_file.entry(&r.file).or_default().push(r);
        }
        for (path, mut removals) in by_file {
            removals.sort_by_key(|r| r.start);
            let display = path.display().to_string().replace('\\', "/");
            let text = files
                .iter()
                .find(|f| &f.path == path)
                .map(|f| f.text.as_str())
                .unwrap_or("");
            // Keep each line's terminator so CRLF files produce matching lines.
            let lines: Vec<&str> = text.split_inclusive('\n').collect();
            // One flag per 1-based line number.
            let mut removed = vec![false; lines.len() + 1];
            for r in &removals {
                for flag in &mut removed[r.start..=r.end] {
                    *flag = true;
                }
            }

            // Old-side line ranges (1-based, inclusive) that make up the hunks.
            let mut hunks: Vec<(usize, usize)> = Vec::new();
            for r in &removals {
                let lo = r.start.saturating_sub(CONTEXT).max(1);
                let hi = (r.end + CONTEXT).min(lines.len());
                match hunks.last_mut() {
                    Some(last) if lo <= last.1 + 1 => last.1 = last.1.max(hi),
                    _ => hunks.push((lo, hi)),
                }
            }

            let _ = writeln!(out, "--- a/{display}");
            let _ = writeln!(out, "+++ b/{display}");
            for (lo, hi) in hunks {
                let old_len = hi - lo + 1;
                let removed_here = (lo..=hi).filter(|&l| removed[l]).count();
                let new_len = old_len - removed_here;
                let removed_before = (1..lo).filter(|&l| removed[l]).count();
                let mut new_start = lo - removed_before;
                if new_len == 0 {
                    // With no lines on the new side, the header names the line before.
                    new_start -= 1;
                }
                let _ = writeln!(out, "@@ -{lo},{old_len} +{new_start},{new_len} @@");
                for l in lo..=hi {
                    let line = lines[l - 1];
                    let marker = if removed[l] { '-' } else { ' ' };
                    let content = line.strip_suffix('\n').unwrap_or(line);
                    let _ = writeln!(out, "{marker}{content}");
                    if !line.ends_with('\n') {
                        out.push_str("\\ No newline at end of file\n");
                    }
                }
            }
        }
        out
    }
}

/// Parses every file and records its top-level items.
pub fn scan(files: &[SourceFile]) -> Result<Scan, String> {
    let mut scan = Scan::default();
    for file in files {
        let parsed =
            syn::parse_file(&file.text).map_err(|e| format!("{}: {e}", file.path.display()))?;
        collect(&parsed.items, &file.path, &mut scan)?;
    }
    Ok(scan)
}

fn collect(items: &[Item], file: &PathBuf, scan: &mut Scan) -> Result<(), String> {
    for item in items {
        if let Item::Mod(m) = item {
            // Inline modules are transparent: their items are the units we track.
            if let Some((_, inner)) = &m.content {
                collect(inner, file, scan)?;
            }
            continue;
        }
        let span = item.span();
        let (label, declares, model, migration) =
            describe(item).map_err(|e| format!("{}: {e}", file.display()))?;
        let mut idents = BTreeSet::new();
        idents_into(item.to_token_stream(), &mut idents);
        if let Some(own) = &declares {
            // A declaration does not refer to itself.
            idents.remove(own);
        }
        scan.items.push(Span {
            label,
            file: file.clone(),
            start: span.start().line,
            end: span.end().line,
            idents,
            declares,
            model,
            migration,
        });
    }
    Ok(())
}

type Described = (String, Option<String>, Option<Model>, Option<Migration>);

fn describe(item: &Item) -> Result<Described, String> {
    let described = match item {
        Item::Struct(s) => {
            let ident = s.ident.to_string();
            let model = if has_derive(&s.attrs, "Model")? {
                let (name, version) = tpt_version(&s.attrs, &ident)?;
                Some(Model {
                    ident: ident.clone(),
                    name,
                    version,
                })
            } else {
                None
            };
            (format!("struct {ident}"), Some(ident), model, None)
        }
        Item::Enum(e) => {
            let ident = e.ident.to_string();
            let model = if has_derive(&e.attrs, "Model")? {
                let (name, version) = tpt_version(&e.attrs, &ident)?;
                Some(Model {
                    ident: ident.clone(),
                    name,
                    version,
                })
            } else {
                None
            };
            (format!("enum {ident}"), Some(ident), model, None)
        }
        Item::Impl(i) if is_migration_impl(i) => {
            let marker = type_name(&i.self_ty);
            let migration = Migration {
                marker: marker.clone(),
                from: assoc_type(i, "From").unwrap_or_default(),
                to: assoc_type(i, "To").unwrap_or_default(),
            };
            (
                format!("impl Migration for {marker}"),
                None,
                None,
                Some(migration),
            )
        }
        Item::Fn(f) => (format!("fn {}", f.sig.ident), None, None, None),
        Item::Impl(i) => (format!("impl {}", type_name(&i.self_ty)), None, None, None),
        Item::Use(_) => ("use".to_string(), None, None, None),
        _ => ("item".to_string(), None, None, None),
    };
    Ok(described)
}

/// Analyses a scan for a given minimum supported version.
pub fn analyze(scan: &Scan, min: u32) -> Result<Plan, Refusal> {
    let models: Vec<&Model> = scan.items.iter().filter_map(|s| s.model.as_ref()).collect();

    // The minimum must not be newer than the newest version of any type.
    let mut latest: BTreeMap<&str, u32> = BTreeMap::new();
    for m in &models {
        let e = latest.entry(m.name.as_str()).or_insert(0);
        *e = (*e).max(m.version);
    }
    for (name, &top) in &latest {
        if min > top {
            return Err(Refusal::MinAboveLatest {
                name: (*name).to_string(),
                min,
                latest: top,
            });
        }
    }

    let version_of: BTreeMap<&str, u32> = models
        .iter()
        .map(|m| (m.ident.as_str(), m.version))
        .collect();
    let mut plan = Plan {
        min_supported: min,
        ..Plan::default()
    };
    let mut doomed: BTreeSet<String> = BTreeSet::new();
    let mut removed_spans: BTreeSet<(PathBuf, usize, String)> = BTreeSet::new();

    for s in &scan.items {
        if let Some(m) = &s.model {
            if m.version < min {
                remove(
                    &mut plan,
                    &mut doomed,
                    &mut removed_spans,
                    format!("{} ({} v{})", m.ident, m.name, m.version),
                    s,
                    &m.ident,
                );
            } else {
                plan.kept
                    .push(format!("{} ({} v{})", m.ident, m.name, m.version));
            }
        }
    }

    // A migration is dead when its source version is below the minimum.
    let mut dead_migrations: Vec<&Span> = Vec::new();
    for s in &scan.items {
        let Some(mig) = &s.migration else { continue };
        match version_of.get(mig.from.as_str()) {
            Some(&from) if from < min => {
                let to = version_of.get(mig.to.as_str()).copied().unwrap_or(0);
                remove(
                    &mut plan,
                    &mut doomed,
                    &mut removed_spans,
                    format!("impl Migration for {} (v{from} -> v{to})", mig.marker),
                    s,
                    &mig.marker,
                );
                dead_migrations.push(s);
            }
            Some(_) => plan.kept.push(format!(
                "impl Migration for {} (v{} -> v{})",
                mig.marker,
                version_of[mig.from.as_str()],
                version_of.get(mig.to.as_str()).copied().unwrap_or(0)
            )),
            None => {
                plan.warnings.push(format!(
                    "impl Migration for {} refers to `{}`, which is not a versioned model; left in place",
                    mig.marker, mig.from
                ));
                plan.kept.push(format!("impl Migration for {}", mig.marker));
            }
        }
    }

    // Marker types of dead migrations go too, when they are declared in the scanned sources.
    for m in dead_migrations {
        let marker = m
            .migration
            .as_ref()
            .map(|x| x.marker.clone())
            .unwrap_or_default();
        if let Some(decl) = scan
            .items
            .iter()
            .find(|s| s.declares.as_deref() == Some(marker.as_str()))
        {
            if decl.model.is_none()
                && !removed_spans.contains(&(decl.file.clone(), decl.start, decl.label.clone()))
            {
                remove(
                    &mut plan,
                    &mut doomed,
                    &mut removed_spans,
                    format!("marker type {marker}"),
                    decl,
                    &marker,
                );
            }
        }
    }

    // Anything that stays must not mention anything that goes.
    for s in &scan.items {
        if removed_spans.contains(&(s.file.clone(), s.start, s.label.clone())) {
            continue;
        }
        if let Some(target) = s.idents.iter().find(|i| doomed.contains(i.as_str())) {
            return Err(Refusal::StillReferenced {
                target: target.clone(),
                by: s.label.clone(),
                file: s.file.clone(),
                line: s.start,
            });
        }
    }

    // Removal works on whole lines. A kept item on a removed line would be deleted with it.
    for r in &plan.removals {
        for s in &scan.items {
            if removed_spans.contains(&(s.file.clone(), s.start, s.label.clone()))
                || s.file != r.file
            {
                continue;
            }
            if s.start <= r.end && r.start <= s.end {
                return Err(Refusal::SharesLines {
                    target: r.what.clone(),
                    by: s.label.clone(),
                    file: s.file.clone(),
                    line: s.start,
                });
            }
        }
    }

    plan.removals
        .sort_by(|a, b| a.file.cmp(&b.file).then(a.start.cmp(&b.start)));
    Ok(plan)
}

/// Returns the files with the planned line ranges removed.
pub fn apply(plan: &Plan, files: &[SourceFile]) -> Vec<SourceFile> {
    files
        .iter()
        .map(|f| {
            // Lines keep their own terminators, so CRLF and a missing final newline survive.
            let mut lines: Vec<&str> = f.text.split_inclusive('\n').collect();
            let mut ranges: Vec<(usize, usize)> = plan
                .removals
                .iter()
                .filter(|r| r.file == f.path)
                .map(|r| (r.start, r.end))
                .collect();
            ranges.sort();
            for (start, end) in ranges.into_iter().rev() {
                lines.drain(start - 1..end);
            }
            let text = lines.concat();
            SourceFile {
                path: f.path.clone(),
                text,
            }
        })
        .collect()
}

// ---- helpers ------------------------------------------------------------------

/// A JSON string literal with the escapes JSON requires.
fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Whether `#[derive(...)]` names `derive`. A malformed derive list is an error, not `false`.
fn has_derive(attrs: &[Attribute], derive: &str) -> Result<bool, String> {
    for attr in attrs.iter().filter(|a| a.path().is_ident("derive")) {
        let paths = attr
            .parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
            .map_err(|e| format!("malformed #[derive] attribute: {e}"))?;
        if paths
            .iter()
            .any(|p| p.segments.last().is_some_and(|s| s.ident == derive))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Reads `#[tpt(name = "...", version = N)]`, with the same defaults as the derive.
/// A malformed `#[tpt]` attribute is an error, because the version it sets would be lost.
fn tpt_version(attrs: &[Attribute], ident: &str) -> Result<(String, u32), String> {
    let mut name = None;
    let mut version = 1u32;
    for attr in attrs.iter().filter(|a| a.path().is_ident("tpt")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("version") {
                let lit: syn::LitInt = meta.value()?.parse()?;
                version = lit.base10_parse()?;
            } else if meta.path.is_ident("name") {
                let lit: syn::LitStr = meta.value()?.parse()?;
                name = Some(lit.value());
            } else {
                meta.value()?.parse::<syn::Expr>()?;
            }
            Ok(())
        })
        .map_err(|e| format!("malformed #[tpt] attribute on `{ident}`: {e}"))?;
    }
    Ok((name.unwrap_or_else(|| default_name(ident)), version))
}

fn default_name(ident: &str) -> String {
    if let Some(pos) = ident.rfind('V') {
        let suffix = &ident[pos + 1..];
        if pos > 0 && !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()) {
            return ident[..pos].to_string();
        }
    }
    ident.to_string()
}

fn is_migration_impl(i: &syn::ItemImpl) -> bool {
    i.trait_
        .as_ref()
        .is_some_and(|(_, path, _)| path.segments.last().is_some_and(|s| s.ident == "Migration"))
}

fn assoc_type(i: &syn::ItemImpl, wanted: &str) -> Option<String> {
    i.items.iter().find_map(|item| match item {
        syn::ImplItem::Type(t) if t.ident == wanted => Some(type_name(&t.ty)),
        _ => None,
    })
}

fn type_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn idents_into(tokens: TokenStream, out: &mut BTreeSet<String>) {
    for tt in tokens {
        match tt {
            TokenTree::Ident(i) => {
                out.insert(i.to_string());
            }
            TokenTree::Group(g) => idents_into(g.stream(), out),
            _ => {}
        }
    }
}

/// Records one removal and marks its name and span as doomed.
fn remove(
    plan: &mut Plan,
    doomed: &mut BTreeSet<String>,
    removed_spans: &mut BTreeSet<(PathBuf, usize, String)>,
    what: String,
    span: &Span,
    ident: &str,
) {
    doomed.insert(ident.to_string());
    removed_spans.insert((span.file.clone(), span.start, span.label.clone()));
    plan.removals.push(Removal {
        what,
        file: span.file.clone(),
        start: span.start,
        end: span.end,
    });
}

/// Writes Rust source for the next version of the model `name`: a copy of its latest version
/// with the version and type name bumped, and a migration stub from the latest version to the
/// new one. The text is printed for pasting; nothing is written to disk.
pub fn scaffold_migration(files: &[SourceFile], name: &str) -> Result<String, String> {
    let scanned = scan(files)?;
    let (span, model) = scanned
        .items
        .iter()
        .filter_map(|s| s.model.as_ref().filter(|m| m.name == name).map(|m| (s, m)))
        .max_by_key(|(_, m)| m.version)
        .ok_or_else(|| format!("no versioned model named `{name}` in the scanned sources"))?;
    let file = files
        .iter()
        .find(|f| f.path == span.file)
        .ok_or_else(|| format!("{} is not among the scanned sources", span.file.display()))?;
    let lines: Vec<&str> = file.text.lines().collect();
    let body = lines[span.start - 1..span.end].join("\n");

    let old = model.version;
    let new = old + 1;
    let old_ident = model.ident.clone();
    let new_ident = bump_ident(&old_ident, old, new);
    let body = replace_word(&body, &old_ident, &new_ident);
    let body = replace_version(&body, old, new)
        .ok_or_else(|| format!("could not find `version = {old}` on {old_ident}"))?;
    let marker = format!("{name}V{old}ToV{new}");

    Ok(format!(
        "// Version {new} of {name}, copied from {old_ident} (v{old}). Edit the fields.\n\
         // Fields that v{old} lacks need `default` or an Option type. Fields removed in v{new}\n\
         // belong on the v{old} struct as `removed_in = {new}` with a `migrate` function.\n\
         // Imports: tpt_model::{{Migration, Model, UirNode}}, tpt_model::evolve::{{MigrationContext, MigrationError}}\n\
         \n\
         {body}\n\
         \n\
         // Migration from v{old} to v{new}. Automatic steps already cover defaults and renames;\n\
         // write only what they cannot.\n\
         pub struct {marker};\n\
         \n\
         impl Migration for {marker} {{\n    \
         type From = {old_ident};\n    \
         type To = {new_ident};\n\
         \n    \
         fn migrate(old: &UirNode, _ctx: &MigrationContext) -> Result<UirNode, MigrationError> {{\n        \
         Ok(old.clone())\n    \
         }}\n\
         }}\n"
    ))
}

/// `CadPartV2` at version 2 becomes `CadPartV3`. Identifiers without the version suffix gain one.
fn bump_ident(ident: &str, old: u32, new: u32) -> String {
    match ident.strip_suffix(&format!("V{old}")) {
        Some(stem) if !stem.is_empty() => format!("{stem}V{new}"),
        _ => format!("{ident}V{new}"),
    }
}

/// Replaces whole-identifier occurrences of `from`.
fn replace_word(text: &str, from: &str, to: &str) -> String {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (pos, _) in text.match_indices(from) {
        let before = text[..pos].chars().next_back();
        let after = text[pos + from.len()..].chars().next();
        if before.is_some_and(is_ident) || after.is_some_and(is_ident) {
            continue;
        }
        out.push_str(&text[last..pos]);
        out.push_str(to);
        last = pos + from.len();
    }
    out.push_str(&text[last..]);
    out
}

/// Changes the first `version = {old}` to `version = {new}`. `None` if there is none.
fn replace_version(text: &str, old: u32, new: u32) -> Option<String> {
    let needle = format!("version = {old}");
    for (pos, _) in text.match_indices(&needle) {
        let after = text[pos + needle.len()..].chars().next();
        if after.is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        return Some(format!(
            "{}version = {new}{}",
            &text[..pos],
            &text[pos + needle.len()..]
        ));
    }
    None
}
