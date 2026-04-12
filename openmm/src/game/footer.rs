use bevy::prelude::*;

/// Text displayed in the footer bar. Write to this resource from any system
/// to update the footer message (e.g. building names, hints, status text).
///
/// # Example
/// ```ignore
/// fn my_system(mut footer: ResMut<FooterText>) {
///     footer.set("The Knife Shoppe");
/// }
/// ```
#[derive(Resource, Default)]
pub struct FooterText {
    text: String,
    /// Generation counter -- bumped on every change so consumers know to re-render.
    pub(crate) generation: u64,
    /// If set, this text is "locked" until the timer expires.
    /// Hover hints won't overwrite locked text.
    lock_until: Option<f64>,
}

impl FooterText {
    /// Set footer text. This is a "soft" set — won't overwrite locked (status) text.
    pub fn set(&mut self, text: &str) {
        if self.lock_until.is_some() {
            return;
        }
        if self.text != text {
            self.text = text.to_string();
            self.generation += 1;
        }
    }

    /// Set footer text that persists for `duration` seconds.
    /// Cannot be overwritten by hover hints until it expires.
    pub fn set_status(&mut self, text: &str, duration: f64, now: f64) {
        self.text = text.to_string();
        self.lock_until = Some(now + duration);
        self.generation += 1;
    }

    /// Call every frame to expire locked text.
    pub fn tick(&mut self, now: f64) {
        if let Some(until) = self.lock_until
            && now >= until
        {
            self.lock_until = None;
            self.text.clear();
            self.generation += 1;
        }
    }

    /// Clear the footer text.
    pub fn clear(&mut self) {
        self.set("");
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}
