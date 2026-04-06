pub mod provider;

#[cfg(test)]
pub fn test_lod() -> Option<crate::Assets> {
    crate::Assets::new(crate::get_data_path()).ok()
}

pub mod awards;
pub mod billboard;
pub mod blv;
pub mod bsp_model;
pub mod class;
pub mod dchest;
pub mod ddeclist;
pub mod ddm;
pub mod dift;
pub mod dlv;
pub mod dmonlist;
pub mod dobjlist;
pub mod doverlay;
pub mod dpft;
pub mod dsft;
pub mod dsounds;
pub mod dtile;
pub mod enums;
pub mod evt;
pub mod font;
pub mod image;
pub mod items;
pub mod lod_data;
pub mod mapstats;
pub mod monsters;
pub mod npc;
pub mod npcnews;
pub mod npcprof;
pub mod odm;
pub mod palette;
pub mod quest_bits;
pub mod save;
pub mod smk;
pub mod snd;
pub mod spells;
pub mod terrain;
pub mod tft;
pub mod twodevents;
pub mod zlib;

pub use self::blv::{Blv, BlvDoor, DoorState};
pub use self::ddm::{CommonMonsterProps, Ddm, DdmActor};
pub use self::enums::*;
pub use self::evt::EvtFile;
pub use self::font::Font;
pub use self::image::{Image, get_atlas};
pub use self::items::ItemsTable;
pub use self::lod_data::LodData;
pub use self::mapstats::MapStats;
pub use self::odm::{Odm, SpawnPoint};
pub use self::palette::{Palette, Palettes};
pub use self::provider::smk::{SmkArchive, SmkExt, SmkWriter};
pub use self::provider::{Archive, ArchiveEntry, LodArchive, LodWriter, StaticGameData as GameData, Version};
pub use self::smk::{SmkAudioInfo, SmkDecoder, SmkInfo, parse_smk_info};
pub use self::snd::SndArchive;
pub use self::zlib::*;

// Re-export specific types
pub use self::awards::AwardsTable;
pub use self::billboard::BillboardData;
pub use self::bsp_model::BSPModel;
pub use self::class::ClassTable;
pub use self::dlv::Dlv;
pub use self::dmonlist::MonsterList;
pub use self::dtile::{Dtile, TileTable};
pub use self::npc::{NpcNamePool, StreetNpcs};
pub use self::npcnews::NpcNewsTable;
pub use self::npcprof::NpcProfTable;
pub use self::provider::actors::Actors;
pub use self::provider::decorations::Decorations;
pub use self::provider::lod_decoder::LodDecoder;
pub use self::provider::monster::Monsters;
pub use self::quest_bits::QuestBitNames;
pub use self::save::{SaveFile, SaveHeader, list_saves};
pub use self::spells::SpellsTable;
pub use self::twodevents::TwoDEvents;
