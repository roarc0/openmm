use std::error::Error;

use crate::ddm::{Ddm, DdmActor};
use crate::lod_data::LodData;
use crate::LodManager;

/// Parsed DLV indoor delta file. The indoor equivalent of DDM.
/// Actors use the same MapMonster struct (548 bytes each).
pub struct Dlv {
    pub actors: Vec<DdmActor>,
}

impl Dlv {
    pub fn new(lod_manager: &LodManager, map_name: &str) -> Result<Self, Box<dyn Error>> {
        let dlv_name = map_name
            .rsplit_once('.')
            .map(|(base, _)| format!("{}.dlv", base))
            .unwrap_or_else(|| format!("{}.dlv", map_name));

        let raw = lod_manager.try_get_bytes(&format!("games/{}", dlv_name))?;
        let data = LodData::try_from(raw)?;

        let actors = Ddm::parse_from_data(&data.data).unwrap_or_default();

        Ok(Dlv { actors })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_lod_path;

    #[test]
    fn parse_d01_dlv() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();
        let dlv = Dlv::new(&lod_manager, "d01.blv").unwrap();
        println!("d01.dlv actor count: {}", dlv.actors.len());
        for (i, actor) in dlv.actors.iter().enumerate() {
            println!(
                "  [{}] name={:?} monlist_id={} npc_id={} pos={:?}",
                i, actor.name, actor.monlist_id, actor.npc_id, actor.position
            );
        }
    }
}
