use lod::{LodManager, odm::Odm};

fn main() {
    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();
    let map = Odm::new(&lod_manager, "oute3.odm").unwrap();
    
    let mut wall_count = 0;
    let mut floor_count = 0;
    let mut ceil_count = 0;
    let mut skip_count = 0;
    
    for model in &map.bsp_models {
        for face in &model.faces {
            if face.vertices_count < 3 || face.is_invisible() {
                skip_count += 1;
                continue;
            }
            let nx = face.plane.normal[0] as f32 / 65536.0;
            let ny = face.plane.normal[2] as f32 / 65536.0; // Bevy Y = MM6 Z
            let nz = -face.plane.normal[1] as f32 / 65536.0;
            
            if ny > 0.5 { floor_count += 1; }
            else if ny < -0.5 { ceil_count += 1; }
            if ny.abs() < 0.7 { wall_count += 1; }
        }
    }
    println!("Walls: {}, Floors: {}, Ceilings: {}, Skipped: {}", wall_count, floor_count, ceil_count, skip_count);
    
    // Print some sample wall face data to check if they make sense
    for (mi, model) in map.bsp_models.iter().enumerate().take(3) {
        println!("\nModel {} '{}' pos=({},{},{})", mi, model.header.name,
            model.header.position[0], model.header.position[1], model.header.position[2]);
        for (fi, face) in model.faces.iter().enumerate() {
            let ny = face.plane.normal[2] as f32 / 65536.0;
            if ny.abs() < 0.7 && face.vertices_count >= 3 {
                // This is a "wall" face - show its vertex positions
                let v0 = model.vertices[face.vertices_ids[0] as usize];
                let v1 = model.vertices[face.vertices_ids[1] as usize];
                println!("  Wall face {}: normal_y={:.2}, v0=({:.0},{:.0},{:.0}) v1=({:.0},{:.0},{:.0})",
                    fi, ny, v0[0], v0[1], v0[2], v1[0], v1[1], v1[2]);
                if fi > 3 { break; }
            }
        }
    }
}
