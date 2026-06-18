use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use flate2::Compression;
use flate2::write::GzEncoder;
use mvt::{GeomEncoder, GeomType, Tile as MvtTile};
use pmtiles2::{PMTiles, TileType, Compression as PmCompression};
use pmtiles2::util::tile_id;
use rustc_hash::{FxHashMap, FxHashSet};

use osmpbf::{Element, ElementReader};

// Progress Message enum to communicate back to the TUI main thread
#[allow(dead_code)]
pub enum ProgressMessage {
    DownloadStart { country: String },
    DownloadProgress { country: String, downloaded: u64, total: Option<u64> },
    DownloadComplete { country: String },
    
    Pass1Start { total_files: usize },
    Pass1Progress { file_index: usize, file_name: String, percentage: u8 },
    
    Pass2Start { total_files: usize },
    Pass2Progress { file_index: usize, file_name: String, percentage: u8 },
    
    TileGenStart,
    TileGenProgress { current: usize, total: usize },
    
    WritingStart,
    Complete { output_file: PathBuf, total_tiles: usize },
    
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WayType {
    Road { class: RoadClass },
    Water,
    Waterway,
    Poi,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoadClass {
    Motorway,
    Primary,
    Secondary,
    Tertiary,
    Unclassified,
    Residential,
    Minor,
}

impl RoadClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Motorway => "motorway",
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Tertiary => "tertiary",
            Self::Unclassified => "unclassified",
            Self::Residential => "residential",
            Self::Minor => "minor",
        }
    }
}

pub struct WayData {
    pub way_type: WayType,
    pub nodes: Vec<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

// Custom reader wrapper to monitor read progress of the PBF files
struct ProgressReader<R> {
    inner: R,
    bytes_read: u64,
    total_bytes: u64,
    last_reported_percent: u8,
    file_index: usize,
    file_name: String,
    tx: Sender<ProgressMessage>,
    is_pass1: bool,
}

impl<R: Read> Read for ProgressReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.bytes_read += n as u64;
        let percent = ((self.bytes_read as f64 / self.total_bytes as f64) * 100.0).min(100.0) as u8;
        if percent != self.last_reported_percent {
            self.last_reported_percent = percent;
            if self.is_pass1 {
                let _ = self.tx.send(ProgressMessage::Pass1Progress {
                    file_index: self.file_index,
                    file_name: self.file_name.clone(),
                    percentage: percent,
                });
            } else {
                let _ = self.tx.send(ProgressMessage::Pass2Progress {
                    file_index: self.file_index,
                    file_name: self.file_name.clone(),
                    percentage: percent,
                });
            }
        }
        Ok(n)
    }
}

// Convert longitude to fractional tile coordinate x at zoom z
pub fn lon_to_tile_x(lon: f64, z: u8) -> f64 {
    let n = 2.0f64.powi(z as i32);
    (lon + 180.0) / 360.0 * n
}

// Convert latitude to fractional tile coordinate y at zoom z
pub fn lat_to_tile_y(lat: f64, z: u8) -> f64 {
    let n = 2.0f64.powi(z as i32);
    let lat_rad = lat.to_radians();
    (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * n
}

fn classify_highway(val: &str) -> RoadClass {
    match val {
        "motorway" | "motorway_link" | "trunk" | "trunk_link" => RoadClass::Motorway,
        "primary" | "primary_link" => RoadClass::Primary,
        "secondary" | "secondary_link" => RoadClass::Secondary,
        "tertiary" | "tertiary_link" => RoadClass::Tertiary,
        "unclassified" => RoadClass::Unclassified,
        "residential" | "living_street" => RoadClass::Residential,
        _ => RoadClass::Minor,
    }
}

fn is_relevant_poi_tag(key: &str, val: &str, include_fuel: bool, include_charging: bool) -> bool {
    if key != "amenity" {
        return false;
    }
    if val == "fuel" && include_fuel {
        return true;
    }
    if val == "charging_station" && include_charging {
        return true;
    }
    false
}

fn get_way_type<'a>(
    tags: &mut impl Iterator<Item = (&'a str, &'a str)>,
    include_fuel: bool,
    include_charging: bool,
) -> Option<WayType> {
    let mut is_road = false;
    let mut highway_val = None;
    let mut has_natural_water = false;
    let mut has_landuse_reservoir = false;
    let mut has_waterway = false;
    let mut water_val = None;
    let mut is_poi = false;

    for (key, val) in tags {
        if key == "highway" {
            is_road = true;
            highway_val = Some(val);
        } else if key == "natural" && val == "water" {
            has_natural_water = true;
        } else if key == "landuse" && val == "reservoir" {
            has_landuse_reservoir = true;
        } else if key == "water" {
            water_val = Some(val);
        } else if key == "waterway" {
            has_waterway = true;
        } else if is_relevant_poi_tag(key, val, include_fuel, include_charging) {
            is_poi = true;
        }
    }

    if is_road {
        if let Some(val) = highway_val {
            return Some(WayType::Road { class: classify_highway(val) });
        }
    }

    // A waterway feature is either tagged with a "waterway" key,
    // or tagged with "natural=water" with a "water" sub-tag of river, canal, stream, ditch, lock, or riverbank.
    let is_waterway_feature = has_waterway || (has_natural_water && match water_val {
        Some("river") | Some("canal") | Some("stream") | Some("ditch") | Some("lock") | Some("riverbank") => true,
        _ => false,
    });

    if is_waterway_feature {
        return Some(WayType::Waterway);
    }
    if has_natural_water || has_landuse_reservoir {
        return Some(WayType::Water);
    }
    if is_poi {
        return Some(WayType::Poi);
    }
    None
}

fn is_relevant_node<'a>(
    tags: &mut impl Iterator<Item = (&'a str, &'a str)>,
    include_fuel: bool,
    include_charging: bool,
) -> bool {
    for (key, val) in tags {
        if is_relevant_poi_tag(key, val, include_fuel, include_charging) {
            return true;
        }
    }
    false
}

// Cohen-Sutherland line clipping helpers
const INSIDE: i32 = 0; // 0000
const LEFT: i32 = 1;   // 0001
const RIGHT: i32 = 2;  // 0010
const BOTTOM: i32 = 4; // 0100
const TOP: i32 = 8;    // 1000

fn compute_out_code(x: f64, y: f64, xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> i32 {
    let mut code = INSIDE;
    if x < xmin { code |= LEFT; }
    else if x > xmax { code |= RIGHT; }
    if y < ymin { code |= BOTTOM; }
    else if y > ymax { code |= TOP; }
    code
}

fn clip_segment(
    mut x0: f64, mut y0: f64, mut x1: f64, mut y1: f64,
    xmin: f64, ymin: f64, xmax: f64, ymax: f64
) -> Option<(f64, f64, f64, f64)> {
    let mut code0 = compute_out_code(x0, y0, xmin, ymin, xmax, ymax);
    let mut code1 = compute_out_code(x1, y1, xmin, ymin, xmax, ymax);
    loop {
        if (code0 | code1) == 0 {
            return Some((x0, y0, x1, y1));
        } else if (code0 & code1) != 0 {
            return None;
        } else {
            let code_out = if code0 != 0 { code0 } else { code1 };
            let mut x = 0.0;
            let mut y = 0.0;
            if (code_out & TOP) != 0 {
                x = x0 + (x1 - x0) * (ymax - y0) / (y1 - y0);
                y = ymax;
            } else if (code_out & BOTTOM) != 0 {
                x = x0 + (x1 - x0) * (ymin - y0) / (y1 - y0);
                y = ymin;
            } else if (code_out & RIGHT) != 0 {
                y = y0 + (y1 - y0) * (xmax - x0) / (x1 - x0);
                x = xmax;
            } else if (code_out & LEFT) != 0 {
                y = y0 + (y1 - y0) * (xmin - x0) / (x1 - x0);
                x = xmin;
            }
            if code_out == code0 {
                x0 = x; y0 = y;
                code0 = compute_out_code(x0, y0, xmin, ymin, xmax, ymax);
            } else {
                x1 = x; y1 = y;
                code1 = compute_out_code(x1, y1, xmin, ymin, xmax, ymax);
            }
        }
    }
}

pub fn clip_linestring(pts: &[Pt], xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> Vec<Vec<Pt>> {
    let mut lines = Vec::new();
    if pts.is_empty() { return lines; }
    let mut current_line = Vec::new();
    for i in 0..pts.len() - 1 {
        let p0 = pts[i];
        let p1 = pts[i+1];
        if let Some((cx0, cy0, cx1, cy1)) = clip_segment(p0.x, p0.y, p1.x, p1.y, xmin, ymin, xmax, ymax) {
            let cp0 = Pt { x: cx0, y: cy0 };
            let cp1 = Pt { x: cx1, y: cy1 };
            if current_line.is_empty() {
                current_line.push(cp0);
            } else {
                let last = current_line[current_line.len() - 1];
                if (last.x - cx0).abs() > 1e-5 || (last.y - cy0).abs() > 1e-5 {
                    lines.push(current_line);
                    current_line = vec![cp0];
                }
            }
            current_line.push(cp1);
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    lines
}

// Sutherland-Hodgman Polygon Clipping
pub fn clip_polygon(poly: &[Pt], xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> Vec<Pt> {
    if poly.is_empty() { return Vec::new(); }
    let mut current = poly.to_vec();
    current = clip_poly_edge(&current, xmin, false, true);
    current = clip_poly_edge(&current, xmax, false, false);
    current = clip_poly_edge(&current, ymin, true, true);
    current = clip_poly_edge(&current, ymax, true, false);
    current
}

fn clip_poly_edge(ring: &[Pt], edge_pos: f64, is_horizontal: bool, is_greater: bool) -> Vec<Pt> {
    let mut output = Vec::new();
    if ring.is_empty() { return output; }
    let is_inside = |p: Pt| {
        let val = if is_horizontal { p.y } else { p.x };
        if is_greater { val >= edge_pos } else { val <= edge_pos }
    };
    let intersection = |p1: Pt, p2: Pt| {
        if is_horizontal {
            let t = (edge_pos - p1.y) / (p2.y - p1.y);
            Pt { x: p1.x + t * (p2.x - p1.x), y: edge_pos }
        } else {
            let t = (edge_pos - p1.x) / (p2.x - p1.x);
            Pt { x: edge_pos, y: p1.y + t * (p2.y - p1.y) }
        }
    };
    let mut s = ring[ring.len() - 1];
    for &p in ring {
        if is_inside(p) {
            if !is_inside(s) {
                output.push(intersection(s, p));
            }
            output.push(p);
        } else if is_inside(s) {
            output.push(intersection(s, p));
        }
        s = p;
    }
    output
}

fn gzip_compress(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}

pub fn run_conversion(
    pbf_files: &[PathBuf],
    output_file: &Path,
    include_fuel: bool,
    include_charging: bool,
    bbox: Option<(u64, u64, u64, u64)>,
    progress_tx: Sender<ProgressMessage>
) -> Result<(), String> {
    // ----------------------------------------------------
    // PASS 1: Scan Ways and POI Nodes
    // ----------------------------------------------------
    let _ = progress_tx.send(ProgressMessage::Pass1Start { total_files: pbf_files.len() });
    
    let mut ways: FxHashMap<i64, WayData> = FxHashMap::default();
    let mut referenced_nodes: FxHashSet<i64> = FxHashSet::default();
    // Keep a list of POI points per tile (zoom 16)
    // Key: (tx, ty), Value: list of local coordinates
    let mut poi_points: FxHashMap<(u64, u64), Vec<Pt>> = FxHashMap::default();

    for (file_idx, path) in pbf_files.iter().enumerate() {
        let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let file = File::open(path).map_err(|e| format!("Failed to open PBF file: {}", e))?;
        let metadata = file.metadata().map_err(|e| format!("Failed to read PBF metadata: {}", e))?;
        let total_bytes = metadata.len();
        
        let preader = ProgressReader {
            inner: file,
            bytes_read: 0,
            total_bytes,
            last_reported_percent: 0,
            file_index: file_idx,
            file_name: file_name.clone(),
            tx: progress_tx.clone(),
            is_pass1: true,
        };
        let reader = ElementReader::new(BufReader::new(preader));
        
        reader.for_each(|element| {
            match element {
                Element::Node(node) => {
                    let mut tags = node.tags();
                    if is_relevant_node(&mut tags, include_fuel, include_charging) {
                        let tx = lon_to_tile_x(node.lon(), 16);
                        let ty = lat_to_tile_y(node.lat(), 16);
                        let tile_x = tx.floor() as u64;
                        let tile_y = ty.floor() as u64;
                        
                        if let Some((min_x, max_x, min_y, max_y)) = bbox {
                            if tile_x < min_x || tile_x > max_x || tile_y < min_y || tile_y > max_y {
                                return;
                            }
                        }
                        
                        let local_x = (tx - tile_x as f64) * 4096.0;
                        let local_y = (ty - tile_y as f64) * 4096.0;
                        
                        poi_points.entry((tile_x, tile_y))
                            .or_default()
                            .push(Pt { x: local_x, y: local_y });
                    }
                }
                Element::DenseNode(dense) => {
                    let mut tags = dense.tags();
                    if is_relevant_node(&mut tags, include_fuel, include_charging) {
                        let tx = lon_to_tile_x(dense.lon(), 16);
                        let ty = lat_to_tile_y(dense.lat(), 16);
                        let tile_x = tx.floor() as u64;
                        let tile_y = ty.floor() as u64;
                        
                        if let Some((min_x, max_x, min_y, max_y)) = bbox {
                            if tile_x < min_x || tile_x > max_x || tile_y < min_y || tile_y > max_y {
                                return;
                            }
                        }
                        
                        let local_x = (tx - tile_x as f64) * 4096.0;
                        let local_y = (ty - tile_y as f64) * 4096.0;
                        
                        poi_points.entry((tile_x, tile_y))
                            .or_default()
                            .push(Pt { x: local_x, y: local_y });
                    }
                }
                Element::Way(way) => {
                    let mut tags = way.tags();
                    if let Some(way_type) = get_way_type(&mut tags, include_fuel, include_charging) {
                        let nodes: Vec<i64> = way.refs().collect();
                        for &n_id in &nodes {
                            referenced_nodes.insert(n_id);
                        }
                        ways.insert(way.id(), WayData { way_type, nodes });
                    }
                }
                _ => {}
            }
        }).map_err(|e| format!("Error parsing PBF in Pass 1: {}", e))?;
    }

    // ----------------------------------------------------
    // PASS 2: Scan node coordinates for referenced nodes
    // ----------------------------------------------------
    let _ = progress_tx.send(ProgressMessage::Pass2Start { total_files: pbf_files.len() });
    let mut node_coords: FxHashMap<i64, Pt> = FxHashMap::default();

    for (file_idx, path) in pbf_files.iter().enumerate() {
        let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let file = File::open(path).map_err(|e| format!("Failed to open PBF file: {}", e))?;
        let metadata = file.metadata().map_err(|e| format!("Failed to read PBF metadata: {}", e))?;
        let total_bytes = metadata.len();
        
        let preader = ProgressReader {
            inner: file,
            bytes_read: 0,
            total_bytes,
            last_reported_percent: 0,
            file_index: file_idx,
            file_name: file_name.clone(),
            tx: progress_tx.clone(),
            is_pass1: false,
        };
        let reader = ElementReader::new(BufReader::new(preader));
        
        reader.for_each(|element| {
            match element {
                Element::Node(node) => {
                    if referenced_nodes.contains(&node.id()) {
                        node_coords.insert(node.id(), Pt { x: node.lon(), y: node.lat() });
                    }
                }
                Element::DenseNode(dense) => {
                    if referenced_nodes.contains(&dense.id()) {
                        node_coords.insert(dense.id(), Pt { x: dense.lon(), y: dense.lat() });
                    }
                }
                _ => {}
            }
        }).map_err(|e| format!("Error parsing PBF in Pass 2: {}", e))?;
    }

    // ----------------------------------------------------
    // PASS 3: Generate Tile Geometries
    // ----------------------------------------------------
    let _ = progress_tx.send(ProgressMessage::TileGenStart);
    
    // Grouped geometries per tile (zoom 16)
    // Key: (tx, ty) -> TileGeometries
    struct TileGeometries {
        roads: Vec<(RoadClass, Vec<Vec<Pt>>)>,
        water_polys: Vec<Vec<Pt>>,
        water_lines: Vec<Vec<Pt>>,
    }
    
    let mut tile_geoms: FxHashMap<(u64, u64), TileGeometries> = FxHashMap::default();
    
    let total_ways = ways.len();
    let mut ways_processed = 0;

    for (_, way_data) in ways {
        ways_processed += 1;
        if ways_processed % 50000 == 0 {
            let _ = progress_tx.send(ProgressMessage::TileGenProgress {
                current: ways_processed,
                total: total_ways,
            });
        }
        
        // Resolve way coordinates
        let mut pts = Vec::with_capacity(way_data.nodes.len());
        let mut missing = false;
        for &node_id in &way_data.nodes {
            if let Some(&pt) = node_coords.get(&node_id) {
                pts.push(pt);
            } else {
                missing = true;
                break;
            }
        }
        
        if missing || pts.is_empty() {
            continue;
        }
        
        // Convert to fractional tile coordinates at zoom 16
        let tile_pts: Vec<Pt> = pts.iter().map(|p| Pt {
            x: lon_to_tile_x(p.x, 16),
            y: lat_to_tile_y(p.y, 16),
        }).collect();
        
        // Find bounding box in tile coordinates
        let mut min_tx = tile_pts[0].x.floor() as u64;
        let mut max_tx = min_tx;
        let mut min_ty = tile_pts[0].y.floor() as u64;
        let mut max_ty = min_ty;
        
        for &p in &tile_pts[1..] {
            let tx = p.x.floor() as u64;
            let ty = p.y.floor() as u64;
            min_tx = min_tx.min(tx);
            max_tx = max_tx.max(tx);
            min_ty = min_ty.min(ty);
            max_ty = max_ty.max(ty);
        }
        
        if let Some((min_x, max_x, min_y, max_y)) = bbox {
            if max_tx < min_x || min_tx > max_x || max_ty < min_y || min_ty > max_y {
                continue;
            }
        }
        
        // Add to intersecting tiles
        for tx in min_tx..=max_tx {
            for ty in min_ty..=max_ty {
                if let Some((min_x, max_x, min_y, max_y)) = bbox {
                    if tx < min_x || tx > max_x || ty < min_y || ty > max_y {
                        continue;
                    }
                }
                // Bounds in local coordinates for tile (tx, ty)
                // We add a 256 unit buffer (extent is 4096) to prevent edge cutoffs
                let local_pts: Vec<Pt> = tile_pts.iter().map(|p| Pt {
                    x: (p.x - tx as f64) * 4096.0,
                    y: (p.y - ty as f64) * 4096.0,
                }).collect();
                
                let xmin = -256.0;
                let xmax = 4096.0 + 256.0;
                let ymin = -256.0;
                let ymax = 4096.0 + 256.0;
                
                match way_data.way_type {
                    WayType::Road { class } => {
                        let clipped_lines = clip_linestring(&local_pts, xmin, ymin, xmax, ymax);
                        if !clipped_lines.is_empty() {
                            let entry = tile_geoms.entry((tx, ty)).or_insert_with(|| TileGeometries {
                                roads: Vec::new(),
                                water_polys: Vec::new(),
                                water_lines: Vec::new(),
                            });
                            entry.roads.push((class, clipped_lines));
                        }
                    }
                    WayType::Water => {
                        let clipped_poly = clip_polygon(&local_pts, xmin, ymin, xmax, ymax);
                        if clipped_poly.len() >= 3 {
                            let entry = tile_geoms.entry((tx, ty)).or_insert_with(|| TileGeometries {
                                roads: Vec::new(),
                                water_polys: Vec::new(),
                                water_lines: Vec::new(),
                            });
                            entry.water_polys.push(clipped_poly);
                        }
                    }
                    WayType::Waterway => {
                        let clipped_lines = clip_linestring(&local_pts, xmin, ymin, xmax, ymax);
                        if !clipped_lines.is_empty() {
                            for line in clipped_lines {
                                let entry = tile_geoms.entry((tx, ty)).or_insert_with(|| TileGeometries {
                                    roads: Vec::new(),
                                    water_polys: Vec::new(),
                                    water_lines: Vec::new(),
                                });
                                entry.water_lines.push(line);
                            }
                        }
                    }
                    WayType::Poi => {
                        // POI areas: take centroid as a POI point
                        let mut sum_x = 0.0;
                        let mut sum_y = 0.0;
                        for p in &local_pts {
                            sum_x += p.x;
                            sum_y += p.y;
                        }
                        let cx = sum_x / local_pts.len() as f64;
                        let cy = sum_y / local_pts.len() as f64;
                        if cx >= xmin && cx <= xmax && cy >= ymin && cy <= ymax {
                            poi_points.entry((tx, ty))
                                .or_default()
                                .push(Pt { x: cx, y: cy });
                        }
                    }
                }
            }
        }
    }
    
    let _ = progress_tx.send(ProgressMessage::TileGenProgress {
        current: total_ways,
        total: total_ways,
    });

    // ----------------------------------------------------
    // PASS 4: Compile Tiles & Write PMTiles Archive
    // ----------------------------------------------------
    let _ = progress_tx.send(ProgressMessage::WritingStart);
    
    let mut pm_tiles = PMTiles::new(TileType::Mvt, PmCompression::GZip);
    
    // Find all unique tile coordinates that contain roads, water, or POIs
    let mut all_tiles: FxHashSet<(u64, u64)> = FxHashSet::default();
    for &k in tile_geoms.keys() {
        all_tiles.insert(k);
    }
    for &k in poi_points.keys() {
        all_tiles.insert(k);
    }
    
    let _total_tiles = all_tiles.len();
    let mut tiles_written = 0;
    
    for (tx, ty) in all_tiles {
        let mut mvt_tile = MvtTile::new(4096);
        
        let has_geom = tile_geoms.contains_key(&(tx, ty));
        let has_pois = poi_points.contains_key(&(tx, ty));
        
        if !has_geom && !has_pois {
            continue;
        }
        
        // 1. Add Water Layer
        if let Some(geoms) = tile_geoms.get(&(tx, ty)) {
            if !geoms.water_polys.is_empty() || !geoms.water_lines.is_empty() {
                let mut layer = mvt_tile.create_layer("water");
                // Add polygons
                for poly in &geoms.water_polys {
                    let mut encoder = GeomEncoder::new(GeomType::Polygon);
                    for p in poly {
                        encoder = encoder.point(p.x, p.y).unwrap();
                    }
                    if let Ok(geom_data) = encoder.encode() {
                        let feature = layer.into_feature(geom_data);
                        layer = feature.into_layer();
                    }
                }
                // Add linestrings (waterways)
                for line in &geoms.water_lines {
                    let mut encoder = GeomEncoder::new(GeomType::Linestring);
                    for p in line {
                        encoder = encoder.point(p.x, p.y).unwrap();
                    }
                    if let Ok(geom_data) = encoder.encode() {
                        let feature = layer.into_feature(geom_data);
                        layer = feature.into_layer();
                    }
                }
                let _ = mvt_tile.add_layer(layer);
            }
        }
        
        // 2. Add Roads Layer
        if let Some(geoms) = tile_geoms.get(&(tx, ty)) {
            if !geoms.roads.is_empty() {
                let mut layer = mvt_tile.create_layer("roads");
                for (class, lines) in &geoms.roads {
                    for line in lines {
                        let mut encoder = GeomEncoder::new(GeomType::Linestring);
                        for p in line {
                            encoder = encoder.point(p.x, p.y).unwrap();
                        }
                        if let Ok(geom_data) = encoder.encode() {
                            let mut feature = layer.into_feature(geom_data);
                            feature.add_tag_string("class", class.as_str());
                            layer = feature.into_layer();
                        }
                    }
                }
                let _ = mvt_tile.add_layer(layer);
            }
        }
        
        // 3. Add POI Layer
        if let Some(pois) = poi_points.get(&(tx, ty)) {
            if !pois.is_empty() {
                let mut layer = mvt_tile.create_layer("poi");
                for p in pois {
                    let encoder = GeomEncoder::new(GeomType::Point);
                    if let Ok(geom_data) = encoder.point(p.x, p.y).unwrap().encode() {
                        let feature = layer.into_feature(geom_data);
                        layer = feature.into_layer();
                    }
                }
                let _ = mvt_tile.add_layer(layer);
            }
        }
        
        // Encode MVT and Gzip compress it
        if let Ok(mvt_bytes) = mvt_tile.to_bytes() {
            if let Ok(compressed) = gzip_compress(&mvt_bytes) {
                // Add to PMTiles builder
                let _ = pm_tiles.add_tile(tile_id(16, tx, ty), compressed);
                tiles_written += 1;
            }
        }
    }
    
    // Write PMTiles to file
    let file = File::create(output_file).map_err(|e| format!("Failed to create output PMTiles file: {}", e))?;
    let mut writer = BufWriter::new(file);
    pm_tiles.to_writer(&mut writer).map_err(|e| format!("Failed to write PMTiles archive: {}", e))?;
    
    let _ = progress_tx.send(ProgressMessage::Complete {
        output_file: output_file.to_path_buf(),
        total_tiles: tiles_written,
    });
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monaco_conversion() {
        let pbf_path = PathBuf::from("../data/tmp/monaco-latest.osm.pbf");
        if !pbf_path.exists() {
            // Skip the test if Monaco PBF wasn't downloaded (e.g. inside CI without network)
            return;
        }
        let out_path = PathBuf::from("../data/tmp/monaco_test.pmtiles");
        let (tx, rx) = std::sync::mpsc::channel();
        
        let handle = std::thread::spawn(move || {
            while let Ok(_) = rx.recv() {}
        });
        
        let result = run_conversion(&[pbf_path], &out_path, true, true, None, tx);
        assert!(result.is_ok(), "Conversion failed: {:?}", result);
        assert!(out_path.exists(), "Output file does not exist");
        
        let file_meta = std::fs::metadata(&out_path).unwrap();
        assert!(file_meta.len() > 0, "Output file is empty");
        
        let _ = std::fs::remove_file(out_path);
        let _ = handle.join();
    }
}

