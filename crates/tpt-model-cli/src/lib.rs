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

    /// Unified-diff patch that deletes the planned lines, one hunk per removal.
    pub fn patch(&self, files: &[SourceFile]) -> String {
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
            let lines: Vec<&str> = text.lines().collect();
            let _ = writeln!(out, "--- a/{display}");
            let _ = writeln!(out, "+++ b/{display}");
            let mut removed_before = 0usize;
            for r in removals {
                let len = r.end - r.start + 1;
                let _ = writeln!(
                    out,
                    "@@ -{},{} +{},0 @@",
                    r.start,
                    len,
                    r.start - removed_before
                );
                for line in &lines[r.start - 1..r.end] {
                    let _ = writeln!(out, "-{line}");
                }
                removed_before += len;
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
        collect(&parsed.items, &file.path, &mut scan);
    }
    Ok(scan)
}

fn collect(items: &[Item], file: &PathBuf, scan: &mut Scan) {
    for item in items {
        if let Item::Mod(m) = item {
            // Inline modules are transparent: their items are the units we track.
            if let Some((_, inner)) = &m.content {
                collect(inner, file, scan);
            }
            continue;
        }
        let span = item.span();
        let (label, declares, model, migration) = describe(item);
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
}

type Described = (String, Option<String>, Option<Model>, Option<Migration>);

fn describe(item: &Item) -> Described {
    match item {
        Item::Struct(s) => {
            let ident = s.ident.to_string();
            let model = has_derive(&s.attrs, "Model").then(|| {
                let (name, version) = tpt_version(&s.attrs, &ident);
                Model {
                    ident: ident.clone(),
                    name,
                    version,
                }
            });
            (format!("struct {ident}"), Some(ident), model, None)
        }
        Item::Enum(e) => {
            let ident = e.ident.to_string();
            let model = has_derive(&e.attrs, "Model").then(|| {
                let (name, version) = tpt_version(&e.attrs, &ident);
                Model {
                    ident: ident.clone(),
                    name,
                    version,
                }
            });
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
    }
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
    let mut removed_spans: BTreeSet<(PathBuf, usize)> = BTreeSet::new();

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
            if decl.model.is_none() && !removed_spans.contains(&(decl.file.clone(), decl.start)) {
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
        if removed_spans.contains(&(s.file.clone(), s.start)) {
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

    plan.removals
        .sort_by(|a, b| a.file.cmp(&b.file).then(a.start.cmp(&b.start)));
    Ok(plan)
}

/// Returns the files with the planned line ranges removed.
pub fn apply(plan: &Plan, files: &[SourceFile]) -> Vec<SourceFile> {
    files
        .iter()
        .map(|f| {
            let mut lines: Vec<&str> = f.text.lines().collect();
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
            let mut text = lines.join("\n");
            if f.text.ends_with('\n') {
                text.push('\n');
            }
            SourceFile {
                path: f.path.clone(),
                text,
            }
        })
        .collect()
}

// ---- helpers ------------------------------------------------------------------

fn has_derive(attrs: &[Attribute], derive: &str) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        attr.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
            .map(|paths| {
                paths
                    .iter()
                    .any(|p| p.segments.last().is_some_and(|s| s.ident == derive))
            })
            .unwrap_or(false)
    })
}

/// Reads `#[tpt(name = "...", version = N)]`, with the same defaults as the derive.
fn tpt_version(attrs: &[Attribute], ident: &str) -> (String, u32) {
    let mut name = None;
    let mut version = 1u32;
    for attr in attrs.iter().filter(|a| a.path().is_ident("tpt")) {
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("version") {
                let lit: syn::LitInt = meta.value()?.parse()?;
                version = lit.base10_parse()?;
            } else if meta.path.is_ident("name") {
                let lit: syn::LitStr = meta.value()?.parse()?;
                name = Some(lit.value());
            } else {
                let _ = meta.value().and_then(|v| v.parse::<syn::Expr>());
            }
            Ok(())
        });
    }
    (name.unwrap_or_else(|| default_name(ident)), version)
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
    removed_spans: &mut BTreeSet<(PathBuf, usize)>,
    what: String,
    span: &Span,
    ident: &str,
) {
    doomed.insert(ident.to_string());
    removed_spans.insert((span.file.clone(), span.start));
    plan.removals.push(Removal {
        what,
        file: span.file.clone(),
        start: span.start,
        end: span.end,
    });
}
