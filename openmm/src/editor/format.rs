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
    #[serde(default)]
    pub size: Option<(f32, f32)>,
    #[serde(default)]
    pub z: i32,
    #[serde(default)]
    pub states: BTreeMap<String, ElementState>,
    #[serde(default)]
    pub on_click: Vec<String>,
}

/// Visual state of an element — currently just a texture name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementState {
    pub texture: String,
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
            },
        );
        Self {
            id: id.into(),
            position,
            size: None,
            z: 0,
            states,
            on_click: Vec::new(),
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
        let screen = Screen {
            id: "title".to_string(),
            background: Some("title.pcx".to_string()),
            elements: vec![ScreenElement {
                id: "new_game_btn".to_string(),
                position: (482.0, 9.0),
                size: Some((135.0, 45.0)),
                z: 10,
                states: BTreeMap::from([
                    (
                        "default".to_string(),
                        ElementState {
                            texture: "mmnew0".to_string(),
                        },
                    ),
                    (
                        "hover".to_string(),
                        ElementState {
                            texture: "mmnew1".to_string(),
                        },
                    ),
                ]),
                on_click: vec!["PlaySound 75".to_string(), "GoToScreen segue".to_string()],
            }],
        };

        let ron_str = ron::ser::to_string_pretty(&screen, ron::ser::PrettyConfig::default()).unwrap();
        let parsed: Screen = ron::from_str(&ron_str).unwrap();

        assert_eq!(parsed.id, "title");
        assert_eq!(parsed.background.as_deref(), Some("title.pcx"));
        assert_eq!(parsed.elements.len(), 1);

        let btn = &parsed.elements[0];
        assert_eq!(btn.id, "new_game_btn");
        assert_eq!(btn.position, (482.0, 9.0));
        assert_eq!(btn.size, Some((135.0, 45.0)));
        assert_eq!(btn.z, 10);
        assert_eq!(btn.states.len(), 2);
        assert_eq!(btn.texture_for_state("hover"), Some("mmnew1"));
        assert_eq!(btn.texture_for_state("missing"), Some("mmnew0"));
        assert_eq!(btn.on_click.len(), 2);
        assert_eq!(btn.on_click[0], "PlaySound 75");
    }

    #[test]
    fn deserialize_minimal_screen() {
        let ron_str = r#"Screen(id: "empty", elements: [])"#;
        let screen: Screen = ron::from_str(ron_str).unwrap();
        assert_eq!(screen.id, "empty");
        assert!(screen.background.is_none());
        assert!(screen.elements.is_empty());
    }
}
