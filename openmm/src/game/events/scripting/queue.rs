use std::collections::VecDeque;

use openmm_data::evt::{EvtFile, EvtStep, GameEvent};

use crate::game::sound::effects::PlayUiSoundEvent;

/// An event sequence — a list of steps from one event_id, executed as a script.
#[derive(Clone)]
pub(crate) struct EventSequence {
    pub event_id: Option<u16>,
    pub steps: Vec<EvtStep>,
}

/// Queue of event sequences waiting to be processed.
/// Each sequence is executed in full (with control flow) in one frame.
#[derive(bevy::prelude::Resource, Default)]
pub struct EventQueue {
    sequences: VecDeque<EventSequence>,
}

impl EventQueue {
    /// Enqueue all steps for a given event_id from the EvtFile as a single sequence.
    pub fn push_all(&mut self, event_id: u16, evt: &EvtFile) {
        if let Some(steps) = evt.events.get(&event_id)
            && !steps.is_empty()
        {
            self.sequences.push_back(EventSequence {
                event_id: Some(event_id),
                steps: steps.clone(),
            });
        }
    }

    /// Pop the next sequence from the front.
    pub(crate) fn pop(&mut self) -> Option<EventSequence> {
        self.sequences.pop_front()
    }

    /// Enqueue a single synthesized event (not from an EvtFile).
    pub fn push_single(&mut self, event: GameEvent) {
        self.sequences.push_back(EventSequence {
            event_id: None,
            steps: vec![EvtStep { step: 0, event }],
        });
    }

    /// Enqueue steps from index `start` onward (used to skip lifecycle marker steps).
    pub fn push_from(&mut self, event_id: u16, evt: &EvtFile, start: usize) {
        if let Some(steps) = evt.events.get(&event_id) {
            let tail: Vec<_> = steps[start.min(steps.len())..].to_vec();
            if !tail.is_empty() {
                self.sequences.push_back(EventSequence {
                    event_id: Some(event_id),
                    steps: tail,
                });
            }
        }
    }

    /// Clear all pending sequences.
    pub fn clear(&mut self) {
        self.sequences.clear();
    }

    /// Extract and play all PlaySound events, keeping everything else queued.
    /// Used during UI overlays so sounds play but other events survive.
    pub fn drain_sounds(&mut self, ui_sound: &mut bevy::ecs::message::MessageWriter<PlayUiSoundEvent>) {
        for seq in &mut self.sequences {
            seq.steps.retain(|step| {
                if let GameEvent::PlaySound { sound_id } = &step.event {
                    ui_sound.write(PlayUiSoundEvent { sound_id: *sound_id });
                    false // remove from queue
                } else {
                    true // keep
                }
            });
        }
        // Remove empty sequences.
        self.sequences.retain(|seq| !seq.steps.is_empty());
    }
}
