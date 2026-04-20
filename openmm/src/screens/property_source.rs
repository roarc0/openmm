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
        if let Some(i) = spec.find('.') {
            let (name, path) = (&spec[..i], &spec[i + 1..]);
            self.sources.get(name).and_then(|s| s.resolve(path))
        } else {
            // Bare object name — resolve with empty path (default property).
            self.sources.get(spec).and_then(|s| s.resolve(""))
        }
    }
}

/// Interpolate all `${object.property}` placeholders in `template` using the registry.
/// Placeholders that cannot be resolved are left as-is.
/// If the template contains no `${`, it is returned unchanged (zero allocation).
pub fn interpolate(template: &str, registry: &PropertyRegistry) -> String {
    if !template.contains("${") {
        return template.to_string();
    }
    let mut result = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        if let Some(end) = rest.find('}') {
            let expr = &rest[..end];
            rest = &rest[end + 1..];
            match registry.resolve(expr) {
                Some(val) => result.push_str(&val),
                None => {
                    result.push_str("${");
                    result.push_str(expr);
                    result.push('}');
                }
            }
        } else {
            // Unclosed placeholder — emit literally and stop.
            result.push_str("${");
            result.push_str(rest);
            break;
        }
    }
    result.push_str(rest);
    result
}

/// Marks an image entity whose texture name is a template string resolved each frame.
/// Attach alongside the entity's [`bevy::prelude::ImageNode`].
/// The system `dynamic_texture_update` will resolve `${...}` placeholders and swap
/// the handle whenever the resolved name changes.
#[derive(Component)]
pub struct DynamicTexture {
    /// Template string, e.g. `"icons/${char0.portrait}"`.
    pub template: String,
    /// Transparent-color key passed to texture loader (empty = opaque).
    pub transparent_color: String,
    /// Last resolved texture name — used to skip redundant reloads.
    pub last_resolved: String,
}
