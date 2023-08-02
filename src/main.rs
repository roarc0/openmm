use std::error::Error;

mod image;
mod lod;
mod odm6;
mod raw;
mod sprite;
mod utils;

fn main() {
    let filename = "/home/roarc/games.lod";
    let lod = lod::Lod::open(filename);
    match lod {
        Ok(lod) => {
            match lod.version {
                lod::Version::MM6 => println!("Game Version: MM6"),
                lod::Version::MM7 => println!("Game Version: MM7"),
                lod::Version::MM8 => println!("Game Version: MM8"),
            }
            let lod_files = lod.files();
            println!("{:?}", lod_files);

            // for f in lod_files {
            //     match dump_image_to_file(&lod, f) {
            //         Ok(()) => println!("Saving {}", f),
            //         Err(e) => println!("Error saving {}: {}", f, e),
            //     }
            // }

            let map = "Outa1.odm";
            let o = lod.get::<raw::Raw>(map);
            if let Ok(o) = o {
                o.to_file(map).unwrap();
            } else {
                println!("failed to open file");
                return;
            };

            let odm = odm6::Odm6::try_from(lod.get::<raw::Raw>(map).unwrap()).unwrap();
            println!("{:?}", odm);
        }
        Err(err) => eprintln!("Error: {}", err),
    }
}

fn dump_image_to_file(lod: &lod::Lod, name: &str) -> Result<(), Box<dyn Error>> {
    lod.get::<image::Image>(name)?
        .to_png_file(&format!("{}.png", name))?;
    Ok(())
}
