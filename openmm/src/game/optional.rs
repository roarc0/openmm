//! Helpers for systems that operate on optional plugins.
//!
//! Several plugins (sound, sprite materials, world inspector, ...) can be
//! disabled at startup. Systems that depend on them therefore receive
//! `Option<MessageWriter<_>>` / `Option<ResMut<_>>` and would otherwise be
//! peppered with `if let Some(events) = w.as_mut() { events.write(...) }`.
//!
//! [`OptionalWrite::try_write`] keeps the call site one line — the event is
//! constructed unconditionally and silently dropped when the writer is absent.
//! Cost: a single null check on a hot path; events on disabled plugins were
//! already a no-op.

use bevy::ecs::message::{Message, MessageWriter};

pub trait OptionalWrite<E: Message> {
    /// Write `event` if the writer is present; do nothing otherwise.
    fn try_write(&mut self, event: E);
}

impl<E: Message> OptionalWrite<E> for Option<MessageWriter<'_, E>> {
    #[inline]
    fn try_write(&mut self, event: E) {
        if let Some(w) = self.as_mut() {
            w.write(event);
        }
    }
}
