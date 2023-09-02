use std::{
    error::Error,
    io::{Cursor, Read},
};

use image::{DynamicImage, GenericImageView};

use crate::{
    ddeclist::{DDecList, DDecListItem},
    dsft::{DSFTFrame, DSFT},
    utils::try_read_string_block,
    LodManager,
};

#[repr(C)]
#[derive(Default, Debug)]
pub struct BillboardData {
    pub declist_id: u16,
    pub attributes: u16,
    pub position: [i32; 3],
    pub direction: i32,
    pub event_variable: i16,
    pub event: i16,
    pub trigger_radius: i16,
    pub direction_degrees: i16,
}

impl BillboardData {
    pub fn is_triggered_by_touch(&self) -> bool {
        (self.attributes & 0x0001) != 0
    }

    pub fn is_triggered_by_monster(&self) -> bool {
        (self.attributes & 0x0002) != 0
    }

    pub fn is_triggered_by_object(&self) -> bool {
        (self.attributes & 0x0004) != 0
    }

    pub fn is_shown_on_map(&self) -> bool {
        (self.attributes & 0x0010) != 0
    }

    pub fn is_chest(&self) -> bool {
        (self.attributes & 0x0020) != 0
    }

    pub fn is_invisible(&self) -> bool {
        (self.attributes & 0x0040) != 0
    }

    pub fn is_ship(&self) -> bool {
        (self.attributes & 0x0080) != 0
    }
}

#[derive(Default, Debug)]
pub struct Billboard {
    pub declist_name: String,
    pub data: BillboardData,
}

pub(super) fn read_billboards(
    cursor: &mut Cursor<&[u8]>,
    count: usize,
) -> Result<Vec<Billboard>, Box<dyn Error>> {
    let mut billboards_data = Vec::new();

    for _i in 0..count {
        let size = std::mem::size_of::<BillboardData>();
        let mut entity_data = BillboardData::default();
        cursor.read_exact(unsafe {
            std::slice::from_raw_parts_mut(&mut entity_data as *mut _ as *mut u8, size)
        })?;
        billboards_data.push(entity_data);
    }

    let mut billboard_names = Vec::new();
    for _i in 0..count {
        let name = try_read_string_block(cursor, 32);
        billboard_names.push(name?.to_lowercase());
    }

    let billboards = billboards_data
        .into_iter()
        .zip(billboard_names)
        .map(|(data, name)| Billboard {
            declist_name: name,
            data,
        })
        .collect();

    Ok(billboards)
}

pub struct BillboardManager {
    d_declist: DDecList,
    d_sft: DSFT,
}

pub struct BillboardSprite {
    pub image: DynamicImage,
    pub d_declist_item: DDecListItem,
    pub d_sft_frame: DSFTFrame,
}

impl BillboardSprite {
    pub fn dimensions(&self) -> (f32, f32) {
        let dimensions = self.image.dimensions();

        let height = dimensions.1 as f32;
        // if sprite.d_declist_item.height != 0 {
        //     size[0]
        //     sprite.d_declist_item.height as f32 * size[0] / 30.0
        // } else {
        //     size[0]
        // };
        let width = height * (dimensions.0 as f32 / dimensions.1 as f32);
        (width, height)
    }
}

impl BillboardManager {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let d_declist = DDecList::new(lod_manager)?;
        let d_sft = DSFT::new(lod_manager)?;
        Ok(Self { d_declist, d_sft })
    }

    pub fn get(
        &self,
        lod_manager: &LodManager,
        name: &str,
        declist_id: u16,
    ) -> Option<BillboardSprite> {
        let declist_item = self.d_declist.items.get(declist_id as usize)?;
        let sft_frame = self.d_sft.frames.get(declist_item.sft_index() as usize)?;

        let image = if let Some(image) = lod_manager.sprite(&declist_item.name()?) {
            image
        } else if let Some(image) = lod_manager.sprite(&sft_frame.sprite_name()?) {
            image
        } else if let Some(image) = lod_manager.sprite(name) {
            image
        } else {
            dbg!(format!(
                "failed to read entity: id:{}|name:{:?}|game_name:{:?}, sft group_name:{:?}|sprite_name:{:?}",
                declist_item.sft_index(),
                declist_item.name(),
                declist_item.game_name(),
                sft_frame.group_name(),
                sft_frame.sprite_name()
            ));
            lod_manager.sprite("pending").unwrap()
        };

        Some(BillboardSprite {
            image,
            d_declist_item: declist_item.clone(),
            d_sft_frame: sft_frame.clone(),
        })
    }
}
