use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A UI screen definition — the root of a .screen.ron file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screen {
    pub id: String,
    /// Background music track name (e.g. "15" for Music/15.mp3). Empty = no music.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bg_music: String,
    /// Keyboard shortcuts. Key = key name (e.g. "Escape", "Return", "N"), Value = actions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub elements: Vec<ScreenElement>,
}

/// A screen element — image, video, or dynamic text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScreenElement {
    Image(ImageElement),
    Video(VideoElement),
    Text(TextElement),
}

/// Shared fields accessible on any element variant.
impl ScreenElement {
    pub fn id(&self) -> &str {
        match self {
            Self::Image(e) => &e.id,
            Self::Video(e) => &e.id,
            Self::Text(e) => &e.id,
        }
    }
    pub fn position(&self) -> (f32, f32) {
        match self {
            Self::Image(e) => e.position,
            Self::Video(e) => e.position,
            Self::Text(e) => e.position,
        }
    }
    pub fn set_position(&mut self, pos: (f32, f32)) {
        match self {
            Self::Image(e) => e.position = pos,
            Self::Video(e) => e.position = pos,
            Self::Text(e) => e.position = pos,
        }
    }
    pub fn size(&self) -> (f32, f32) {
        match self {
            Self::Image(e) => e.size,
            Self::Video(e) => e.size,
            Self::Text(e) => e.size,
        }
    }
    pub fn set_size(&mut self, size: (f32, f32)) {
        match self {
            Self::Image(e) => e.size = size,
            Self::Video(e) => e.size = size,
            Self::Text(e) => e.size = size,
        }
    }
    pub fn z(&self) -> i32 {
        match self {
            Self::Image(e) => e.z,
            Self::Video(e) => e.z,
            Self::Text(e) => e.z,
        }
    }
    pub fn set_z(&mut self, z: i32) {
        match self {
            Self::Image(e) => e.z = z,
            Self::Video(e) => e.z = z,
            Self::Text(e) => e.z = z,
        }
    }
    pub fn hidden(&self) -> bool {
        match self {
            Self::Image(e) => e.hidden,
            Self::Video(e) => e.hidden,
            Self::Text(e) => e.hidden,
        }
    }
    pub fn on_click(&self) -> &[String] {
        match self {
            Self::Image(e) => &e.on_click,
            Self::Video(_) | Self::Text(_) => &[],
        }
    }
    pub fn on_hover(&self) -> &[String] {
        match self {
            Self::Image(e) => &e.on_hover,
            Self::Video(_) | Self::Text(_) => &[],
        }
    }
    pub fn as_image(&self) -> Option<&ImageElement> {
        match self { Self::Image(e) => Some(e), _ => None }
    }
    pub fn as_image_mut(&mut self) -> Option<&mut ImageElement> {
        match self { Self::Image(e) => Some(e), _ => None }
    }
    pub fn as_video(&self) -> Option<&VideoElement> {
        match self { Self::Video(e) => Some(e), _ => None }
    }
    pub fn as_video_mut(&mut self) -> Option<&mut VideoElement> {
        match self { Self::Video(e) => Some(e), _ => None }
    }
    pub fn as_text(&self) -> Option<&TextElement> {
        match self { Self::Text(e) => Some(e), _ => None }
    }
    pub fn as_text_mut(&mut self) -> Option<&mut TextElement> {
        match self { Self::Text(e) => Some(e), _ => None }
    }
}

/// A static image element with texture states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageElement {
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
    /// Runtime variable bindings.
    #[serde(default)]
    pub bindings: BTreeMap<String, String>,
    /// Start hidden.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// Color key for transparency.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub transparent_color: String,
}

/// A video element that plays an SMK file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoElement {
    pub id: String,
    pub position: (f32, f32),
    /// Size in reference pixels. (0,0) = use video native resolution.
    #[serde(default)]
    pub size: (f32, f32),
    #[serde(default)]
    pub z: i32,
    /// SMK file name without extension (e.g. "3dologo").
    pub video: String,
    /// Start hidden.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub looping: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skippable: bool,
    /// Actions when video ends (ignored if looping).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_end: Vec<String>,
}

/// A dynamic text element bound to a runtime data source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextElement {
    pub id: String,
    pub position: (f32, f32),
    /// Size in reference pixels.
    #[serde(default)]
    pub size: (f32, f32),
    #[serde(default)]
    pub z: i32,
    /// Start hidden.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// Data source binding (e.g. "footer_text", "gold", "food").
    #[serde(default)]
    pub source: String,
    /// MM6 font name (e.g. "smallnum", "arrus", "book").
    #[serde(default = "TextElement::default_font")]
    pub font: String,
    /// Text color: "white", "yellow", "red", "green".
    #[serde(default = "TextElement::default_color")]
    pub color: String,
    /// Text alignment: "left", "center", "right".
    /// - "left": position is the left edge, text grows right (default)
    /// - "center": position is the center point, text grows both ways
    /// - "right": position is the right edge, text grows left (gold/food)
    #[serde(default = "TextElement::default_align")]
    pub align: String,
}

/// Valid text sources for the source dropdown.
pub const TEXT_SOURCES: &[&str] = &["footer_text", "gold", "food"];
/// Valid text alignments.
pub const TEXT_ALIGNS: &[&str] = &["left", "center", "right"];
/// Valid text colors.
pub const TEXT_COLORS: &[&str] = &["white", "yellow", "red", "green"];

impl TextElement {
    fn default_font() -> String { "smallnum".into() }
    fn default_color() -> String { "white".into() }
    fn default_align() -> String { "left".into() }

    pub fn color_rgba(&self) -> [u8; 4] {
        match self.color.as_str() {
            "yellow" => crate::fonts::YELLOW,
            "red" => crate::fonts::RED,
            "green" => crate::fonts::GREEN,
            _ => crate::fonts::WHITE,
        }
    }
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
            bg_music: String::new(),
            keys: BTreeMap::new(),
            elements: Vec::new(),
        }
    }
}

impl ImageElement {
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
            hidden: false,
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
    fn round_trip_image_element() {
        let mut img = ImageElement::new("new_game_btn", "mmnew0", (482.0, 9.0));
        img.size = (135.0, 45.0);
        img.z = 10;
        img.states.insert(
            "hover".to_string(),
            ElementState {
                texture: "mmnew1".to_string(),
                condition: "hover".to_string(),
            },
        );
        img.on_click = vec!["PlaySound 75".to_string(), "GoToScreen segue".to_string()];
        img.on_hover = vec!["SetState hover".to_string()];

        let screen = Screen {
            id: "title".to_string(),
            bg_music: String::new(),
            keys: BTreeMap::new(),
            elements: vec![ScreenElement::Image(img)],
        };

        let ron_str = ron::ser::to_string_pretty(&screen, ron::ser::PrettyConfig::default()).unwrap();
        let parsed: Screen = ron::from_str(&ron_str).unwrap();

        assert_eq!(parsed.id, "title");
        assert_eq!(parsed.elements.len(), 1);

        let btn = parsed.elements[0].as_image().unwrap();
        assert_eq!(btn.id, "new_game_btn");
        assert_eq!(btn.size, (135.0, 45.0));
        assert_eq!(btn.z, 10);
        assert_eq!(btn.on_click.len(), 2);
        assert_eq!(btn.on_hover.len(), 1);
    }

    #[test]
    fn round_trip_video_element() {
        let vid = VideoElement {
            id: "intro".to_string(),
            position: (100.0, 50.0),
            size: (320.0, 240.0),
            z: 5,
            video: "3dologo".to_string(),
            hidden: false,
            looping: true,
            skippable: true,
            on_end: vec!["LoadScreen(\"menu\")".to_string()],
        };

        let screen = Screen {
            id: "splash".to_string(),
            bg_music: String::new(),
            keys: BTreeMap::new(),
            elements: vec![ScreenElement::Video(vid)],
        };

        let ron_str = ron::ser::to_string_pretty(&screen, ron::ser::PrettyConfig::default()).unwrap();
        let parsed: Screen = ron::from_str(&ron_str).unwrap();

        let v = parsed.elements[0].as_video().unwrap();
        assert_eq!(v.video, "3dologo");
        assert!(v.looping);
        assert!(v.skippable);
        assert_eq!(v.on_end.len(), 1);
    }

    #[test]
    fn mixed_elements() {
        let ron_str = r#"(
            id: "test",
            elements: [
                Image((id: "bg", position: (0.0, 0.0), states: {"default": (texture: "bg.pcx")})),
                Video((id: "vid", position: (10.0, 10.0), video: "intro")),
            ],
        )"#;
        let screen: Screen = ron::from_str(ron_str).unwrap();
        assert_eq!(screen.elements.len(), 2);
        assert!(screen.elements[0].as_image().is_some());
        assert!(screen.elements[1].as_video().is_some());
    }

    #[test]
    fn deserialize_minimal_screen() {
        let ron_str = r#"Screen(id: "empty", elements: [])"#;
        let screen: Screen = ron::from_str(ron_str).unwrap();
        assert_eq!(screen.id, "empty");
        assert!(screen.elements.is_empty());
    }
}
