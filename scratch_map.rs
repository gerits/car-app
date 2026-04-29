use pmtiles2::PMTiles;
use std::fs::File;
use std::io::Read;

fn main() {
    let mut file = File::open("assets/map.mbtiles").unwrap();
    let pm = PMTiles::from_reader(file).unwrap();
    let tile = pm.get_tile(14, 8439, 5490).unwrap();
    
    let mut d = flate2::read::GzDecoder::new(tile.data.as_slice());
    let mut decompressed = Vec::new();
    if d.read_to_end(&mut decompressed).is_ok() {
        println!("Decompressed MVT size: {}", decompressed.len());
    } else {
        println!("Tile not gzipped, size: {}", tile.data.len());
        decompressed = tile.data.to_vec();
    }
}
