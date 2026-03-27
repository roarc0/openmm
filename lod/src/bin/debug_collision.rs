use lod::{LodManager, odm::Odm};

fn main() {
    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();
    let map = Odm::new(&lod_manager, "oute3.odm").unwrap();

    // Find a simple box-like model and print all its wall faces with full data
    for (mi, model) in map.bsp_models.iter().enumerate() {
        if model.faces.len() > 15 {
            continue; // Skip complex models, find simple ones
        }
        println!("=== Model {} '{}' faces={} verts={} ===",
            mi, model.header.name, model.faces.len(), model.vertices.len());

        // Print vertices in Bevy coords (already transformed by decode_vertices)
        for (vi, v) in model.vertices.iter().enumerate().take(10) {
            println!("  v{}: ({:.0}, {:.0}, {:.0})", vi, v[0], v[1], v[2]);
        }

        for (fi, face) in model.faces.iter().enumerate() {
            if face.vertices_count < 3 { continue; }
            let raw_nx = face.plane.normal[0];
            let raw_ny = face.plane.normal[1];
            let raw_nz = face.plane.normal[2];
            // Bevy normal: (mm6_x, mm6_z, -mm6_y) / 65536
            let bevy_nx = raw_nx as f32 / 65536.0;
            let bevy_ny = raw_nz as f32 / 65536.0;
            let bevy_nz = -(raw_ny as f32) / 65536.0;

            let vc = face.vertices_count as usize;
            let v0 = model.vertices[face.vertices_ids[0] as usize];

            // Verify normal direction: compute cross product from actual vertices
            let p0 = model.vertices[face.vertices_ids[0] as usize];
            let p1 = model.vertices[face.vertices_ids[1] as usize];
            let p2 = model.vertices[face.vertices_ids[2] as usize];
            let edge1 = [p1[0]-p0[0], p1[1]-p0[1], p1[2]-p0[2]];
            let edge2 = [p2[0]-p0[0], p2[1]-p0[1], p2[2]-p0[2]];
            let cross = [
                edge1[1]*edge2[2] - edge1[2]*edge2[1],
                edge1[2]*edge2[0] - edge1[0]*edge2[2],
                edge1[0]*edge2[1] - edge1[1]*edge2[0],
            ];
            let cross_len = (cross[0]*cross[0] + cross[1]*cross[1] + cross[2]*cross[2]).sqrt();
            let computed_n = if cross_len > 0.0 {
                [cross[0]/cross_len, cross[1]/cross_len, cross[2]/cross_len]
            } else {
                [0.0, 0.0, 0.0]
            };

            // Dot product between stored and computed normal
            let dot = bevy_nx*computed_n[0] + bevy_ny*computed_n[1] + bevy_nz*computed_n[2];

            println!("  Face {}: stored_n=({:.2},{:.2},{:.2}) computed_n=({:.2},{:.2},{:.2}) dot={:.2} v0=({:.0},{:.0},{:.0}) verts={}",
                fi, bevy_nx, bevy_ny, bevy_nz,
                computed_n[0], computed_n[1], computed_n[2],
                dot, v0[0], v0[1], v0[2], vc);
        }
        println!();
        if mi > 25 { break; }
    }
}
