use super::CatalogSchemaEntry;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

fn format_schema_type(prop: &Value) -> String {
    if let Some(r) = prop.get("$ref").and_then(Value::as_str) {
        let name = r.rsplit('/').next().unwrap_or(r);
        return format!("`{name}`");
    }
    if let Some(all_of) = prop.get("allOf").and_then(Value::as_array) {
        if let Some(first_ref) = all_of
            .iter()
            .find_map(|v| v.get("$ref").and_then(Value::as_str))
        {
            let name = first_ref.rsplit('/').next().unwrap_or(first_ref);
            return format!("`{name}`");
        }
    }
    if let Some(any_of) = prop
        .get("anyOf")
        .or_else(|| prop.get("oneOf"))
        .and_then(Value::as_array)
    {
        let mut parts = Vec::new();
        for variant in any_of {
            if variant.get("type").and_then(Value::as_str) == Some("null") {
                parts.push("null".to_string());
            } else {
                parts.push(format_schema_type(variant));
            }
        }
        parts.dedup();
        return parts.join(" \\| ");
    }
    if let Some(enum_vals) = prop.get("enum").and_then(Value::as_array) {
        let joined: Vec<String> = enum_vals
            .iter()
            .take(8)
            .map(|v| format!("`{}`", v.as_str().unwrap_or(&v.to_string())))
            .collect();
        let suffix = if enum_vals.len() > 8 { ", ..." } else { "" };
        return format!("enum ({}{suffix})", joined.join(", "));
    }
    match prop.get("type") {
        Some(Value::String(t)) => {
            if t == "array" {
                if let Some(items) = prop.get("items") {
                    if items.is_array() {
                        return format!("`[{}; {}]`", "number", items.as_array().unwrap().len());
                    }
                    return format!("`Vec<{}>`", format_schema_type(items).trim_matches('`'));
                }
                return "`array`".to_string();
            }
            if t == "object" {
                if let Some(ap) = prop.get("additionalProperties") {
                    if ap.is_object() {
                        return format!(
                            "`Map<String, {}>`",
                            format_schema_type(ap).trim_matches('`')
                        );
                    }
                }
                return "`object`".to_string();
            }
            format!("`{t}`")
        }
        Some(Value::Array(arr)) => {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(Value::as_str)
                .map(|s| format!("`{s}`"))
                .collect();
            parts.join(" \\| ")
        }
        _ => "`any`".to_string(),
    }
}

fn format_default_val(prop: &Value) -> String {
    match prop.get("default") {
        Some(Value::Null) => "`None`".to_string(),
        Some(Value::Bool(b)) => format!("`{b}`"),
        Some(Value::Number(n)) => format!("`{n}`"),
        Some(Value::String(s)) => format!("`\"{s}\"`"),
        Some(Value::Array(a)) if a.is_empty() => "`[]`".to_string(),
        Some(Value::Object(o)) if o.is_empty() => "`{}`".to_string(),
        Some(other) => {
            let s = other.to_string();
            if s.len() <= 32 {
                format!("`{s}`")
            } else {
                "`{...}`".to_string()
            }
        }
        None => "—".to_string(),
    }
}

fn sanitize_cell(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

fn append_object_properties_table(md: &mut String, schema_obj: &Value) {
    let Some(props) = schema_obj.get("properties").and_then(Value::as_object) else {
        if let Some(one_of) = schema_obj
            .get("oneOf")
            .or_else(|| schema_obj.get("anyOf"))
            .and_then(Value::as_array)
        {
            let mut variants = Vec::new();
            for item in one_of {
                if let Some(ev) = item.get("enum").and_then(Value::as_array) {
                    for v in ev {
                        if let Some(s) = v.as_str() {
                            variants.push(format!("`\"{s}\"`"));
                        }
                    }
                } else if let Some(props) = item.get("properties").and_then(Value::as_object) {
                    for (k, v) in props {
                        if let Some(ev) = v.get("enum").and_then(Value::as_array) {
                            for tag in ev {
                                if let Some(s) = tag.as_str() {
                                    variants.push(format!("`{k} = \"{s}\"`"));
                                }
                            }
                        }
                    }
                }
            }
            if !variants.is_empty() {
                md.push_str(&format!("- **Variants:** {}\n\n", variants.join(", ")));
            }
        } else if let Some(ev) = schema_obj.get("enum").and_then(Value::as_array) {
            let variants: Vec<String> = ev
                .iter()
                .filter_map(Value::as_str)
                .map(|s| format!("`\"{s}\"`"))
                .collect();
            if !variants.is_empty() {
                md.push_str(&format!("- **Enum Values:** {}\n\n", variants.join(", ")));
            }
        }
        return;
    };

    let required_set: BTreeSet<&str> = schema_obj
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    md.push_str("| Field | Type | Required | Default | Description |\n");
    md.push_str("| :--- | :--- | :---: | :--- | :--- |\n");
    for (field_name, prop) in props {
        let ty = format_schema_type(prop);
        let req = if required_set.contains(field_name.as_str()) {
            "yes"
        } else {
            "no"
        };
        let def_str = format_default_val(prop);
        let desc = prop
            .get("description")
            .and_then(Value::as_str)
            .map(sanitize_cell)
            .unwrap_or_default();
        md.push_str(&format!(
            "| `{field_name}` | {ty} | {req} | {def_str} | {desc} |\n"
        ));
    }
    md.push('\n');
}

/// Generates `docs/MOD_SCHEMA_REFERENCE.md` directly from the auto-derived `schemars` `RootSchema`
/// definitions of all 14 SDK catalogs (`ModManifest`, `FactionCatalog`, `UnitCatalog`, etc.).
pub fn generate_mod_schema_reference_markdown(entries: &[CatalogSchemaEntry]) -> String {
    let mut md = String::with_capacity(64 * 1024);
    md.push_str(
        "# Rust 3D RTS Engine — Auto-Derived Mod Schema Reference (`MOD_SCHEMA_REFERENCE.md`)\n\n",
    );
    md.push_str(
        "> **100% Auto-Derived from `rts-sdk` Rust types via `schemars::schema_for!(...)`.**\n",
    );
    md.push_str("> Regenerate schemas, `taplo.toml`, and this document with:\n");
    md.push_str("> `cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- gen-docs --mod-path rust-ra2`\n\n");

    md.push_str("## 1. Catalog & JSON-Schema Index\n\n");
    md.push_str("| Catalog | SDK Root Type | JSON Schema File | Taplo Glob Patterns | Summary |\n");
    md.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
    for entry in entries {
        let globs = entry
            .taplo_globs
            .iter()
            .map(|g| format!("`{g}`"))
            .collect::<Vec<_>>()
            .join(", ");
        md.push_str(&format!(
            "| **`{}`** | `{}` | `schemas/{}` | {} | {} |\n",
            entry.slug, entry.rust_type, entry.schema_filename, globs, entry.description
        ));
    }
    md.push('\n');

    md.push_str("## 2. Root Catalog Schemas\n\n");
    let mut shared_defs: BTreeMap<String, Value> = BTreeMap::new();

    for entry in entries {
        md.push_str(&format!(
            "### 2.{} `{}` (`schemas/{}` — `{}`)\n\n",
            entries
                .iter()
                .position(|e| e.slug == entry.slug)
                .unwrap_or(0)
                + 1,
            entry.slug,
            entry.schema_filename,
            entry.rust_type
        ));
        md.push_str(&format!("{}\n\n", entry.description));
        append_object_properties_table(&mut md, &entry.json_value);

        if let Some(defs) = entry
            .json_value
            .get("definitions")
            .or_else(|| entry.json_value.get("$defs"))
            .and_then(Value::as_object)
        {
            for (def_name, def_val) in defs {
                shared_defs
                    .entry(def_name.clone())
                    .or_insert_with(|| def_val.clone());
            }
        }
    }

    md.push_str("## 3. Shared SDK Struct & Enum Definitions\n\n");
    for (def_name, def_schema) in &shared_defs {
        md.push_str(&format!("### `{def_name}`\n\n"));
        if let Some(desc) = def_schema.get("description").and_then(Value::as_str) {
            let clean = sanitize_cell(desc);
            if !clean.is_empty() {
                md.push_str(&format!("{clean}\n\n"));
            }
        }
        append_object_properties_table(&mut md, def_schema);
    }

    md
}

/// Writes `MOD_SCHEMA_REFERENCE.md` to disk, creating parent directories if needed.
pub fn write_mod_schema_reference_markdown(
    output_path: &Path,
    entries: &[CatalogSchemaEntry],
) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory {}", parent.display())
            })?;
        }
    }
    let md = generate_mod_schema_reference_markdown(entries);
    fs::write(output_path, md).with_context(|| {
        format!(
            "Failed to write schema reference Markdown to {}",
            output_path.display()
        )
    })
}
