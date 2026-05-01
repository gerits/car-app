use geo_types::Geometry;
use log::error;
use lru::LruCache;
use mvt_reader::feature::Value;
use pmtiles2::PMTiles;
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::cell::RefCell;
use std::fs::File;
use std::io::Read;
use std::num::NonZeroUsize;
use tiny_skia::{Color, Paint, PathBuilder, PixmapMut, Stroke, Transform};

use std::sync::Arc;

struct TilePaths {
    water_fill: Option<tiny_skia::Path>,
    water_stroke: Option<tiny_skia::Path>,
    road_major: Option<tiny_skia::Path>,
    road_minor: Option<tiny_skia::Path>,
    poi_point: Option<tiny_skia::Path>,
    poi_area: Option<tiny_skia::Path>,
}

thread_local! {
    static PM: RefCell<Option<PMTiles<File>>> = RefCell::new({
        match File::open("assets/map.mbtiles") {
            Ok(f) => PMTiles::from_reader(f).ok(),
            Err(e) => {
                error!("Failed to open map asset: {}", e);
                None
            }
        }
    });
    static TILE_CACHE: RefCell<LruCache<(u64, u64, u8), Arc<TilePaths>>> = RefCell::new(
        LruCache::new(NonZeroUsize::new(128).unwrap())
    );
}

pub struct MapView {
    pub center_x: u64,
    pub center_y: u64,
    pub zoom: u8,
}

pub fn render_map(
    offset_x: f32,
    offset_y: f32,
    width: u32,
    height: u32,
    view: &MapView,
    is_dark: bool,
) -> SharedPixelBuffer<Rgba8Pixel> {
    if width == 0 || height == 0 {
        return SharedPixelBuffer::<Rgba8Pixel>::new(1, 1);
    }

    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    let mut pixmap = {
        let pixels = buffer.make_mut_slice();
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(pixels.as_mut_ptr() as *mut u8, pixels.len() * 4)
        };
        PixmapMut::from_bytes(bytes, width, height).unwrap()
    };

    // NFS_COLOR_MAP_LAND: Dark: 0x1C1C1C, Light: 0xE8E8E8
    if is_dark {
        pixmap.fill(Color::from_rgba8(0x1C, 0x1C, 0x1C, 255));
    } else {
        pixmap.fill(Color::from_rgba8(0xE8, 0xE8, 0xE8, 255));
    }

    let scale = (width as f32 / 800.0).max(height as f32 / 480.0);

    // NFS_COLOR_MAP_WATER: Dark: 0x050505, Light: 0xC4D3DF
    let mut paint_water = Paint::default();
    if is_dark {
        paint_water.set_color_rgba8(0x05, 0x05, 0x05, 255);
    } else {
        paint_water.set_color_rgba8(0xC4, 0xD3, 0xDF, 255);
    }
    paint_water.anti_alias = true;
    let stroke_water = Stroke {
        width: 32.0, // 1.0 at scale 1 (128px tile) = 1.0 * (4096/128)
        ..Default::default()
    };

    // NFS_COLOR_MAP_ROAD_MAJOR: Dark: 0x353535, Light: 0xFFFFFF
    let mut paint_road_major = Paint::default();
    if is_dark {
        paint_road_major.set_color_rgba8(0x35, 0x35, 0x35, 255);
    } else {
        paint_road_major.set_color_rgba8(0xFF, 0xFF, 0xFF, 255);
    }
    paint_road_major.anti_alias = true;
    let stroke_road_major = Stroke {
        width: 64.0, // 2.0 at scale 1
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };

    // NFS_COLOR_MAP_ROAD_MINOR: Dark: 0x1C1B1B, Light: 0xF5F5F5
    let mut paint_road_minor = Paint::default();
    if is_dark {
        paint_road_minor.set_color_rgba8(0x1C, 0x1B, 0x1B, 255);
    } else {
        paint_road_minor.set_color_rgba8(0xF5, 0xF5, 0xF5, 255);
    }
    paint_road_minor.anti_alias = true;
    let stroke_road_minor = Stroke {
        width: 32.0, // 1.0 at scale 1
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };

    // POI Primary: Dark: 0xC9C7B8 (Cream), Light: 0x353535 (Tech Grey)
    let mut paint_poi = Paint::default();
    if is_dark {
        paint_poi.set_color_rgba8(0xC9, 0xC7, 0xB8, 255);
    } else {
        paint_poi.set_color_rgba8(0x35, 0x35, 0x35, 255);
    }
    paint_poi.anti_alias = true;

    let center_car_x = width as f32 / 2.0;
    let center_car_y = height as f32 / 2.0;

    PM.with(|pm_ref| {
        if let Some(pm) = pm_ref.borrow_mut().as_mut() {
            let tile_size = (512.0 / 4.0) * scale; // 128px tiles at simulated Z=14

            // Center relative to focal point fractional offsets at Z=16
            let start_x = center_car_x - (0.0061724 * tile_size);
            let start_y = center_car_y - (0.5817041 * tile_size);

            let scaled_offset_x = offset_x * scale;
            let scaled_offset_y = offset_y * scale;

            let min_dx = ((-scaled_offset_x - tile_size - start_x) / tile_size).floor() as i32 - 1;
            let max_dx =
                ((width as f32 - scaled_offset_x - start_x) / tile_size).floor() as i32 + 1;

            let min_dy = ((-scaled_offset_y - tile_size - start_y) / tile_size).floor() as i32 - 1;
            let max_dy =
                ((height as f32 - scaled_offset_y - start_y) / tile_size).floor() as i32 + 1;

            for dx in min_dx..=max_dx {
                for dy in min_dy..=max_dy {
                    let tile_x = (view.center_x as i32 + dx) as u64;
                    let tile_y = (view.center_y as i32 + dy) as u64;
                    let zoom = view.zoom;

                    let screen_origin_x = scaled_offset_x + start_x + (dx as f32 * tile_size);
                    let screen_origin_y = scaled_offset_y + start_y + (dy as f32 * tile_size);
                    let transform = Transform::from_translate(screen_origin_x, screen_origin_y);

                    let cached_tile = TILE_CACHE
                        .with(|cache| cache.borrow_mut().get(&(tile_x, tile_y, zoom)).cloned());

                    let tile_paths = if let Some(tile) = cached_tile {
                        tile
                    } else {
                        let mut new_paths = TilePaths {
                            water_fill: None,
                            water_stroke: None,
                            road_major: None,
                            road_minor: None,
                            poi_point: None,
                            poi_area: None,
                        };

                        if let Ok(Some(tile_data)) = pm.get_tile(tile_x, tile_y, zoom) {
                            let mut d = flate2::read::GzDecoder::new(tile_data.as_slice());
                            let mut decompressed = Vec::new();
                            if d.read_to_end(&mut decompressed).is_ok()
                                && let Ok(reader) = mvt_reader::Reader::new(decompressed) {
                                    let mut water_fill_pb = PathBuilder::new();
                                    let mut water_stroke_pb = PathBuilder::new();
                                    let mut road_major_pb = PathBuilder::new();
                                    let mut road_minor_pb = PathBuilder::new();
                                    let mut poi_point_pb = PathBuilder::new();
                                    let mut poi_area_pb = PathBuilder::new();

                                    let layers = reader.get_layer_names().unwrap_or_default();
                                    for (i, layer_name) in layers.iter().enumerate() {
                                        if layer_name.contains("water") {
                                            if let Ok(features) = reader.get_features(i) {
                                                for f in features {
                                                    match f.get_geometry() {
                                                        Geometry::Polygon(poly) => {
                                                            add_polygon_to_pb(
                                                                &mut water_fill_pb,
                                                                poly.exterior().0.as_slice(),
                                                            )
                                                        }
                                                        Geometry::MultiPolygon(mpoly) => {
                                                            for poly in mpoly {
                                                                add_polygon_to_pb(
                                                                    &mut water_fill_pb,
                                                                    poly.exterior().0.as_slice(),
                                                                );
                                                            }
                                                        }
                                                        Geometry::LineString(ls) => {
                                                            add_linestring_to_pb(
                                                                &mut water_stroke_pb,
                                                                ls.0.as_slice(),
                                                            )
                                                        }
                                                        Geometry::MultiLineString(mls) => {
                                                            for ls in mls {
                                                                add_linestring_to_pb(
                                                                    &mut water_stroke_pb,
                                                                    ls.0.as_slice(),
                                                                );
                                                            }
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        } else if layer_name == "roads"
                                            || layer_name == "transportation"
                                            || layer_name == "highway"
                                            || layer_name.contains("road")
                                        {
                                            if let Ok(features) = reader.get_features(i) {
                                                for f in features {
                                                    let mut is_major = false;
                                                    if let Some(properties) = &f.properties
                                                        && let Some(Value::String(val)) =
                                                            properties.get("class")
                                                            && (val == "secondary"
                                                                || val == "unclassified"
                                                                || val == "primary"
                                                                || val == "residential"
                                                                || val == "motorway")
                                                            {
                                                                is_major = true;
                                                            }
                                                    let pb = if is_major {
                                                        &mut road_major_pb
                                                    } else {
                                                        &mut road_minor_pb
                                                    };
                                                    match f.get_geometry() {
                                                        Geometry::LineString(ls) => {
                                                            add_linestring_to_pb(
                                                                pb,
                                                                ls.0.as_slice(),
                                                            )
                                                        }
                                                        Geometry::MultiLineString(mls) => {
                                                            for ls in mls {
                                                                add_linestring_to_pb(
                                                                    pb,
                                                                    ls.0.as_slice(),
                                                                );
                                                            }
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        } else if (layer_name.contains("poi")
                                            || layer_name == "place")
                                            && let Ok(features) = reader.get_features(i) {
                                                for f in features {
                                                    match f.get_geometry() {
                                                        Geometry::Point(pt) => {
                                                            poi_point_pb.push_circle(
                                                                pt.x(),
                                                                pt.y(),
                                                                2.5 * (4096.0 / 128.0),
                                                            );
                                                        }
                                                        Geometry::MultiPoint(mp) => {
                                                            for pt in mp {
                                                                poi_point_pb.push_circle(
                                                                    pt.x(),
                                                                    pt.y(),
                                                                    2.5 * (4096.0 / 128.0),
                                                                );
                                                            }
                                                        }
                                                        Geometry::Polygon(poly) => {
                                                            add_polygon_to_pb(
                                                                &mut poi_area_pb,
                                                                poly.exterior().0.as_slice(),
                                                            )
                                                        }
                                                        Geometry::MultiPolygon(mpoly) => {
                                                            for poly in mpoly {
                                                                add_polygon_to_pb(
                                                                    &mut poi_area_pb,
                                                                    poly.exterior().0.as_slice(),
                                                                );
                                                            }
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                    }
                                    new_paths.water_fill = water_fill_pb.finish();
                                    new_paths.water_stroke = water_stroke_pb.finish();
                                    new_paths.road_major = road_major_pb.finish();
                                    new_paths.road_minor = road_minor_pb.finish();
                                    new_paths.poi_point = poi_point_pb.finish();
                                    new_paths.poi_area = poi_area_pb.finish();
                                }
                        }
                        let arc_tile = Arc::new(new_paths);
                        TILE_CACHE.with(|cache| {
                            cache
                                .borrow_mut()
                                .put((tile_x, tile_y, zoom), arc_tile.clone());
                        });
                        arc_tile
                    };

                    let final_transform =
                        transform.pre_scale(tile_size / 4096.0, tile_size / 4096.0);

                    if let Some(path) = &tile_paths.water_fill {
                        pixmap.fill_path(
                            path,
                            &paint_water,
                            tiny_skia::FillRule::Winding,
                            final_transform,
                            None,
                        );
                    }
                    if let Some(path) = &tile_paths.water_stroke {
                        pixmap.stroke_path(
                            path,
                            &paint_water,
                            &stroke_water,
                            final_transform,
                            None,
                        );
                    }
                    if let Some(path) = &tile_paths.road_minor {
                        pixmap.stroke_path(
                            path,
                            &paint_road_minor,
                            &stroke_road_minor,
                            final_transform,
                            None,
                        );
                    }
                    if let Some(path) = &tile_paths.road_major {
                        pixmap.stroke_path(
                            path,
                            &paint_road_major,
                            &stroke_road_major,
                            final_transform,
                            None,
                        );
                    }
                    if let Some(path) = &tile_paths.poi_area {
                        let mut area_paint = paint_poi.clone();
                        if is_dark {
                            area_paint.set_color_rgba8(0xC9, 0xC7, 0xB8, 100);
                        } else {
                            area_paint.set_color_rgba8(0x35, 0x35, 0x35, 100);
                        }
                        pixmap.fill_path(
                            path,
                            &area_paint,
                            tiny_skia::FillRule::Winding,
                            final_transform,
                            None,
                        );
                    }
                    if let Some(path) = &tile_paths.poi_point {
                        pixmap.fill_path(
                            path,
                            &paint_poi,
                            tiny_skia::FillRule::Winding,
                            final_transform,
                            None,
                        );
                    }
                }
            }
        }
    });

    let min_dim = width.min(height) as f32;
    let radius_max = min_dim / 2.0;
    let radius_outer = radius_max * 0.70;
    let radius_inner = radius_max * 0.65;

    // --- Draw Dashboard Rings (as done in car-app) ---

    // 1. Outer Ring (70% - 100% radius) - Lighter dark zone for speedometer
    let mut pb_outer = PathBuilder::new();
    pb_outer.push_circle(center_car_x, center_car_y, radius_max);
    pb_outer.push_circle(center_car_x, center_car_y, radius_outer);
    if let Some(path) = pb_outer.finish() {
        let mut paint = Paint::default();
        if is_dark {
            paint.set_color_rgba8(0x0A, 0x0A, 0x0A, 180);
        } else {
            paint.set_color_rgba8(0xFF, 0xFF, 0xFF, 120); // Lighter semi-transparent ring
        }
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::EvenOdd,
            Transform::identity(),
            None,
        );
    }

    // 2. Inner Ring (65% - 70% radius) - Vertical depth gradient
    let mut pb_inner = PathBuilder::new();
    pb_inner.push_circle(center_car_x, center_car_y, radius_outer);
    pb_inner.push_circle(center_car_x, center_car_y, radius_inner);
    if let Some(path) = pb_inner.finish() {
        let mut paint = Paint::default();
        if let Some(shader) = tiny_skia::LinearGradient::new(
            tiny_skia::Point::from_xy(0.0, 0.0),
            tiny_skia::Point::from_xy(0.0, height as f32),
            if is_dark {
                vec![
                    tiny_skia::GradientStop::new(0.0, Color::from_rgba8(0x0A, 0x0A, 0x0A, 160)),
                    tiny_skia::GradientStop::new(1.0, Color::from_rgba8(0x0A, 0x0A, 0x0A, 40)),
                ]
            } else {
                vec![
                    tiny_skia::GradientStop::new(0.0, Color::from_rgba8(0xFF, 0xFF, 0xFF, 140)),
                    tiny_skia::GradientStop::new(1.0, Color::from_rgba8(0xFF, 0xFF, 0xFF, 20)),
                ]
            },
            tiny_skia::SpreadMode::Pad,
            Transform::identity(),
        ) {
            paint.shader = shader;
        }
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::EvenOdd,
            Transform::identity(),
            None,
        );
    }

    // 3. Hardware Mask - Solid RED outside the dashboard circle (Dev visibility)
    let mut pb_mask = PathBuilder::new();
    if let Some(rect) = tiny_skia::Rect::from_xywh(0.0, 0.0, width as f32, height as f32) {
        pb_mask.push_rect(rect);
    }
    pb_mask.push_circle(center_car_x, center_car_y, radius_max);
    if let Some(path) = pb_mask.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(255, 0, 0, 255);
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::EvenOdd,
            Transform::identity(),
            None,
        );
    }

    // --- Draw Car Marker (as done in car-app) ---
    // NFS_COLOR_CAR_GLOW: 0xFF8C00, Opa: 60/255
    let mut paint_glow = Paint::default();
    paint_glow.set_color_rgba8(0xFF, 0x8C, 0x00, 60);
    paint_glow.anti_alias = true;

    let mut pb_glow = PathBuilder::new();
    pb_glow.push_circle(center_car_x, center_car_y, 20.0 * scale);
    if let Some(path) = pb_glow.finish() {
        pixmap.fill_path(
            &path,
            &paint_glow,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    // NFS_COLOR_CAR_INNER: 0xFFB366, Opa: 255/255
    let mut paint_inner = Paint::default();
    paint_inner.set_color_rgba8(0xFF, 0xB3, 0x66, 255);
    paint_inner.anti_alias = true;

    let mut pb_inner = PathBuilder::new();
    pb_inner.push_circle(center_car_x, center_car_y, 10.0 * scale);
    if let Some(path) = pb_inner.finish() {
        pixmap.fill_path(
            &path,
            &paint_inner,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    // NFS_COLOR_ARROW_FILL: 0x313126 (Tech-Ink)
    let mut paint_arrow = Paint::default();
    paint_arrow.set_color_rgba8(0x31, 0x31, 0x26, 255);
    paint_arrow.anti_alias = true;

    let s_arrow = (12.0 / 36.0) * scale;
    let mut pb_arrow = PathBuilder::new();
    let pts = [
        (4.9f32, 33.0f32),
        (3.0f32, 31.4f32),
        (18.0f32, 3.0f32),
        (33.0f32, 31.4f32),
        (31.1f32, 33.0f32),
        (18.0f32, 28.3f32),
    ];

    pb_arrow.move_to(
        (pts[0].0 - 18.0) * s_arrow + center_car_x,
        (pts[0].1 - 18.0) * s_arrow + center_car_y,
    );
    for pt in pts.iter().skip(1) {
        pb_arrow.line_to(
            (pt.0 - 18.0) * s_arrow + center_car_x,
            (pt.1 - 18.0) * s_arrow + center_car_y,
        );
    }
    pb_arrow.close();

    if let Some(path) = pb_arrow.finish() {
        pixmap.fill_path(
            &path,
            &paint_arrow,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    buffer
}

fn add_polygon_to_pb(pb: &mut PathBuilder, coords: &[geo_types::Coord<f32>]) {
    if coords.is_empty() {
        return;
    }
    pb.move_to(coords[0].x, coords[0].y);
    for point in coords.iter().skip(1) {
        pb.line_to(point.x, point.y);
    }
    pb.close();
}

fn add_linestring_to_pb(pb: &mut PathBuilder, coords: &[geo_types::Coord<f32>]) {
    if coords.is_empty() {
        return;
    }
    pb.move_to(coords[0].x, coords[0].y);
    for point in coords.iter().skip(1) {
        pb.line_to(point.x, point.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_types::Coord;

    #[test]
    fn test_add_polygon_to_pb() {
        let mut pb = PathBuilder::new();
        let coords = vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 10.0, y: 0.0 },
            Coord { x: 10.0, y: 10.0 },
            Coord { x: 0.0, y: 10.0 },
        ];
        add_polygon_to_pb(&mut pb, &coords);
        let path = pb.finish();
        assert!(path.is_some());
    }

    #[test]
    fn test_add_linestring_to_pb() {
        let mut pb = PathBuilder::new();
        let coords = vec![Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 10.0 }];
        add_linestring_to_pb(&mut pb, &coords);
        let path = pb.finish();
        assert!(path.is_some());
    }

    #[test]
    fn test_render_map_empty_dimensions() {
        let view = MapView {
            center_x: 0,
            center_y: 0,
            zoom: 14,
        };
        let buffer = render_map(0.0, 0.0, 0, 0, &view, true);
        assert_eq!(buffer.width(), 1);
        assert_eq!(buffer.height(), 1);
    }

    #[test]
    fn test_render_map_basic() {
        let view = MapView {
            center_x: 33756,
            center_y: 21962,
            zoom: 16,
        };
        let buffer = render_map(0.0, 0.0, 100, 100, &view, true);
        assert_eq!(buffer.width(), 100);
    }

    #[test]
    fn test_render_map_dark_vs_light() {
        let view = MapView {
            center_x: 33756,
            center_y: 21962,
            zoom: 16,
        };
        let mut buffer_dark = render_map(0.0, 0.0, 10, 10, &view, true);
        let mut buffer_light = render_map(0.0, 0.0, 10, 10, &view, false);
        // Buffers should be different (at least the background color)
        assert_ne!(buffer_dark.make_mut_slice(), buffer_light.make_mut_slice());
    }

    #[test]
    fn test_render_map_offsets() {
        let view = MapView {
            center_x: 33756,
            center_y: 21962,
            zoom: 16,
        };
        let mut buffer1 = render_map(0.0, 0.0, 50, 50, &view, true);
        let mut buffer2 = render_map(10.0, 10.0, 50, 50, &view, true);
        assert_ne!(buffer1.make_mut_slice(), buffer2.make_mut_slice());
    }

    #[test]
    fn test_add_polygon_to_pb_empty() {
        let mut pb = PathBuilder::new();
        add_polygon_to_pb(&mut pb, &[]);
        let path = pb.finish();
        assert!(path.is_none());
    }

    #[test]
    fn test_add_linestring_to_pb_empty() {
        let mut pb = PathBuilder::new();
        add_linestring_to_pb(&mut pb, &[]);
        let path = pb.finish();
        assert!(path.is_none());
    }

    #[test]
    fn test_render_map_zoom_levels() {
        let view = MapView {
            center_x: 33756,
            center_y: 21962,
            zoom: 10,
        };
        let buffer = render_map(0.0, 0.0, 100, 100, &view, false);
        assert_eq!(buffer.width(), 100);

        let view_high = MapView {
            center_x: 33756,
            center_y: 21962,
            zoom: 18,
        };
        let buffer_high = render_map(0.0, 0.0, 100, 100, &view_high, false);
        assert_eq!(buffer_high.width(), 100);
    }
}
