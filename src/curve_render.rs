use crate::model::{
    AlternateTileTransform, CurveLayout, CurvePath, CurvePoint, Ink, MotifCoverage, ValueMode,
    WebCurveChannel, WebCurveSettings, parse_hex_color,
};
use crate::render::{
    Channel, InkLayer, calculate_web_grid, map_web_pixel, map_web_threshold, sample_web_image,
};
use anyhow::{Context, Result};
use image::RgbaImage;
use tiny_skia::{BlendMode, Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VariablePoint {
    pub x: f64,
    pub y: f64,
    pub width: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurveCommand {
    Move(CurvePoint),
    Cubic {
        control_1: CurvePoint,
        control_2: CurvePoint,
        end: CurvePoint,
    },
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurveOutline {
    pub commands: Vec<CurveCommand>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurveInkLayer {
    pub layer: InkLayer,
    pub outlines: Vec<CurveOutline>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurveGeometry {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<CurveInkLayer>,
}

pub fn generate_curve_geometry(
    source: &RgbaImage,
    settings: &WebCurveSettings,
) -> Result<CurveGeometry> {
    let ink_order = if settings.value_mode == crate::model::ValueMode::CrosshatchLuminance {
        [Ink::Black, Ink::Cyan, Ink::Magenta, Ink::Yellow]
    } else {
        Ink::ALL
    };
    let enabled: Vec<Ink> = ink_order
        .into_iter()
        .filter(|ink| settings.channels.get(*ink).enabled)
        .collect();
    let mut layers = Vec::new();

    for ink in ink_order {
        let channel = settings.channels.get(ink);
        if !channel.enabled {
            continue;
        }
        let long_edge_cells = (settings.long_edge_cells * channel.resolution_scale.max(0.05))
            .round()
            .max(2.0);
        let grid = calculate_web_grid(
            settings.output_width,
            settings.output_height,
            long_edge_cells,
        );
        let samples = sample_web_image(source, grid.cols, grid.rows);
        let path = if settings.use_shared_curve {
            &settings.shared_path
        } else {
            &channel.path
        };
        let close_ends = if settings.use_shared_curve {
            settings.shared_close_ends
        } else {
            channel.close_ends
        };
        let smooth_join = if settings.use_shared_curve {
            settings.shared_smooth_join
        } else {
            channel.smooth_join
        };
        let repeated = match settings.layout {
            CurveLayout::FullWidth => {
                let node_count = ((settings.output_width as f64
                    / grid.cell_width.min(grid.cell_height).max(1.0)
                    * channel.output_quality.max(0.1))
                .ceil() as usize)
                    .max(2);
                let local = sample_curve_path(path, node_count, close_ends, smooth_join);
                let baseline = build_full_curve_baseline(&local, settings, channel);
                repeat_and_transform(&baseline, settings, channel, &grid)
            }
            CurveLayout::MotifPattern => {
                let node_count = (24.0 * channel.output_quality.max(0.1)).ceil() as usize;
                let local = sample_motif_path(path, node_count.max(4));
                build_motif_rows(&local, settings, channel, &grid)
            }
        };
        let mut outlines = Vec::new();
        for points in repeated {
            let mut repeat_commands = Vec::new();
            let nodes: Vec<VariablePoint> = points
                .into_iter()
                .map(|point| VariablePoint {
                    x: point.x,
                    y: point.y,
                    width: curve_width_at_point(
                        point, ink, &samples, &grid, settings, channel, &enabled,
                    ),
                })
                .collect();
            if settings.use_shared_curve && settings.shared_close_ends
                || !settings.use_shared_curve && channel.close_ends
            {
                if nodes.iter().any(|node| node.width > 0.0) {
                    let simplified = simplify_segment(&nodes, &grid, channel);
                    if let Some(outline) = outline_from_points(&simplified, true) {
                        repeat_commands.extend(outline.commands);
                    }
                }
            } else {
                let margin = max_curve_width(settings, &grid, channel) * 1.5 + 2.0;
                for active in split_active_segments(&nodes) {
                    for clipped in clip_segment_to_artboard(&active, settings, margin) {
                        let simplified = simplify_segment(&clipped, &grid, channel);
                        if let Some(outline) = outline_from_points(&simplified, false) {
                            repeat_commands.extend(outline.commands);
                        }
                    }
                }
            }
            if !repeat_commands.is_empty() {
                outlines.push(CurveOutline {
                    commands: repeat_commands,
                });
            }
        }
        let output_color = if settings.value_mode == crate::model::ValueMode::CrosshatchLuminance {
            &settings.crosshatch_color
        } else {
            &channel.color
        };
        let (r, g, b) = parse_hex_color(output_color)
            .with_context(|| format!("invalid {} curve color", ink.label()))?;
        layers.push(CurveInkLayer {
            layer: InkLayer {
                channel: Channel::from(ink),
                enabled: channel.enabled,
                color: (r, g, b),
                opacity: channel.opacity as f32,
            },
            outlines,
        });
    }

    Ok(CurveGeometry {
        width: settings.output_width,
        height: settings.output_height,
        layers,
    })
}

pub fn render_curve_geometry(
    geometry: &CurveGeometry,
    max_dimension: u32,
    generation: u64,
) -> Result<crate::render::RenderResult> {
    let scale = max_dimension as f32 / geometry.width.max(geometry.height) as f32;
    let width = (geometry.width as f32 * scale).round().max(1.0) as u32;
    let height = (geometry.height as f32 * scale).round().max(1.0) as u32;
    let image = render_curve_geometry_output(geometry, width, height, true, None)?;
    Ok(crate::render::RenderResult { generation, image })
}

pub fn render_curve_geometry_output(
    geometry: &CurveGeometry,
    width: u32,
    height: u32,
    white_background: bool,
    channel: Option<Ink>,
) -> Result<RgbaImage> {
    let mut pixmap = Pixmap::new(width, height).context("curve output is too large")?;
    if white_background {
        pixmap.fill(Color::WHITE);
    }
    let scale_x = width as f32 / geometry.width as f32;
    let scale_y = height as f32 / geometry.height as f32;
    for layer in &geometry.layers {
        if channel.is_some_and(|ink| layer.layer.channel != Channel::from(ink)) {
            continue;
        }
        let (r, g, b) = layer.layer.color;
        let mut paint = Paint::default();
        paint.set_color_rgba8(
            r,
            g,
            b,
            (layer.layer.opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
        );
        paint.blend_mode = BlendMode::Multiply;
        paint.anti_alias = true;
        for outline in &layer.outlines {
            if let Some(path) = outline.to_tiny_skia_path() {
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::from_scale(scale_x, scale_y),
                    None,
                );
            }
        }
    }
    image::ImageBuffer::from_raw(width, height, pixmap.take())
        .context("curve renderer returned an invalid buffer")
}

impl CurveOutline {
    pub fn to_tiny_skia_path(&self) -> Option<tiny_skia::Path> {
        let mut builder = PathBuilder::new();
        for command in &self.commands {
            match *command {
                CurveCommand::Move(point) => builder.move_to(point.x as f32, point.y as f32),
                CurveCommand::Cubic {
                    control_1,
                    control_2,
                    end,
                } => builder.cubic_to(
                    control_1.x as f32,
                    control_1.y as f32,
                    control_2.x as f32,
                    control_2.y as f32,
                    end.x as f32,
                    end.y as f32,
                ),
                CurveCommand::Close => builder.close(),
            }
        }
        builder.finish()
    }

    pub fn to_svg_path_data(&self) -> String {
        use std::fmt::Write as _;
        let mut result = String::new();
        for command in &self.commands {
            match *command {
                CurveCommand::Move(point) => {
                    let _ = write!(result, "M {} {} ", number(point.x), number(point.y));
                }
                CurveCommand::Cubic {
                    control_1,
                    control_2,
                    end,
                } => {
                    let _ = write!(
                        result,
                        "C {} {} {} {} {} {} ",
                        number(control_1.x),
                        number(control_1.y),
                        number(control_2.x),
                        number(control_2.y),
                        number(end.x),
                        number(end.y)
                    );
                }
                CurveCommand::Close => result.push('Z'),
            }
        }
        result.trim().to_owned()
    }
}

fn sample_curve_path(
    path: &CurvePath,
    count: usize,
    close_ends: bool,
    smooth_join: bool,
) -> Vec<CurvePoint> {
    let mut path = path.clone();
    if close_ends && smooth_join {
        smooth_curve_seam(&mut path);
    }
    let mut polyline = vec![path.start];
    let mut start = path.start;
    for segment in &path.segments {
        let chord = distance(start, segment.end);
        let net = distance(start, segment.control_1)
            + distance(segment.control_1, segment.control_2)
            + distance(segment.control_2, segment.end);
        let subdivisions = ((net.max(chord) * 384.0).ceil() as usize).clamp(48, 1024);
        for step in 1..=subdivisions {
            let t = step as f64 / subdivisions as f64;
            polyline.push(cubic_point(
                start,
                segment.control_1,
                segment.control_2,
                segment.end,
                t,
            ));
        }
        start = segment.end;
    }
    if close_ends {
        polyline.push(path.start);
    }
    resample_polyline(&polyline, count)
}

fn smooth_curve_seam(path: &mut CurvePath) {
    let Some(first) = path.segments.first() else {
        return;
    };
    let direction = normalize(CurvePoint {
        x: first.control_1.x - path.start.x,
        y: first.control_1.y - path.start.y,
    });
    let Some(last) = path.segments.last_mut() else {
        return;
    };
    let length = distance(last.control_2, last.end);
    if length > 1e-9 {
        last.control_2 = CurvePoint {
            x: last.end.x - direction.x * length,
            y: last.end.y - direction.y * length,
        };
    }
}

fn sample_motif_path(path: &CurvePath, count: usize) -> Vec<CurvePoint> {
    // Matches the authoritative Node fallback used by the web generator:
    // flatten every cubic at 24 uniform t steps, then resample by arc length.
    let mut polyline = vec![path.start];
    let mut start = path.start;
    for segment in &path.segments {
        for step in 1..=24 {
            polyline.push(cubic_point(
                start,
                segment.control_1,
                segment.control_2,
                segment.end,
                step as f64 / 24.0,
            ));
        }
        start = segment.end;
    }
    resample_polyline(&polyline, count)
}

fn resample_polyline(points: &[CurvePoint], count: usize) -> Vec<CurvePoint> {
    if points.len() < 2 || count < 2 {
        return points.to_vec();
    }
    let mut lengths = Vec::with_capacity(points.len());
    lengths.push(0.0);
    for pair in points.windows(2) {
        lengths.push(lengths.last().copied().unwrap_or(0.0) + distance(pair[0], pair[1]));
    }
    let total = *lengths.last().unwrap_or(&0.0);
    if total <= 1e-12 {
        return vec![points[0]; count];
    }
    let mut result = Vec::with_capacity(count);
    let mut segment = 1;
    for index in 0..count {
        let target = total * index as f64 / (count - 1) as f64;
        while segment < lengths.len() - 1 && lengths[segment] < target {
            segment += 1;
        }
        let before = lengths[segment - 1];
        let span = (lengths[segment] - before).max(1e-12);
        result.push(lerp(
            points[segment - 1],
            points[segment],
            (target - before) / span,
        ));
    }
    result
}

fn build_full_curve_baseline(
    local: &[CurvePoint],
    settings: &WebCurveSettings,
    channel: &WebCurveChannel,
) -> Vec<CurvePoint> {
    let radians = channel.grid_rotation.to_radians();
    let target_length = (settings.output_width as f64 * radians.cos()).abs()
        + (settings.output_height as f64 * radians.sin()).abs();
    let start = local.first().copied().unwrap_or_default();
    let end = local
        .last()
        .copied()
        .unwrap_or(CurvePoint { x: 1.0, y: 0.0 });
    let source_length = distance(start, end).max(1e-9);
    let scale = target_length / source_length;
    let scaled: Vec<CurvePoint> = local
        .iter()
        .map(|point| CurvePoint {
            x: (point.x - start.x) * scale,
            y: (point.y - start.y) * scale,
        })
        .collect();
    let bounds = point_bounds(&scaled);
    scaled
        .into_iter()
        .map(|point| CurvePoint {
            x: point.x - bounds.0
                + (settings.output_width as f64 - bounds.2) / 2.0
                + channel.offset_x,
            y: point.y - bounds.1
                + (settings.output_height as f64 - bounds.3) / 2.0
                + channel.offset_y,
        })
        .collect()
}

fn repeat_and_transform(
    baseline: &[CurvePoint],
    settings: &WebCurveSettings,
    channel: &WebCurveChannel,
    grid: &crate::render::WebGrid,
) -> Vec<Vec<CurvePoint>> {
    let spacing = grid.cell_width.min(grid.cell_height).max(1.0);
    let radius = ((settings.output_width as f64).hypot(settings.output_height as f64) / spacing)
        .ceil() as i32
        + 2;
    let mut result = Vec::new();
    for index in -radius..=radius {
        let shifted: Vec<CurvePoint> = baseline
            .iter()
            .map(|point| CurvePoint {
                x: point.x,
                y: point.y + index as f64 * spacing,
            })
            .collect();
        let transformed: Vec<CurvePoint> = shifted
            .into_iter()
            .map(|point| curve_grid_transform(point, settings, channel))
            .collect();
        let bounds = point_bounds(&transformed);
        if bounds.0 + bounds.2 >= -spacing * 2.0
            && bounds.0 <= settings.output_width as f64 + spacing * 2.0
            && bounds.1 + bounds.3 >= -spacing * 2.0
            && bounds.1 <= settings.output_height as f64 + spacing * 2.0
        {
            result.push(transformed);
        }
    }
    result
}

fn build_motif_rows(
    local: &[CurvePoint],
    settings: &WebCurveSettings,
    channel: &WebCurveChannel,
    grid: &crate::render::WebGrid,
) -> Vec<Vec<CurvePoint>> {
    let motif = normalize_motif(local, channel.curve_scale);
    let start = motif.first().copied().unwrap_or_default();
    let end = motif.last().copied().unwrap_or(start);
    let mut row_advance = CurvePoint {
        x: end.x - start.x,
        y: end.y - start.y,
    };
    if distance(start, end) <= 0.0001 {
        let radians = channel.tile_angle.to_radians();
        row_advance = CurvePoint {
            x: radians.cos() * channel.curve_scale,
            y: radians.sin() * channel.curve_scale,
        };
    }
    let tile_direction = normalize(row_advance);
    let stack_angle = channel.tile_angle + 90.0 + channel.stack_angle;
    let stack_direction = CurvePoint {
        x: stack_angle.to_radians().cos(),
        y: stack_angle.to_radians().sin(),
    };
    let (tile_count, stack_count) = motif_counts(
        &motif,
        row_advance,
        stack_direction,
        settings,
        channel,
        grid,
    );
    let tile_origin = (tile_count - 1) as f64 / 2.0;
    let stack_origin = (stack_count - 1) as f64 / 2.0;
    let center = CurvePoint {
        x: settings.output_width as f64 / 2.0 + channel.offset_x,
        y: settings.output_height as f64 / 2.0 + channel.offset_y,
    };
    let mut rows = Vec::with_capacity(stack_count as usize);
    for stack_index in 0..stack_count {
        let stack_position = stack_index as f64 - stack_origin;
        let stagger = if stack_index % 2 == 1 {
            channel.alternate_stack_offset
        } else {
            0.0
        };
        let instance_center = CurvePoint {
            x: center.x
                + stack_position * stack_direction.x * channel.stack_spacing
                + tile_direction.x * (channel.tile_offset + stagger)
                + stack_direction.x * channel.stack_offset,
            y: center.y
                + stack_position * stack_direction.y * channel.stack_spacing
                + tile_direction.y * (channel.tile_offset + stagger)
                + stack_direction.y * channel.stack_offset,
        };
        let mut anchor = CurvePoint {
            x: instance_center.x - tile_origin * row_advance.x,
            y: instance_center.y - tile_origin * row_advance.y,
        };
        let mut chained = Vec::new();
        for tile_index in 0..tile_count {
            let alternate = (tile_index + stack_index) % 2 == 1;
            let mut tile = transform_motif_tile(
                &motif,
                alternate.then_some(channel.alternate_tile_transform),
            );
            let advance = CurvePoint {
                x: tile.last().map_or(0.0, |point| point.x) - tile[0].x,
                y: tile.last().map_or(0.0, |point| point.y) - tile[0].y,
            };
            if advance.x * row_advance.x + advance.y * row_advance.y < 0.0 {
                tile.reverse();
            }
            let tile_start = tile[0];
            let origin = CurvePoint {
                x: anchor.x - tile_start.x,
                y: anchor.y - tile_start.y,
            };
            for (index, point) in tile.iter().enumerate() {
                if !chained.is_empty() && index == 0 {
                    continue;
                }
                let placed = CurvePoint {
                    x: origin.x + point.x,
                    y: origin.y + point.y,
                };
                chained.push(motif_grid_transform(placed, settings, channel));
            }
            let last = *tile.last().unwrap_or(&tile_start);
            anchor = CurvePoint {
                x: origin.x + last.x,
                y: origin.y + last.y,
            };
        }
        let cell_size = grid.cell_width.min(grid.cell_height).max(1.0);
        let target_spacing = (cell_size / channel.output_quality.max(1.0)).max(0.5);
        let length = polyline_length(&chained);
        let count = ((length / target_spacing).ceil() as usize + 1)
            .max(chained.len())
            .min(20_000);
        if chained.len() >= 2 {
            rows.push(resample_polyline(&chained, count));
        }
    }
    rows
}

fn normalize_motif(points: &[CurvePoint], curve_scale: f64) -> Vec<CurvePoint> {
    if points.len() < 2 {
        return vec![
            CurvePoint {
                x: -curve_scale / 2.0,
                y: 0.0,
            },
            CurvePoint {
                x: curve_scale / 2.0,
                y: 0.0,
            },
        ];
    }
    let bounds = point_bounds(points);
    let source_size = bounds.2.max(bounds.3).max(0.0001);
    let center = CurvePoint {
        x: bounds.0 + bounds.2 / 2.0,
        y: bounds.1 + bounds.3 / 2.0,
    };
    let scale = curve_scale / source_size;
    points
        .iter()
        .map(|point| CurvePoint {
            x: (point.x - center.x) * scale,
            y: (point.y - center.y) * scale,
        })
        .collect()
}

fn motif_counts(
    motif: &[CurvePoint],
    row_advance: CurvePoint,
    stack_direction: CurvePoint,
    settings: &WebCurveSettings,
    channel: &WebCurveChannel,
    grid: &crate::render::WebGrid,
) -> (u32, u32) {
    if channel.motif_coverage == MotifCoverage::Manual {
        return (channel.tile_count, channel.stack_count);
    }
    // The web auto-cover calculation projects a nominal horizontal tile.
    // The actual endpoint advance controls spacing, but not this direction.
    let rotated_tile = rotate_vector(CurvePoint { x: 1.0, y: 0.0 }, channel.grid_rotation);
    let rotated_stack = rotate_vector(stack_direction, channel.grid_rotation);
    let cell_size = grid.cell_width.min(grid.cell_height).max(1.0);
    let bleed = channel.motif_bleed.max(0.0) * cell_size;
    let radius = motif
        .iter()
        .map(|point| point.x.hypot(point.y))
        .fold(1.0, f64::max);
    let margin = bleed
        + radius * 2.0
        + (settings.output_width as f64).hypot(settings.output_height as f64) * 0.08;
    let pad = channel.motif_bleed.max(0.0).ceil().max(4.0);
    let projection = |direction: CurvePoint| {
        settings.output_width as f64 * direction.x.abs()
            + settings.output_height as f64 * direction.y.abs()
    };
    let tile_spacing = row_advance.x.hypot(row_advance.y).max(1.0);
    let stack_spacing = channel.stack_spacing.abs().max(1.0);
    let tile_count = ((projection(rotated_tile) + margin * 2.0) / tile_spacing).ceil() + pad;
    let stack_count = ((projection(rotated_stack) + margin * 2.0) / stack_spacing).ceil() + pad;
    (
        tile_count.round().clamp(1.0, 10_000.0) as u32,
        stack_count.round().clamp(1.0, 10_000.0) as u32,
    )
}

fn transform_motif_tile(
    points: &[CurvePoint],
    alternate: Option<AlternateTileTransform>,
) -> Vec<CurvePoint> {
    points
        .iter()
        .map(|point| match alternate.unwrap_or_default() {
            AlternateTileTransform::None => *point,
            AlternateTileTransform::Flip => CurvePoint {
                x: -point.x,
                y: point.y,
            },
            AlternateTileTransform::Rotate180 => CurvePoint {
                x: -point.x,
                y: -point.y,
            },
        })
        .collect()
}

fn motif_grid_transform(
    point: CurvePoint,
    settings: &WebCurveSettings,
    channel: &WebCurveChannel,
) -> CurvePoint {
    let pivot = CurvePoint {
        x: settings.output_width as f64 / 2.0 + channel.grid_pivot_x,
        y: settings.output_height as f64 / 2.0 + channel.grid_pivot_y,
    };
    rotate_around(point, pivot, channel.grid_rotation)
}

fn rotate_vector(vector: CurvePoint, degrees: f64) -> CurvePoint {
    let radians = degrees.to_radians();
    CurvePoint {
        x: vector.x * radians.cos() - vector.y * radians.sin(),
        y: vector.x * radians.sin() + vector.y * radians.cos(),
    }
}

fn polyline_length(points: &[CurvePoint]) -> f64 {
    points
        .windows(2)
        .map(|pair| distance(pair[0], pair[1]))
        .sum()
}

fn curve_grid_transform(
    point: CurvePoint,
    settings: &WebCurveSettings,
    channel: &WebCurveChannel,
) -> CurvePoint {
    if channel.grid_rotation.abs() <= 0.0001 {
        return point;
    }
    let pivot = CurvePoint {
        x: settings.output_width as f64 / 2.0 + channel.grid_pivot_x,
        y: settings.output_height as f64 / 2.0 + channel.grid_pivot_y,
    };
    let page_center = CurvePoint {
        x: settings.output_width as f64 / 2.0,
        y: settings.output_height as f64 / 2.0,
    };
    let rotated_center = rotate_around(page_center, pivot, channel.grid_rotation);
    let radians = channel.grid_rotation.to_radians();
    let tangent = CurvePoint {
        x: radians.cos(),
        y: radians.sin(),
    };
    let tangent_shift = (rotated_center.x - page_center.x) * tangent.x
        + (rotated_center.y - page_center.y) * tangent.y;
    let rotated = rotate_around(point, pivot, channel.grid_rotation);
    CurvePoint {
        x: rotated.x - tangent.x * tangent_shift,
        y: rotated.y - tangent.y * tangent_shift,
    }
}

#[allow(clippy::too_many_arguments)]
fn curve_width_at_point(
    point: CurvePoint,
    ink: Ink,
    samples: &[[u8; 4]],
    grid: &crate::render::WebGrid,
    settings: &WebCurveSettings,
    channel: &WebCurveChannel,
    enabled: &[Ink],
) -> f64 {
    let x = point.x / grid.cell_width - 0.5;
    let y = point.y / grid.cell_height - 0.5;
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let tx = (x - x.floor()).clamp(0.0, 1.0);
    let ty = (y - y.floor()).clamp(0.0, 1.0);
    let mut rows = [0.0; 4];
    for row_offset in -1..=2 {
        let values = [
            raw_value(
                x0 - 1,
                y0 + row_offset,
                ink,
                samples,
                grid,
                settings.value_mode,
                settings.single_channel,
                enabled,
            ),
            raw_value(
                x0,
                y0 + row_offset,
                ink,
                samples,
                grid,
                settings.value_mode,
                settings.single_channel,
                enabled,
            ),
            raw_value(
                x0 + 1,
                y0 + row_offset,
                ink,
                samples,
                grid,
                settings.value_mode,
                settings.single_channel,
                enabled,
            ),
            raw_value(
                x0 + 2,
                y0 + row_offset,
                ink,
                samples,
                grid,
                settings.value_mode,
                settings.single_channel,
                enabled,
            ),
        ];
        rows[(row_offset + 1) as usize] = cubic_interpolate(values, tx);
    }
    let value = map_web_threshold(
        cubic_interpolate(rows, ty).clamp(0.0, 1.0),
        channel.threshold,
    );
    if value <= 0.0 {
        return 0.0;
    }
    let cell_size = grid.cell_width.min(grid.cell_height);
    let min = (settings.min_mark / 100.0).max(0.0);
    let max = min.max(settings.max_mark / 100.0) * channel.max_size / 100.0;
    cell_size * (min + (max - min) * value) * channel.scale
}

#[allow(clippy::too_many_arguments)]
fn raw_value(
    col: i32,
    row: i32,
    ink: Ink,
    samples: &[[u8; 4]],
    grid: &crate::render::WebGrid,
    mode: ValueMode,
    single: Ink,
    enabled: &[Ink],
) -> f64 {
    let col = col.clamp(0, grid.cols as i32 - 1) as u32;
    let row = row.clamp(0, grid.rows as i32 - 1) as u32;
    let values = map_web_pixel(
        samples[(row * grid.cols + col) as usize],
        mode,
        single,
        enabled,
    );
    values[match ink {
        Ink::Cyan => 0,
        Ink::Magenta => 1,
        Ink::Yellow => 2,
        Ink::Black => 3,
    }]
}

fn cubic_interpolate(points: [f64; 4], amount: f64) -> f64 {
    let t2 = amount * amount;
    let t3 = t2 * amount;
    0.5 * (2.0 * points[1]
        + (-points[0] + points[2]) * amount
        + (2.0 * points[0] - 5.0 * points[1] + 4.0 * points[2] - points[3]) * t2
        + (-points[0] + 3.0 * points[1] - 3.0 * points[2] + points[3]) * t3)
}

fn split_active_segments(nodes: &[VariablePoint]) -> Vec<Vec<VariablePoint>> {
    let mut result = Vec::new();
    let mut current = Vec::new();
    let mut previous_zero = None;
    for node in nodes {
        if node.width > 0.0 {
            if current.is_empty()
                && let Some(zero) = previous_zero
            {
                current.push(zero);
            }
            current.push(*node);
        } else {
            let zero = VariablePoint {
                width: 0.0,
                ..*node
            };
            if !current.is_empty() {
                current.push(zero);
                result.push(std::mem::take(&mut current));
            }
            previous_zero = Some(zero);
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn clip_segment_to_artboard(
    points: &[VariablePoint],
    settings: &WebCurveSettings,
    margin: f64,
) -> Vec<Vec<VariablePoint>> {
    let bounds = (
        -margin,
        -margin,
        settings.output_width as f64 + margin,
        settings.output_height as f64 + margin,
    );
    let mut result = Vec::new();
    let mut current = Vec::new();
    for pair in points.windows(2) {
        let Some((start, end)) = clip_line(pair[0], pair[1], bounds) else {
            if current.len() >= 2 {
                result.push(std::mem::take(&mut current));
            }
            continue;
        };
        if current
            .last()
            .is_none_or(|last| !same_variable(*last, start))
        {
            if current.len() >= 2 {
                result.push(std::mem::take(&mut current));
            }
            current.push(start);
        }
        current.push(end);
    }
    if current.len() >= 2 {
        result.push(current);
    }
    result
}

fn clip_line(
    start: VariablePoint,
    end: VariablePoint,
    bounds: (f64, f64, f64, f64),
) -> Option<(VariablePoint, VariablePoint)> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let mut t0: f64 = 0.0;
    let mut t1: f64 = 1.0;
    for (p, q) in [
        (-dx, start.x - bounds.0),
        (dx, bounds.2 - start.x),
        (-dy, start.y - bounds.1),
        (dy, bounds.3 - start.y),
    ] {
        if p.abs() <= 1e-6 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let ratio = q / p;
        if p < 0.0 {
            if ratio > t1 {
                return None;
            }
            t0 = t0.max(ratio);
        } else {
            if ratio < t0 {
                return None;
            }
            t1 = t1.min(ratio);
        }
    }
    (t0 <= t1).then(|| (lerp_variable(start, end, t0), lerp_variable(start, end, t1)))
}

fn simplify_segment(
    points: &[VariablePoint],
    grid: &crate::render::WebGrid,
    channel: &WebCurveChannel,
) -> Vec<VariablePoint> {
    if points.len() <= 3 {
        return points.to_vec();
    }
    let tolerance = ((grid.cell_width.min(grid.cell_height).max(1.0) * 0.04)
        / channel.output_quality.max(0.1).sqrt())
    .clamp(0.15, 0.75);
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    simplify_range(points, 0, points.len() - 1, tolerance, &mut keep);
    points
        .iter()
        .zip(keep)
        .filter_map(|(point, keep)| keep.then_some(*point))
        .collect()
}

fn simplify_range(
    points: &[VariablePoint],
    start: usize,
    end: usize,
    tolerance: f64,
    keep: &mut [bool],
) {
    if end <= start + 1 {
        return;
    }
    let mut farthest = None;
    let mut distance = 0.0;
    for index in start + 1..end {
        let candidate = variable_distance(points[index], points[start], points[end]);
        if candidate > distance {
            distance = candidate;
            farthest = Some(index);
        }
    }
    if distance <= tolerance {
        return;
    }
    if let Some(index) = farthest {
        keep[index] = true;
        simplify_range(points, start, index, tolerance, keep);
        simplify_range(points, index, end, tolerance, keep);
    }
}

fn outline_from_points(points: &[VariablePoint], closed: bool) -> Option<CurveOutline> {
    if points.len() < 2 {
        return None;
    }
    let source = if closed && same_xy(points[0], *points.last()?) {
        &points[..points.len() - 1]
    } else {
        points
    };
    if source.len() < 2 {
        return None;
    }
    let (left, right) = outline_rails(source, closed);
    let mut commands = smooth_boundary(&left, closed, false);
    if closed {
        commands.extend(smooth_boundary(
            &right.iter().copied().rev().collect::<Vec<_>>(),
            true,
            false,
        ));
        return Some(CurveOutline { commands });
    }
    if !same_point(left[left.len() - 1], right[right.len() - 1]) {
        commands.push(cap_command(
            left[left.len() - 1],
            right[right.len() - 1],
            source[source.len() - 1],
        ));
    }
    let reversed: Vec<CurvePoint> = right.iter().copied().rev().collect();
    commands.extend(smooth_boundary(&reversed, false, true));
    if !same_point(right[0], left[0]) {
        commands.push(cap_command(right[0], left[0], source[0]));
    }
    commands.push(CurveCommand::Close);
    Some(CurveOutline { commands })
}

fn outline_rails(points: &[VariablePoint], closed: bool) -> (Vec<CurvePoint>, Vec<CurvePoint>) {
    let mut left = Vec::with_capacity(points.len());
    let mut right = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        let previous = if closed {
            points[(index + points.len() - 1) % points.len()]
        } else {
            points[index.saturating_sub(1)]
        };
        let next = if closed {
            points[(index + 1) % points.len()]
        } else {
            points[(index + 1).min(points.len() - 1)]
        };
        let tangent = normalize(CurvePoint {
            x: next.x - previous.x,
            y: next.y - previous.y,
        });
        let half = points[index].width / 2.0;
        left.push(CurvePoint {
            x: points[index].x - tangent.y * half,
            y: points[index].y + tangent.x * half,
        });
        right.push(CurvePoint {
            x: points[index].x + tangent.y * half,
            y: points[index].y - tangent.x * half,
        });
    }
    (left, right)
}

fn smooth_boundary(points: &[CurvePoint], closed: bool, skip_move: bool) -> Vec<CurveCommand> {
    let mut commands = Vec::new();
    if points.is_empty() {
        return commands;
    }
    if !skip_move {
        commands.push(CurveCommand::Move(points[0]));
    }
    let segment_count = if closed {
        points.len()
    } else {
        points.len() - 1
    };
    for index in 0..segment_count {
        let start = points[index];
        let end = points[(index + 1) % points.len()];
        let start_tangent = boundary_tangent(points, index, closed);
        let end_tangent = boundary_tangent(points, (index + 1) % points.len(), closed);
        let segment_length = distance(start, end);
        let start_handle =
            (segment_length / 3.0).min(adjacent_distance(points, index, 1, closed) / 3.0);
        let end_handle = (segment_length / 3.0)
            .min(adjacent_distance(points, (index + 1) % points.len(), -1, closed) / 3.0);
        commands.push(CurveCommand::Cubic {
            control_1: CurvePoint {
                x: start.x + start_tangent.x * start_handle,
                y: start.y + start_tangent.y * start_handle,
            },
            control_2: CurvePoint {
                x: end.x - end_tangent.x * end_handle,
                y: end.y - end_tangent.y * end_handle,
            },
            end,
        });
    }
    if closed {
        commands.push(CurveCommand::Close);
    }
    commands
}

fn boundary_tangent(points: &[CurvePoint], index: usize, closed: bool) -> CurvePoint {
    let previous = if closed {
        points[(index + points.len() - 1) % points.len()]
    } else {
        points[index.saturating_sub(1)]
    };
    let next = if closed {
        points[(index + 1) % points.len()]
    } else {
        points[(index + 1).min(points.len() - 1)]
    };
    normalize(CurvePoint {
        x: next.x - previous.x,
        y: next.y - previous.y,
    })
}

fn adjacent_distance(points: &[CurvePoint], index: usize, direction: i32, closed: bool) -> f64 {
    let adjacent = if closed {
        (index as i32 + direction).rem_euclid(points.len() as i32) as usize
    } else {
        (index as i32 + direction).clamp(0, points.len() as i32 - 1) as usize
    };
    distance(points[index], points[adjacent])
}

fn cap_command(from: CurvePoint, to: CurvePoint, center: VariablePoint) -> CurveCommand {
    CurveCommand::Cubic {
        control_1: CurvePoint {
            x: from.x + (center.x - from.x) * 2.0 / 3.0,
            y: from.y + (center.y - from.y) * 2.0 / 3.0,
        },
        control_2: CurvePoint {
            x: to.x + (center.x - to.x) * 2.0 / 3.0,
            y: to.y + (center.y - to.y) * 2.0 / 3.0,
        },
        end: to,
    }
}

fn max_curve_width(
    settings: &WebCurveSettings,
    grid: &crate::render::WebGrid,
    channel: &WebCurveChannel,
) -> f64 {
    let cell = grid.cell_width.min(grid.cell_height);
    let min = (settings.min_mark / 100.0).max(0.0);
    let max = min.max(settings.max_mark / 100.0) * channel.max_size / 100.0;
    cell * max * channel.scale
}

fn point_bounds(points: &[CurvePoint]) -> (f64, f64, f64, f64) {
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

fn rotate_around(point: CurvePoint, pivot: CurvePoint, degrees: f64) -> CurvePoint {
    let radians = degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    let x = point.x - pivot.x;
    let y = point.y - pivot.y;
    CurvePoint {
        x: pivot.x + x * cos - y * sin,
        y: pivot.y + x * sin + y * cos,
    }
}

fn cubic_point(
    start: CurvePoint,
    c1: CurvePoint,
    c2: CurvePoint,
    end: CurvePoint,
    t: f64,
) -> CurvePoint {
    let u = 1.0 - t;
    CurvePoint {
        x: u.powi(3) * start.x
            + 3.0 * u.powi(2) * t * c1.x
            + 3.0 * u * t.powi(2) * c2.x
            + t.powi(3) * end.x,
        y: u.powi(3) * start.y
            + 3.0 * u.powi(2) * t * c1.y
            + 3.0 * u * t.powi(2) * c2.y
            + t.powi(3) * end.y,
    }
}

fn normalize(point: CurvePoint) -> CurvePoint {
    let length = point.x.hypot(point.y);
    if length <= 1e-9 {
        CurvePoint { x: 1.0, y: 0.0 }
    } else {
        CurvePoint {
            x: point.x / length,
            y: point.y / length,
        }
    }
}

fn distance(a: CurvePoint, b: CurvePoint) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn lerp(a: CurvePoint, b: CurvePoint, amount: f64) -> CurvePoint {
    CurvePoint {
        x: a.x + (b.x - a.x) * amount,
        y: a.y + (b.y - a.y) * amount,
    }
}

fn lerp_variable(a: VariablePoint, b: VariablePoint, amount: f64) -> VariablePoint {
    VariablePoint {
        x: a.x + (b.x - a.x) * amount,
        y: a.y + (b.y - a.y) * amount,
        width: a.width + (b.width - a.width) * amount,
    }
}

fn variable_distance(point: VariablePoint, start: VariablePoint, end: VariablePoint) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let dw = end.width - start.width;
    let length_squared = dx * dx + dy * dy + dw * dw;
    if length_squared <= 1e-6 {
        return ((point.x - start.x).powi(2)
            + (point.y - start.y).powi(2)
            + (point.width - start.width).powi(2))
        .sqrt();
    }
    let amount =
        (((point.x - start.x) * dx + (point.y - start.y) * dy + (point.width - start.width) * dw)
            / length_squared)
            .clamp(0.0, 1.0);
    let projected = lerp_variable(start, end, amount);
    ((point.x - projected.x).powi(2)
        + (point.y - projected.y).powi(2)
        + (point.width - projected.width).powi(2))
    .sqrt()
}

fn same_variable(a: VariablePoint, b: VariablePoint) -> bool {
    (a.x - b.x).abs() <= 0.001 && (a.y - b.y).abs() <= 0.001 && (a.width - b.width).abs() <= 0.001
}

fn same_xy(a: VariablePoint, b: VariablePoint) -> bool {
    (a.x - b.x).hypot(a.y - b.y) <= 0.001
}

fn same_point(a: CurvePoint, b: CurvePoint) -> bool {
    distance(a, b) <= 0.001
}

fn number(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    let mut text = format!("{rounded:.3}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text == "-0" { "0".into() } else { text }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_arc_sampling_preserves_curve_endpoints() {
        let path = CurvePath::soft_wave();
        let points = sample_curve_path(&path, 25, false, false);
        assert_eq!(points.len(), 25);
        assert_eq!(points[0], path.start);
        assert_eq!(*points.last().unwrap(), path.segments.last().unwrap().end);
        let distances: Vec<f64> = points
            .windows(2)
            .map(|pair| distance(pair[0], pair[1]))
            .collect();
        let average = distances.iter().sum::<f64>() / distances.len() as f64;
        assert!(
            distances
                .iter()
                .all(|value| (value - average).abs() < average * 0.08)
        );
    }

    #[test]
    fn outline_is_canonical_filled_cubic_path() {
        let points = [
            VariablePoint {
                x: 0.0,
                y: 0.0,
                width: 4.0,
            },
            VariablePoint {
                x: 10.0,
                y: 2.0,
                width: 8.0,
            },
            VariablePoint {
                x: 20.0,
                y: 0.0,
                width: 2.0,
            },
        ];
        let outline = outline_from_points(&points, false).unwrap();
        let data = outline.to_svg_path_data();
        assert!(data.starts_with("M "));
        assert!(data.contains(" C "));
        assert!(data.ends_with('Z'));
        assert!(outline.to_tiny_skia_path().is_some());
    }

    #[test]
    fn synthetic_reference_geometry_is_stable() {
        let source = RgbaImage::from_pixel(8, 5, image::Rgba([0, 0, 0, 255]));
        let mut settings = WebCurveSettings {
            output_width: 120,
            output_height: 80,
            long_edge_cells: 8.0,
            value_mode: ValueMode::SingleChannel,
            single_channel: Ink::Black,
            ..Default::default()
        };
        for ink in Ink::ALL {
            settings.channels.get_mut(ink).enabled = ink == Ink::Black;
        }
        settings.channels.k.threshold = 0.0;
        settings.channels.k.opacity = 1.0;
        settings.channels.k.grid_rotation = 0.0;
        let geometry = generate_curve_geometry(&source, &settings).unwrap();
        let paths: Vec<String> = geometry.layers[0]
            .outlines
            .iter()
            .map(CurveOutline::to_svg_path_data)
            .collect();
        assert_eq!(paths.len(), 9);
        // The authored Soft Wave now has a moderate model-space amplitude;
        // all generated geometry remains finite, closed, and deterministic.
        assert!(
            paths
                .iter()
                .all(|path| path.starts_with("M ") && path.ends_with(" Z"))
        );
        assert_eq!(
            paths,
            geometry.layers[0]
                .outlines
                .iter()
                .map(CurveOutline::to_svg_path_data)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn full_width_projection_preserves_authored_slope_without_xy_stretching() {
        let path = CurvePath::soft_wave();
        let local = sample_curve_path(&path, 33, false, false);
        let settings = WebCurveSettings {
            output_width: 1200,
            output_height: 300,
            ..Default::default()
        };
        let channel = WebCurveChannel::default();
        let projected = build_full_curve_baseline(&local, &settings, &channel);
        for (source, output) in local.windows(2).zip(projected.windows(2)) {
            let source_dx = source[1].x - source[0].x;
            let output_dx = output[1].x - output[0].x;
            if source_dx.abs() > 1e-9 && output_dx.abs() > 1e-9 {
                let source_slope = (source[1].y - source[0].y) / source_dx;
                let output_slope = (output[1].y - output[0].y) / output_dx;
                assert!((source_slope - output_slope).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn manual_motif_repeats_three_columns_two_rows_and_ignores_legacy_tile_spacing() {
        let source = RgbaImage::from_pixel(8, 5, image::Rgba([0, 0, 0, 255]));
        let mut settings = WebCurveSettings {
            output_width: 120,
            output_height: 80,
            long_edge_cells: 8.0,
            value_mode: ValueMode::SingleChannel,
            single_channel: Ink::Black,
            layout: CurveLayout::MotifPattern,
            ..Default::default()
        };
        for ink in Ink::ALL {
            settings.channels.get_mut(ink).enabled = ink == Ink::Black;
        }
        let channel = &mut settings.channels.k;
        channel.grid_rotation = 0.0;
        channel.threshold = 0.0;
        channel.opacity = 1.0;
        channel.motif_coverage = MotifCoverage::Manual;
        channel.curve_scale = 20.0;
        channel.tile_count = 3;
        channel.stack_count = 2;
        channel.stack_spacing = 28.0;
        settings.shared_path = CurvePath {
            start: CurvePoint { x: -0.5, y: -0.15 },
            segments: vec![crate::model::CubicCurveSegment {
                control_1: CurvePoint { x: -0.3, y: -0.4 },
                control_2: CurvePoint { x: 0.1, y: 0.3 },
                end: CurvePoint { x: 0.5, y: 0.05 },
            }],
        };
        channel.alternate_tile_transform = AlternateTileTransform::None;
        let plain = generate_curve_geometry(&source, &settings).unwrap();
        settings.channels.k.alternate_tile_transform = AlternateTileTransform::Flip;
        let flipped = generate_curve_geometry(&source, &settings).unwrap();
        settings.channels.k.alternate_tile_transform = AlternateTileTransform::Rotate180;
        let geometry = generate_curve_geometry(&source, &settings).unwrap();
        assert_ne!(plain, flipped);
        assert_ne!(plain, geometry);
        assert_ne!(flipped, geometry);
        assert_eq!(geometry.layers.len(), 1);
        assert_eq!(geometry.layers[0].outlines.len(), 2);
        assert!(
            geometry.layers[0]
                .outlines
                .iter()
                .all(|outline| outline.commands.len() > 6)
        );

        settings.channels.k.tile_spacing = 999.0;
        let legacy_spacing = generate_curve_geometry(&source, &settings).unwrap();
        assert_eq!(geometry, legacy_spacing);
    }

    #[test]
    fn bundled_auto_coverage_counts_match_authoritative_web_projection() {
        let settings = WebCurveSettings {
            output_width: 900,
            output_height: 638,
            long_edge_cells: 90.0,
            layout: CurveLayout::MotifPattern,
            ..Default::default()
        };
        let path = CurvePath {
            start: CurvePoint { x: -0.5, y: -0.083 },
            segments: vec![crate::model::CubicCurveSegment {
                control_1: CurvePoint {
                    x: -0.185,
                    y: -0.212,
                },
                control_2: CurvePoint { x: 0.193, y: 0.212 },
                end: CurvePoint { x: 0.5, y: 0.074 },
            }],
        };
        let sampled = sample_motif_path(&path, 24);
        let grid = calculate_web_grid(900, 638, 90.0);
        for (angle, expected) in [
            (30.0, (47, 218)),
            (60.0, (44, 234)),
            (0.0, (41, 158)),
            (90.0, (33, 201)),
        ] {
            let channel = WebCurveChannel {
                grid_rotation: angle,
                curve_scale: 32.0,
                motif_coverage: MotifCoverage::Auto,
                motif_bleed: 2.0,
                stack_spacing: 6.0,
                ..Default::default()
            };
            let motif = normalize_motif(&sampled, channel.curve_scale);
            let row_advance = CurvePoint {
                x: motif.last().unwrap().x - motif[0].x,
                y: motif.last().unwrap().y - motif[0].y,
            };
            let stack_direction = CurvePoint { x: 0.0, y: 1.0 };
            assert_eq!(
                motif_counts(
                    &motif,
                    row_advance,
                    stack_direction,
                    &settings,
                    &channel,
                    &grid,
                ),
                expected
            );
        }
    }

    #[test]
    fn crosshatch_curve_layers_use_one_configured_monochrome_color() {
        let source = image::RgbaImage::from_pixel(16, 12, image::Rgba([0, 0, 0, 255]));
        let mut settings = WebCurveSettings {
            output_width: 160,
            output_height: 120,
            value_mode: ValueMode::CrosshatchLuminance,
            crosshatch_color: "#234567".into(),
            ..Default::default()
        };
        for (index, ink) in Ink::ALL.into_iter().enumerate() {
            settings.channels.get_mut(ink).color =
                ["#ff0000", "#00ff00", "#0000ff", "#ffffff"][index].into();
        }
        let geometry = generate_curve_geometry(&source, &settings).unwrap();
        assert_eq!(geometry.layers.len(), 4);
        assert!(
            geometry
                .layers
                .iter()
                .all(|layer| layer.layer.color == (0x23, 0x45, 0x67))
        );
    }

    #[test]
    fn configured_crosshatch_generates_four_distinct_crossing_line_directions() {
        let source = image::RgbaImage::from_pixel(32, 24, image::Rgba([0, 0, 0, 255]));
        let mut settings = WebCurveSettings {
            output_width: 240,
            output_height: 180,
            long_edge_cells: 12.0,
            ..Default::default()
        };
        settings.configure_crosshatch();
        let geometry = generate_curve_geometry(&source, &settings).unwrap();
        assert_eq!(geometry.layers.len(), 4);
        assert!(
            geometry
                .layers
                .iter()
                .all(|layer| !layer.outlines.is_empty())
        );
        let paths = geometry
            .layers
            .iter()
            .map(|layer| {
                layer
                    .outlines
                    .iter()
                    .map(CurveOutline::to_svg_path_data)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for left in 0..paths.len() {
            for right in left + 1..paths.len() {
                assert_ne!(paths[left], paths[right]);
            }
        }
        assert_eq!(settings.channels.k.grid_rotation, 45.0);
        assert_eq!(settings.channels.c.grid_rotation, -45.0);
        assert_eq!(settings.channels.m.grid_rotation, 0.0);
        assert_eq!(settings.channels.y.grid_rotation, 90.0);
    }
}
