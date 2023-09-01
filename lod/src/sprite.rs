use std::error::Error;

use image::DynamicImage;

use crate::{
    ddeclist::{DDecList, DDecListItem},
    dsft::{DSFTFrame, DSFT},
    LodManager,
};

pub struct SpriteManager {
    d_declist: DDecList,
    d_sft: DSFT,
}

pub struct Sprite {
    pub image: DynamicImage,
    pub d_declist_item: DDecListItem,
    pub d_sft_frame: DSFTFrame,
}

impl SpriteManager {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let d_declist = DDecList::new(&lod_manager)?;
        let d_sft = DSFT::new(&lod_manager)?;
        Ok(Self { d_declist, d_sft })
    }

    pub fn sprite(&self, lod_manager: &LodManager, name: &str, declist_id: u16) -> Option<Sprite> {
        let declist_item = self.d_declist.items.get(declist_id as usize)?;
        let sft_frame = self.d_sft.frames.get(declist_item.sft_index() as usize)?;

        let image = if let Some(image) = lod_manager.sprite(&declist_item.name()?) {
            image
        } else if let Some(image) = lod_manager.sprite(&sft_frame.sprite_name()?) {
            image
        } else if let Some(image) = lod_manager.sprite(name) {
            image
        } else {
            println!(
                "failed to read entity: id:{}|name:{:?}|game_name:{:?}, sft group_name:{:?}|sprite_name:{:?}",
                declist_item.sft_index(),
                declist_item.name(),
                declist_item.game_name(),
                sft_frame.group_name(),
                sft_frame.sprite_name()
            );
            lod_manager.sprite("pending").unwrap()
        };

        Some(Sprite {
            image,
            d_declist_item: declist_item.clone(),
            d_sft_frame: sft_frame.clone(),
        })
    }
}
