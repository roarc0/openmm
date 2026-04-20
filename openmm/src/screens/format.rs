use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Editor-only metadata embedded in the screen RON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EditorSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locked: Vec<String>,
}

fn editor_section_is_empty(s: &EditorSection) -> bool {
    s.locked.is_empty()
}

fn is_default_kind(k: &ScreenKind) -> bool {
    matches!(k, ScreenKind::Base)
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn is_empty(v: &str) -> bool {
    v.is_empty()
}

fn deserialize_click_sound<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ClickSoundValue {
        Name(String),
        Id(u32),
    }

    match ClickSoundValue::deserialize(deserializer)? {
        ClickSoundValue::Name(name) => Ok(name),
        ClickSoundValue::Id(id) => Ok(format!("#{id}")),
    }
}

/// Background sound definition.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub enum Sound {
    #[default]
    None,
    Id(String),
    Sound {
        id: String,
        start_sec: f32,
        looping: bool,
    },
}

impl Sound {
    fn default_true() -> bool {
        true
    }

    pub fn id(&self) -> &str {
        match self {
            Self::None => "",
            Self::Id(id) => id,
            Self::Sound { id, .. } => id,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.id().is_empty()
    }
    pub fn start_sec(&self) -> f32 {
        match self {
            Self::Sound { start_sec, .. } => *start_sec,
            _ => 0.0,
        }
    }
    pub fn looping(&self) -> bool {
        match self {
            Self::Sound { looping, .. } => *looping,
            _ => true,
        }
    }
}

impl<'de> serde::Deserialize<'de> for Sound {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct SoundFields {
            id: String,
            #[serde(default)]
            start_sec: f32,
            #[serde(default = "Sound::default_true")]
            looping: bool,
        }

        struct SoundFieldsVisitor;
        impl<'de> serde::de::Visitor<'de> for SoundFieldsVisitor {
            type Value = SoundFields;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("Sound fields")
            }
            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                SoundFields::deserialize(serde::de::value::MapAccessDeserializer::new(map))
            }
            fn visit_seq<S>(self, seq: S) -> Result<Self::Value, S::Error>
            where
                S: serde::de::SeqAccess<'de>,
            {
                SoundFields::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))
            }
        }

        struct SoundVisitor;
        impl<'de> serde::de::Visitor<'de> for SoundVisitor {
            type Value = Sound;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or a Sound(...) object")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v.is_empty() {
                    Ok(Sound::None)
                } else {
                    Ok(Sound::Id(v.to_owned()))
                }
            }

            fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::EnumAccess<'de>,
            {
                use serde::de::VariantAccess;
                let (variant, access): (String, _) = data.variant()?;
                if variant == "Sound" {
                    let fields = access.struct_variant(&["id", "start_sec", "looping"], SoundFieldsVisitor)?;
                    Ok(Sound::Sound {
                        id: fields.id,
                        start_sec: fields.start_sec,
                        looping: fields.looping,
                    })
                } else {
                    Err(serde::de::Error::unknown_variant(&variant, &["Sound"]))
                }
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let fields = SoundFields::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(Sound::Sound {
                    id: fields.id,
                    start_sec: fields.start_sec,
                    looping: fields.looping,
                })
            }

            fn visit_seq<S>(self, seq: S) -> Result<Self::Value, S::Error>
            where
                S: serde::de::SeqAccess<'de>,
            {
                let fields = SoundFields::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))?;
                Ok(Sound::Sound {
                    id: fields.id,
                    start_sec: fields.start_sec,
                    looping: fields.looping,
                })
            }
        }

        deserializer.deserialize_any(SoundVisitor)
    }
}

/// Screen layer kind — controls stacking priority and mutual exclusivity.
///
/// - `Hud`: persistent layer, lowest key priority (e.g. playing, ingame)
/// - `Modal`: exclusive overlay, highest key priority (e.g. options_main, npc_speak)
/// - `Base`: default — splash screens, menus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScreenKind {
    #[default]
    Base,
    Hud,
    Modal,
}

/// A UI screen definition — the root of a .screen.ron file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screen {
    pub id: String,
    /// Screen layer kind — controls key priority and stacking behavior.
    #[serde(default, skip_serializing_if = "is_default_kind")]
    pub kind: ScreenKind,
    /// Background music track (e.g. "15" or Sound(id: "15", start_sec: 1.5, looping: true)).
    #[serde(
        default,
        rename = "sound",
        alias = "bg_sound",
        skip_serializing_if = "Sound::is_empty"
    )]
    pub sound: Sound,
    /// Keyboard shortcuts. Key = key name (e.g. "Escape", "Return", "N"), Value = actions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<String, Vec<String>>,
    /// Actions executed when the screen is first loaded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_load: Vec<String>,
    /// Actions executed when the screen is hidden/closed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_close: Vec<String>,
    #[serde(default)]
    pub elements: Vec<ScreenElement>,
    /// Editor-only section — locked elements, etc. Stripped at runtime.
    #[serde(default, skip_serializing_if = "editor_section_is_empty")]
    pub editor: EditorSection,
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
            Self::Text(e) => &e.on_click,
            Self::Video(_) => &[],
        }
    }
    pub fn on_hover(&self) -> &[String] {
        match self {
            Self::Image(e) => &e.on_hover,
            Self::Text(e) => &e.on_hover,
            Self::Video(_) => &[],
        }
    }
    pub fn as_image(&self) -> Option<&ImageElement> {
        match self {
            Self::Image(e) => Some(e),
            _ => None,
        }
    }
    pub fn as_image_mut(&mut self) -> Option<&mut ImageElement> {
        match self {
            Self::Image(e) => Some(e),
            _ => None,
        }
    }
    pub fn as_video(&self) -> Option<&VideoElement> {
        match self {
            Self::Video(e) => Some(e),
            _ => None,
        }
    }
    pub fn as_video_mut(&mut self) -> Option<&mut VideoElement> {
        match self {
            Self::Video(e) => Some(e),
            _ => None,
        }
    }
    pub fn as_text(&self) -> Option<&TextElement> {
        match self {
            Self::Text(e) => Some(e),
            _ => None,
        }
    }
    pub fn as_text_mut(&mut self) -> Option<&mut TextElement> {
        match self {
            Self::Text(e) => Some(e),
            _ => None,
        }
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
    /// Frame animation: cycles through prefix01, prefix02, ... textures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<Animation>,
    /// Color key for transparency.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub transparent_color: String,
    /// Crop viewport width in reference pixels. When set, the image spawns inside
    /// a clip container of this size and can scroll within it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub crop_w: f32,
    /// Crop viewport height in reference pixels.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub crop_h: f32,
    /// When true and explicit size is set, crop the texture to size instead of stretching.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub crop: bool,
    /// Sound to play on click.
    /// Name lookup (e.g. "ClickStart") or explicit ID with '#', e.g. "#42".
    #[serde(
        default,
        skip_serializing_if = "is_empty",
        deserialize_with = "deserialize_click_sound",
        alias = "click_sound_id"
    )]
    pub click_sound: String,
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
    /// Data source binding (e.g. "ui.footer", "player.gold", "npc.greeting").
    #[serde(default)]
    pub source: String,
    /// Default static value if source is missing or empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_click: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_hover: Vec<String>,
    /// MM6 font name (e.g. "smallnum", "arrus", "book").
    #[serde(default = "TextElement::default_font")]
    pub font: String,
    /// Glyph height in reference pixels. 0 = use `size.1`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub font_size: f32,
    /// Text color: "white", "yellow", "red", "green".
    #[serde(default = "TextElement::default_color")]
    pub color: String,
    /// Color to swap to when the mouse is over the text (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hover_color: Option<String>,
    /// Text alignment: "left", "center", "right".
    /// - "left": position is the left edge, text grows right (default)
    /// - "center": position is the center point, text grows both ways
    /// - "right": position is the right edge, text grows left (gold/food)
    #[serde(default = "TextElement::default_align")]
    pub align: String,
}

/// Valid text sources for the source dropdown (object.property format).
pub const TEXT_SOURCES: &[&str] = &[
    "ui.footer",
    "player.gold",
    "player.food",
    "player.reputation",
    "player.map_name",
    "npc.name",
    "npc.full_name",
    "npc.greeting",
    "npc.profession",
    "member0.class",
    "member0.selected_stat",
    "house.name",
    "house.owner",
    "loading.step",
];
/// Valid text alignments.
pub const TEXT_ALIGNS: &[&str] = &["left", "center", "right"];
/// Valid text colors.
pub const TEXT_COLORS: &[&str] = &["white", "yellow", "gold", "red", "green", "blue", "cyan", "magenta"];

impl TextElement {
    fn default_font() -> String {
        "smallnum".into()
    }
    fn default_color() -> String {
        "white".into()
    }
    fn default_align() -> String {
        "left".into()
    }

    pub fn color_rgba(&self) -> [u8; 4] {
        Self::resolve_color(&self.color)
    }

    pub fn hover_rgba(&self) -> Option<[u8; 4]> {
        self.hover_color.as_ref().map(|c| Self::resolve_color(c))
    }

    pub fn resolve_color(name: &str) -> [u8; 4] {
        super::fonts::resolve_text_color(name)
    }
}

/// Frame animation descriptor — cycles through numbered textures.
///
/// `pattern` is a printf-style format string with one `%d` placeholder:
///   - `"icons/Watwalk%02d"` → Watwalk01, Watwalk02, ...
///   - `"icons/spell%03d"`   → spell001, spell002, ...
///   - `"icons/fire%d"`      → fire1, fire2, ...
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Animation {
    /// Printf-style pattern with `%d`, `%02d`, `%03d`, etc.
    pub pattern: String,
    /// Total frame count.
    pub frames: u32,
    /// The starting frame index (usually 0 or 1).
    #[serde(default = "default_start_frame")]
    pub start_frame: u32,
    /// Frames per second.
    #[serde(default = "default_fps")]
    pub fps: f32,
    /// If true, the animation cycles 0-1-2-1-0 instead of 0-1-2-0.
    #[serde(default)]
    pub ping_pong: bool,
}

fn default_start_frame() -> u32 {
    1
}

fn default_fps() -> f32 {
    10.0
}

/// Visual state of an element — texture + optional trigger condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementState {
    pub texture: String,
    /// Condition that activates this state (e.g. "hover", "time_of_day > 0.75").
    /// Empty = default state (always active as fallback).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub condition: String,
    /// Per-state transparency color key. Overrides the element-level `transparent_color`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub transparent_color: String,
    /// Frame animation for this state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<Animation>,
}

impl Screen {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: ScreenKind::default(),
            sound: Sound::None,
            keys: BTreeMap::new(),
            on_load: Vec::new(),
            on_close: Vec::new(),
            elements: Vec::new(),
            editor: EditorSection::default(),
        }
    }

    /// Highest z-order among all elements, or 0 if empty.
    pub fn max_z(&self) -> i32 {
        self.elements.iter().map(|e| e.z()).max().unwrap_or(0)
    }

    /// Prune the locked list to only include current element IDs.
    pub fn prune_locked_elements(&mut self) {
        let element_ids: std::collections::HashSet<_> = self.elements.iter().map(|e| e.id().to_string()).collect();
        self.editor.locked.retain(|id| element_ids.contains(id));
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
                transparent_color: String::new(),
                animation: None,
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
            animation: None,
            transparent_color: String::new(),
            crop_w: 0.0,
            crop_h: 0.0,
            crop: false,
            click_sound: String::new(),
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
                animation: None,
                transparent_color: String::new(),
            },
        );
        img.on_click = vec!["PlaySound 75".to_string(), "GoToScreen segue".to_string()];
        img.on_hover = vec!["SetState hover".to_string()];

        let mut screen = Screen::new("title");
        screen.elements = vec![ScreenElement::Image(img)];

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
    fn image_click_sound_accepts_name() {
        let ron_str = r#"(
            id: "test",
            elements: [
                Image((
                    id: "btn",
                    position: (0.0, 0.0),
                    states: {"default": (texture: "btn")},
                    click_sound: "ClickStart",
                )),
            ],
        )"#;
        let screen: Screen = ron::from_str(ron_str).unwrap();
        let btn = screen.elements[0].as_image().unwrap();
        assert_eq!(btn.click_sound, "ClickStart");
    }

    #[test]
    fn image_click_sound_accepts_numeric_legacy_field() {
        let ron_str = r#"(
            id: "test",
            elements: [
                Image((
                    id: "btn",
                    position: (0.0, 0.0),
                    states: {"default": (texture: "btn")},
                    click_sound_id: 42,
                )),
            ],
        )"#;
        let screen: Screen = ron::from_str(ron_str).unwrap();
        let btn = screen.elements[0].as_image().unwrap();
        assert_eq!(btn.click_sound, "#42");
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

        let mut screen = Screen::new("splash");
        screen.elements = vec![ScreenElement::Video(vid)];

        let ron_str = ron::ser::to_string_pretty(&screen, ron::ser::PrettyConfig::default()).unwrap();
        let parsed: Screen = ron::from_str(&ron_str).unwrap();

        let v = parsed.elements[0].as_video().unwrap();
        assert_eq!(v.video, "3dologo");
        assert!(v.looping);
        assert!(v.skippable);
        assert_eq!(v.on_end.len(), 1);
    }

    #[test]
    fn round_trip_text_element() {
        let txt = TextElement {
            id: "status".to_string(),
            position: (10.0, 10.0),
            size: (100.0, 20.0),
            z: 20,
            hidden: false,
            source: "gold".to_string(),
            value: String::new(),
            font: "arrus".to_string(),
            font_size: 14.0,
            color: "yellow".to_string(),
            hover_color: None,
            align: "right".to_string(),
            on_click: vec!["PlaySound 1".to_string()],
            on_hover: vec!["evt:Hint(\"Gold\")".to_string()],
        };

        let mut screen = Screen::new("stats");
        screen.elements = vec![ScreenElement::Text(txt)];

        let ron_str = ron::ser::to_string_pretty(&screen, ron::ser::PrettyConfig::default()).unwrap();
        let parsed: Screen = ron::from_str(&ron_str).unwrap();

        let t = parsed.elements[0].as_text().unwrap();
        assert_eq!(t.id, "status");
        assert_eq!(t.on_click.len(), 1);
        assert_eq!(t.on_hover.len(), 1);
        assert_eq!(t.font, "arrus");
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

    #[test]
    fn prune_locked_elements() {
        let mut screen = Screen::new("test");
        screen.elements = vec![ScreenElement::Image(ImageElement::new("valid", "tex", (0.0, 0.0)))];
        screen.editor.locked = vec!["valid".to_string(), "invalid".to_string()];

        screen.prune_locked_elements();

        assert_eq!(screen.editor.locked.len(), 1);
        assert_eq!(screen.editor.locked[0], "valid");
    }

    #[test]
    fn resolve_color_supports_gold_cyan_and_magenta() {
        assert_eq!(TextElement::resolve_color("gold"), super::super::fonts::GOLD);
        assert_eq!(TextElement::resolve_color("cyan"), super::super::fonts::CYAN);
        assert_eq!(TextElement::resolve_color("magenta"), super::super::fonts::MAGENTA);
    }
}
