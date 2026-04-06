use openmm_data::{LodManager, odm::Odm};

fn main() {
    let lod_manager = LodManager::new(openmm_data::get_data_path()).unwrap();
    let map = Odm::load(&lod_manager, "oute3.odm").unwrap();

    for (mi, model) in map.bsp_models.iter().enumerate().take(5) {
        println!("=== Model {} '{}' ===", mi, model.header.name);
        println!(
            "  texture_names[{}]: {:?}",
            model.texture_names.len(),
            model.texture_names
        );
        println!("  vertices: {}, faces: {}", model.vertices.len(), model.faces.len());

        for (fi, face) in model.faces.iter().enumerate().take(4) {
            let vc = face.vertices_count as usize;
            println!(
                "  Face {}: tex_id={}, verts={}, tex_u={}, tex_v={}",
                fi, face.texture_id, vc, face.texture_u, face.texture_v
            );
            println!("    vert_ids: {:?}", &face.vertices_ids[..vc]);
            println!("    u_ids:    {:?}", &face.texture_u_ids[..vc]);
            println!("    v_ids:    {:?}", &face.texture_v_ids[..vc]);
            println!(
                "    attrs: 0x{:08x}, invisible={}",
                face.attributes,
                face.is_invisible()
            );

            let tex_idx = face.texture_id as usize;
            if tex_idx < model.texture_names.len() {
                let name = &model.texture_names[tex_idx];
                if let Some(img) = lod_manager.game().bitmap(name) {
                    println!("    texture '{}': {}x{}", name, img.width(), img.height());
                    // Show what UVs would be
                    let w = img.width() as f32;
                    let h = img.height() as f32;
                    for i in 0..vc {
                        let u = (face.texture_u_ids[i] as f32 + face.texture_u as f32) / w;
                        let v = (face.texture_v_ids[i] as f32 + face.texture_v as f32) / h;
                        println!(
                            "      v{}: raw_u={} raw_v={} -> uv=({:.3}, {:.3})",
                            i, face.texture_u_ids[i], face.texture_v_ids[i], u, v
                        );
                    }
                } else {
                    println!("    texture '{}': NOT FOUND", name);
                }
            }
        }
        println!();
    }
}
