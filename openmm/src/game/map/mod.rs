//! Map infrastructure: indoor (BLV) and outdoor (ODM) maps, collision,
//! coordinate conversion, and spatial indexing.

use bevy::prelude::*;
use openmm_data::utils::MapName;

pub mod collision;
pub mod coords;
pub(crate) mod indoor;
pub(crate) mod outdoor;
pub(crate) mod spatial_index;

/// Resource indicating the currently active and fully loaded map type.
/// Replaces scattered `resource_exists::<Prepared(Indoor)World>` checks.
#[derive(Resource, Deref, DerefMut, Clone, Debug)]
pub struct CurrentMap(pub MapName);

/// Run condition: the current map is outdoor.
pub fn is_outdoor(current: Option<Res<CurrentMap>>) -> bool {
    current.as_ref().is_some_and(|c| c.0.is_outdoor())
}

/// Run condition: the current map is indoor.
pub fn is_indoor(current: Option<Res<CurrentMap>>) -> bool {
    current.as_ref().is_some_and(|c| c.0.is_indoor())
}
