#[derive(Debug)]
pub struct DecList {
    entries: Vec<DecListEntry>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
struct DecListEntry {
    name: String,
    gameName: String,
    dec_type: u16,
    height: u16,
    radius: u16,
    light_radius: u16,
    sft: SFTData,
    bits: u16, //  bool noBlockMovement,noDraw,flickerSlow,flickerMedium,flickerFast,marker,slowLoop,emitFire,soundOnDawn,soundOnDusk,emitSmoke
    sound_id: u16,
    // skip 2 bytes
}

union SFTData {
    sft_group: [u8; 2],
    sft_index: i16,
}

impl DecList {
    pub fn new(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut entries = Vec::new();

        let mut cursor = Cursor::new(data);
        let count = cursor.read_u32::<LittleEndian>()?;
        for _i in 0..count {
            let name = read_string_block(cursor, 32);
            let gameName = read_string_block(cursor, 32);
            let dec_type = cursor.read_u16::<LittleEndian>()?;
            let height = cursor.read_u16::<LittleEndian>()?;
            let radius = cursor.read_u16::<LittleEndian>()?;
            let light_radius = cursor.read_u16::<LittleEndian>()?;
            //let mut sft:SFTData = SFTData::default
//cursor.read_exact(&sft)
            ///let dec_type = cursor.read_u16::<LittleEndian>()?;
        }

        Self { entries }
    }
}

mod tests {

    #[test]
    fn read_declist_data_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();

        let declist =
            LodData::try_from(lod_manager.try_get_bytes("icons/declist.bin").unwrap()).unwrap();
        let dtile = DecList::new(dtile.data.as_slice());

        let tile_table = dtile.table(map.tile_data).unwrap();
    }
}
