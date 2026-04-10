use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A UI screen definition — the root of a .screen.ron file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screen {
    pub id: String,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub elements: Vec<ScreenElement>,
}

/// A single positioned element on a screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenElement {
    pub id: String,
    pub position: (f32, f32),
    /// Size in reference pixels. (0,0) = auto from texture dimensions.
    #[serde(default)]
    pub size: (f32, f32),
    #[serde(default)]
    pub z: i32,
    #[serde(default)]
    pub states: BTreeMap<String, ElementState>,
    #[serde(default)]
    pub on_click: Vec<String>,
    #[serde(default)]
    pub on_hover: Vec<String>,
    /// Runtime variable bindings. Key = property (texture, text, scroll_x, scroll_y,
    /// offset_x, offset_y, visible), Value = variable name (e.g. "player.compass_yaw").
    #[serde(default)]
    pub bindings: BTreeMap<String, String>,
    /// Color key for transparency. Empty = no transparency.
    /// Values: "black", "cyan", "lime", "red", "magenta", "blue".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub transparent_color: String,
}

/// Visual state of an element — texture + optional trigger condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementState {
    pub texture: String,
    /// Condition that activates this state (e.g. "hover", "time_of_day > 0.75").
    /// Empty = default state (always active as fallback).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub condition: String,
}

impl Screen {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            background: None,
            elements: Vec::new(),
        }
    }
}

impl ScreenElement {
    pub fn new(id: impl Into<String>, texture: impl Into<String>, position: (f32, f32)) -> Self {
        let mut states = BTreeMap::new();
        states.insert(
            "default".to_string(),
            ElementState {
                texture: texture.into(),
                condition: String::new(),
            },
        );
        Self {
            id: id.into(),
            position,
            size: (0.0, 0.0),
            z: 0,
            states,
            on_click: Vec::new(),
            on_hover: Vec::new(),
            bindings: BTreeMap::new(),
            transparent_color: String::new(),
        }
    }

    /// Returns the texture for the given state, falling back to "default".
    pub fn texture_for_state(&self, state: &str) -> Option<&str> {
        self.states
            .get(state)
            .or_else(|| self.states.get("default"))
            .map(|s| s.texture.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_screen_ron() {
        let mut elem = ScreenElement::new("new_game_btn", "mmnew0", (482.0, 9.0));
        elem.size = (135.0, 45.0);
        elem.z = 10;
        elem.states.insert(
            "hover".to_string(),
            ElementState {
                texture: "mmnew1".to_string(),
                condition: "hover".to_string(),
            },
        );
        elem.on_click = vec!["PlaySound 75".to_string(), "GoToScreen segue".to_string()];
        elem.on_hover = vec!["SetState hover".to_string()];

        let screen = Screen {
            id: "title".to_string(),
            background: Some("title.pcx".to_string()),
            elements: vec![elem],
        };

        let ron_str = ron::ser::to_string_pretty(&screen, ron::ser::PrettyConfig::default()).unwrap();
        let parsed: Screen = ron::from_str(&ron_str).unwrap();

        assert_eq!(parsed.id, "title");
        assert_eq!(parsed.elements.len(), 1);

        let btn = &parsed.elements[0];
        assert_eq!(btn.id, "new_game_btn");
        assert_eq!(btn.size, (135.0, 45.0));
        assert_eq!(btn.z, 10);
        assert_eq!(btn.on_click.len(), 2);
        assert_eq!(btn.on_hover.len(), 1);
        assert_eq!(btn.on_hover[0], "SetState hover");
        assert!(btn.bindings.is_empty());
    }

    #[test]
    fn bindings_round_trip() {
        let mut elem = ScreenElement::new("compass", "compass", (100.0, 10.0));
        elem.bindings
            .insert("scroll_x".to_string(), "player.compass_yaw".to_string());
        elem.bindings
            .insert("visible".to_string(), "hud.view_is_world".to_string());

        let ron_str = ron::ser::to_string_pretty(&elem, ron::ser::PrettyConfig::default()).unwrap();
        let parsed: ScreenElement = ron::from_str(&ron_str).unwrap();

        assert_eq!(parsed.bindings.len(), 2);
        assert_eq!(parsed.bindings["scroll_x"], "player.compass_yaw");
        assert_eq!(parsed.bindings["visible"], "hud.view_is_world");
    }

    #[test]
    fn deserialize_minimal_screen() {
        let ron_str = r#"Screen(id: "empty", elements: [])"#;
        let screen: Screen = ron::from_str(ron_str).unwrap();
        assert_eq!(screen.id, "empty");
        assert!(screen.elements.is_empty());
    }

    #[test]
    fn size_zero_means_auto() {
        let ron_str = r#"(id: "x", position: (10.0, 20.0), states: {})"#;
        let elem: ScreenElement = ron::from_str(ron_str).unwrap();
        assert_eq!(elem.size, (0.0, 0.0));
        assert!(elem.on_hover.is_empty());
    }
}
