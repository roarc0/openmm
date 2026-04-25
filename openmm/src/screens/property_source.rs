//! Dynamic property resolution for screen bindings.
//!
//! Resources implement [`PropertySource`] to expose named properties
//! to screen elements without hardcoding resource access in `text_update`.
//!
//! RON text source format: `"object.property"`
//! e.g. `"player.gold"`, `"ui.footer"`, `"loading.step"`
//!
//! Template interpolation: any string field may contain `${object.property}`
//! placeholders that are resolved at runtime. Multiple placeholders per string
//! are supported, e.g. `"Hello ${npc.name}!"`.

use bevy::prelude::{Component, Resource};
use std::borrow::Cow;
use std::collections::HashMap;

/// A resource that exposes named properties to screen bindings.
pub trait PropertySource: Send + Sync {
    /// Unique name identifying this source in RON, e.g. `"player"`, `"ui"`.
    fn source_name(&self) -> &str;
    /// Resolve a property path to a display string.
    /// Returns `None` if the path is unknown.
    fn resolve(&self, path: &str) -> Option<String>;
}

/// Registry of property sources, injected into `text_update` to resolve
/// `"object.property"` bindings without hardcoding resource access.
#[derive(Resource, Default)]
pub struct PropertyRegistry {
    sources: HashMap<String, Box<dyn PropertySource>>,
}

impl PropertyRegistry {
    /// Register a property source (replaces existing if same name).
    pub fn register(&mut self, source: Box<dyn PropertySource>) {
        self.sources.insert(source.source_name().to_string(), source);
    }

    /// Resolve an `"object.property"` spec. Returns `None` if object not found
    /// or property not handled.
    pub fn resolve(&self, spec: &str) -> Option<String> {
        let split_pos = spec.find(['.', '[']);
        if let Some(i) = split_pos {
            let (name, path) = (&spec[..i], &spec[i..]);
            // If we split on '.', strip it from the path. If we split on '[', keep it.
            let clean_path = if path.starts_with('.') { &path[1..] } else { path };
            self.sources.get(name).and_then(|s| s.resolve(clean_path))
        } else {
            // Bare object name — resolve with empty path (default property).
            self.sources.get(spec).and_then(|s| s.resolve(""))
        }
    }
}

/// Interpolate all `${object.property}` and `$var` placeholders in `template`.
/// Placeholders that cannot be resolved are left as-is.
/// Returns `Cow::Borrowed` when no `$` is present.
pub fn interpolate<'a>(template: &'a str, registry: &PropertyRegistry) -> Cow<'a, str> {
    if !template.contains('$') {
        return Cow::Borrowed(template);
    }
    let mut result = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('$') {
        result.push_str(&rest[..start]);
        let mut inner = &rest[start + 1..];

        if inner.starts_with('{') {
            // Handle ${object.property}
            inner = &inner[1..];
            if let Some(end) = inner.find('}') {
                let expr = &inner[..end];
                rest = &inner[end + 1..];
                match registry.resolve(expr) {
                    Some(val) => result.push_str(&val),
                    None => {
                        result.push_str("${");
                        result.push_str(expr);
                        result.push('}');
                    }
                }
            } else {
                result.push_str("${");
                result.push_str(inner);
                return Cow::Owned(result);
            }
        } else {
            // Handle $var or $var[n]
            // Read until next whitespace, punctuation (except . or [ or ]), or $
            let end = inner
                .find(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '[' && c != ']')
                .unwrap_or(inner.len());
            let expr = &inner[..end];
            rest = &inner[end..];

            if expr.is_empty() {
                result.push('$');
                continue;
            }

            // Special case for $currentTime -> time.full (for backward compatibility if needed)
            let resolved_expr = if expr == "currentTime" { "time.full" } else { expr };

            match registry.resolve(resolved_expr) {
                Some(val) => result.push_str(&val),
                None => {
                    result.push('$');
                    result.push_str(expr);
                }
            }
        }
    }
    result.push_str(rest);
    Cow::Owned(result)
}

/// Marks an image entity whose texture name is a template string resolved each frame.
/// Attach alongside the entity's [`bevy::prelude::ImageNode`].
/// The system `dynamic_texture_update` will resolve `${...}` placeholders and swap
/// the handle whenever the resolved name changes.
#[derive(Component)]
pub struct DynamicTexture {
    /// Template string, e.g. `"icons/${member0.portrait}"`.
    pub template: String,
    /// Transparent-color key passed to texture loader (empty = opaque).
    pub transparent_color: String,
    /// Last resolved texture name — used to skip redundant reloads.
    pub last_resolved: String,
}
