use anyhow::{bail, Context};
use game_planet_visuals::{
    known_planet_archetype_keys, PlanetRenderer, PlanetVisualProfile, ProfileSeedInput,
    RenderExecutionMode, RenderOptions,
};
use image::{Rgba, RgbaImage};
use serde_json::json;
use std::{
    collections::{HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    thread,
};

const DEFAULT_SEED: u64 = 0x5EED_1208_CAFE;
const DEFAULT_TIME_DAYS: f32 = 128.0;
const DEFAULT_CELL_SIZE: u32 = 320;
const DEFAULT_COLUMNS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatrixRenderer {
    Raster,
    Hybrid,
}

impl MatrixRenderer {
    fn parse(value: &str) -> anyhow::Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "raster" | "procedural" => Ok(Self::Raster),
            "hybrid" | "pathtrace" | "raytrace" => Ok(Self::Hybrid),
            _ => bail!("unknown renderer '{value}'; expected raster or hybrid"),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Raster => "raster",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatrixQuality {
    Preview,
    Standard,
    Ultra,
}

impl MatrixQuality {
    fn parse(value: &str) -> anyhow::Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "preview" => Ok(Self::Preview),
            "standard" => Ok(Self::Standard),
            "ultra" | "max" => Ok(Self::Ultra),
            _ => bail!("unknown quality '{value}'; expected preview, standard, or ultra"),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Standard => "standard",
            Self::Ultra => "ultra",
        }
    }

    const fn options(self) -> RenderOptions {
        match self {
            Self::Preview => RenderOptions::preview(),
            Self::Standard => RenderOptions::standard(),
            Self::Ultra => RenderOptions::ultra(),
        }
    }
}

#[derive(Debug, Clone)]
struct MatrixOptions {
    output_dir: PathBuf,
    icon_size: u32,
    columns: usize,
    seed: u64,
    time_days: f32,
    modifier_budget: usize,
    renderer: MatrixRenderer,
    quality: MatrixQuality,
    threads: usize,
}

impl Default for MatrixOptions {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("target/planet-matrix/max-ultra"),
            icon_size: DEFAULT_CELL_SIZE,
            columns: DEFAULT_COLUMNS,
            seed: DEFAULT_SEED,
            time_days: DEFAULT_TIME_DAYS,
            modifier_budget: 4,
            renderer: MatrixRenderer::Raster,
            quality: MatrixQuality::Ultra,
            threads: thread::available_parallelism()
                .map(|count| count.get())
                .unwrap_or(1)
                .clamp(1, 8),
        }
    }
}

#[derive(Debug, Clone)]
struct MatrixJob {
    index: usize,
    key: String,
    seed: u64,
}

#[derive(Debug)]
struct MatrixCell {
    index: usize,
    key: String,
    label: String,
    file_name: String,
    seed: u64,
    planet_class: String,
    size_key: String,
    radius_km: i32,
    renderer: &'static str,
    image: RgbaImage,
}

fn main() -> anyhow::Result<()> {
    let Some(options) = parse_args()? else {
        print_help();
        return Ok(());
    };

    fs::create_dir_all(&options.output_dir)
        .with_context(|| format!("creating {}", options.output_dir.display()))?;
    let icons_dir = options.output_dir.join("icons");
    fs::create_dir_all(&icons_dir).with_context(|| format!("creating {}", icons_dir.display()))?;

    let keys = deduped_archetype_keys();
    if keys.is_empty() {
        bail!("no known planet archetype keys are available");
    }

    let jobs = keys
        .into_iter()
        .enumerate()
        .map(|(index, key)| MatrixJob {
            index,
            seed: options
                .seed
                .wrapping_add((index as u64).wrapping_mul(0x9E37_79B9)),
            key,
        })
        .collect::<VecDeque<_>>();

    let total = jobs.len();
    let queue = Arc::new(Mutex::new(jobs));
    let options = Arc::new(options);
    let (tx, rx) = mpsc::channel::<Result<MatrixCell, String>>();

    for _ in 0..options.threads.min(total.max(1)) {
        let tx = tx.clone();
        let queue = Arc::clone(&queue);
        let options = Arc::clone(&options);
        let icons_dir = icons_dir.clone();
        thread::spawn(move || loop {
            let job = {
                let mut queue = queue.lock().expect("matrix render queue mutex poisoned");
                queue.pop_front()
            };
            let Some(job) = job else {
                break;
            };
            let result = render_cell(job, &options, &icons_dir).map_err(|err| format!("{err:#}"));
            if tx.send(result).is_err() {
                break;
            }
        });
    }
    drop(tx);

    let mut cells = Vec::with_capacity(total);
    for received in rx {
        match received {
            Ok(cell) => {
                eprintln!(
                    "[{}/{}] rendered {} ({})",
                    cells.len() + 1,
                    total,
                    cell.label,
                    cell.renderer
                );
                cells.push(cell);
            }
            Err(message) => bail!("{message}"),
        }
    }
    cells.sort_by_key(|cell| cell.index);

    let sheet = compose_sheet(&cells, options.icon_size, options.columns);
    let sheet_path = options.output_dir.join("planet-matrix.png");
    sheet
        .save(&sheet_path)
        .with_context(|| format!("writing {}", sheet_path.display()))?;

    let manifest_path = options.output_dir.join("planet-matrix.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest_json(&options, &cells, &sheet_path)?)?,
    )
    .with_context(|| format!("writing {}", manifest_path.display()))?;

    let html_path = options.output_dir.join("planet-matrix.html");
    fs::write(&html_path, matrix_html(&options, &cells, &sheet_path))
        .with_context(|| format!("writing {}", html_path.display()))?;

    println!(
        "rendered {total} planet types at {} quality into {}",
        options.quality.label(),
        options.output_dir.display()
    );
    println!("wrote {}", sheet_path.display());
    println!("wrote {}", html_path.display());
    println!("wrote {}", manifest_path.display());
    Ok(())
}

fn render_cell(
    job: MatrixJob,
    options: &MatrixOptions,
    icons_dir: &Path,
) -> anyhow::Result<MatrixCell> {
    let mut profile = PlanetVisualProfile::from_seed_input(
        ProfileSeedInput::new(job.seed)
            .with_archetype_key(job.key.clone())
            .with_modifier_budget(options.modifier_budget),
    );
    profile.set_snapshot_time_days(options.time_days);
    let renderer = PlanetRenderer::new(profile.clone());
    let render_options = options.quality.options();

    let (image, renderer_label) = match options.renderer {
        MatrixRenderer::Raster => (
            renderer.render_icon_with_options(options.icon_size, render_options),
            "raster-ultra-icon",
        ),
        MatrixRenderer::Hybrid => match renderer.try_render_raytraced_icon_with_progress(
            options.icon_size,
            render_options,
            RenderExecutionMode::Automatic,
            |_| {},
        ) {
            Ok(image) => (image, "cpu-pathtrace-icon"),
            Err(_) => (
                renderer.render_icon_with_options(options.icon_size, render_options),
                "raster-fallback-icon",
            ),
        },
    };

    let file_name = format!(
        "{:02}-{}.png",
        job.index + 1,
        slug_from_key(&profile.archetype_key)
    );
    let icon_path = icons_dir.join(&file_name);
    image
        .save(&icon_path)
        .with_context(|| format!("writing {}", icon_path.display()))?;

    Ok(MatrixCell {
        index: job.index,
        key: profile.archetype_key.clone(),
        label: label_from_key(&profile.archetype_key),
        file_name,
        seed: job.seed,
        planet_class: profile.planet_class.clone(),
        size_key: profile.size_key.clone(),
        radius_km: profile.radius_km,
        renderer: renderer_label,
        image,
    })
}

fn compose_sheet(cells: &[MatrixCell], icon_size: u32, columns: usize) -> RgbaImage {
    let columns = columns.max(1);
    let rows = cells.len().div_ceil(columns);
    let padding = (icon_size / 18).clamp(8, 28);
    let tile = icon_size + padding * 2;
    let width = tile * columns as u32;
    let height = tile * rows.max(1) as u32;
    let mut sheet = RgbaImage::from_pixel(width, height, Rgba([5, 7, 12, 255]));

    for cell in cells {
        let column = cell.index % columns;
        let row = cell.index / columns;
        let x0 = column as u32 * tile + padding;
        let y0 = row as u32 * tile + padding;
        blend_icon(&mut sheet, &cell.image, x0, y0);
    }

    sheet
}

fn blend_icon(sheet: &mut RgbaImage, icon: &RgbaImage, x0: u32, y0: u32) {
    for y in 0..icon.height() {
        for x in 0..icon.width() {
            let src = icon.get_pixel(x, y).0;
            let alpha = src[3] as f32 / 255.0;
            if alpha <= 0.0 {
                continue;
            }
            let dst = sheet.get_pixel_mut(x0 + x, y0 + y);
            for channel in 0..3 {
                dst[channel] = (src[channel] as f32 * alpha + dst[channel] as f32 * (1.0 - alpha))
                    .round() as u8;
            }
            dst[3] = 255;
        }
    }
}

fn manifest_json(
    options: &MatrixOptions,
    cells: &[MatrixCell],
    sheet_path: &Path,
) -> anyhow::Result<serde_json::Value> {
    Ok(json!({
        "schema": "universus.planet-matrix.v1",
        "renderer": "game-planet-visuals/render_planet_matrix",
        "quality": options.quality.label(),
        "rendererMode": options.renderer.label(),
        "iconSize": options.icon_size,
        "iconSupersample": options.quality.options().icon_supersample_for_size(game_planet_visuals::RenderSize {
            width: options.icon_size,
            height: options.icon_size,
        }),
        "columns": options.columns,
        "seed": options.seed,
        "timeDays": options.time_days,
        "modifierBudget": options.modifier_budget,
        "threads": options.threads,
        "sheet": sheet_path.file_name().and_then(|name| name.to_str()).unwrap_or("planet-matrix.png"),
        "count": cells.len(),
        "cells": cells
            .iter()
            .map(|cell| json!({
                "index": cell.index,
                "key": cell.key,
                "label": cell.label,
                "file": format!("icons/{}", cell.file_name),
                "seed": cell.seed,
                "planetClass": cell.planet_class,
                "size": cell.size_key,
                "radiusKm": cell.radius_km,
                "renderer": cell.renderer,
            }))
            .collect::<Vec<_>>()
    }))
}

fn matrix_html(options: &MatrixOptions, cells: &[MatrixCell], sheet_path: &Path) -> String {
    let mut html = String::new();
    html.push_str(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Planet Matrix</title>",
    );
    html.push_str("<style>body{margin:0;background:#05070c;color:#e8edf7;font:14px system-ui,Segoe UI,sans-serif}main{padding:24px}h1{font-size:22px;margin:0 0 6px}p{color:#9da8ba;margin:0 0 18px}.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(180px,1fr));gap:14px}.cell{background:#0c111c;border:1px solid #1f2a3b;border-radius:8px;padding:10px}.cell img{width:100%;height:auto;display:block}.label{font-weight:650;margin-top:8px}.meta{color:#9da8ba;font-size:12px;margin-top:3px;line-height:1.35}code{font-size:11px;color:#bac7da;word-break:break-all}.sheet{max-width:100%;height:auto;border:1px solid #1f2a3b;border-radius:8px;margin:10px 0 24px}</style></head><body><main>");
    html.push_str(&format!(
        "<h1>Planet Matrix</h1><p>{} types - {} quality - {} renderer - {}px icons - {}x icon AA - time {} days</p>",
        cells.len(),
        html_escape(options.quality.label()),
        html_escape(options.renderer.label()),
        options.icon_size,
        options
            .quality
            .options()
            .icon_supersample_for_size(game_planet_visuals::RenderSize {
                width: options.icon_size,
                height: options.icon_size,
            }),
        options.time_days
    ));
    let sheet_name = sheet_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("planet-matrix.png");
    html.push_str(&format!(
        "<img class=\"sheet\" src=\"{}\" alt=\"Planet matrix sheet\">",
        html_escape(sheet_name)
    ));
    html.push_str("<section class=\"grid\">");
    for cell in cells {
        html.push_str("<article class=\"cell\">");
        html.push_str(&format!(
            "<img src=\"icons/{}\" alt=\"{}\">",
            html_escape(&cell.file_name),
            html_escape(&cell.label)
        ));
        html.push_str(&format!(
            "<div class=\"label\">{}</div>",
            html_escape(&cell.label)
        ));
        html.push_str(&format!(
            "<div class=\"meta\">{} - {} - {} km - seed {:#x}</div>",
            html_escape(&cell.planet_class),
            html_escape(&cell.size_key),
            cell.radius_km,
            cell.seed
        ));
        html.push_str(&format!("<code>{}</code>", html_escape(&cell.key)));
        html.push_str("</article>");
    }
    html.push_str("</section></main></body></html>");
    html
}

fn deduped_archetype_keys() -> Vec<String> {
    let mut seen = HashSet::new();
    let mut keys = Vec::new();
    for key in known_planet_archetype_keys() {
        if seen.insert(key) {
            keys.push(key.to_string());
        }
    }
    keys
}

fn label_from_key(key: &str) -> String {
    let short = key.strip_prefix("catalog.archetype.").unwrap_or(key);
    short
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn slug_from_key(key: &str) -> String {
    let short = key.strip_prefix("catalog.archetype.").unwrap_or(key);
    let mut slug = String::with_capacity(short.len());
    let mut previous_dash = false;
    for ch in short.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn parse_args() -> anyhow::Result<Option<MatrixOptions>> {
    let mut options = MatrixOptions::default();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--help" | "-h" => return Ok(None),
            "--output-dir" => {
                index += 1;
                options.output_dir = PathBuf::from(take_value(&args, index, arg)?);
            }
            "--cell-size" | "--icon-size" => {
                index += 1;
                options.icon_size = parse_u32(take_value(&args, index, arg)?, arg, 64, 1024)?;
            }
            "--columns" => {
                index += 1;
                options.columns = parse_usize(take_value(&args, index, arg)?, arg, 1, 24)?;
            }
            "--seed" => {
                index += 1;
                options.seed = parse_u64(take_value(&args, index, arg)?, arg)?;
            }
            "--time-days" | "--time" => {
                index += 1;
                options.time_days = parse_f32(take_value(&args, index, arg)?, arg)?;
            }
            "--modifier-budget" => {
                index += 1;
                options.modifier_budget = parse_usize(take_value(&args, index, arg)?, arg, 0, 12)?;
            }
            "--quality" => {
                index += 1;
                options.quality = MatrixQuality::parse(take_value(&args, index, arg)?)?;
            }
            "--renderer" => {
                index += 1;
                options.renderer = MatrixRenderer::parse(take_value(&args, index, arg)?)?;
            }
            "--threads" => {
                index += 1;
                options.threads = parse_usize(take_value(&args, index, arg)?, arg, 1, 64)?;
            }
            _ if arg.starts_with("--output-dir=") => {
                options.output_dir = PathBuf::from(&arg["--output-dir=".len()..]);
            }
            _ if arg.starts_with("--cell-size=") => {
                options.icon_size =
                    parse_u32(&arg["--cell-size=".len()..], "--cell-size", 64, 1024)?;
            }
            _ if arg.starts_with("--icon-size=") => {
                options.icon_size =
                    parse_u32(&arg["--icon-size=".len()..], "--icon-size", 64, 1024)?;
            }
            _ if arg.starts_with("--columns=") => {
                options.columns = parse_usize(&arg["--columns=".len()..], "--columns", 1, 24)?;
            }
            _ if arg.starts_with("--seed=") => {
                options.seed = parse_u64(&arg["--seed=".len()..], "--seed")?;
            }
            _ if arg.starts_with("--time-days=") => {
                options.time_days = parse_f32(&arg["--time-days=".len()..], "--time-days")?;
            }
            _ if arg.starts_with("--quality=") => {
                options.quality = MatrixQuality::parse(&arg["--quality=".len()..])?;
            }
            _ if arg.starts_with("--renderer=") => {
                options.renderer = MatrixRenderer::parse(&arg["--renderer=".len()..])?;
            }
            _ if arg.starts_with("--threads=") => {
                options.threads = parse_usize(&arg["--threads=".len()..], "--threads", 1, 64)?;
            }
            _ if arg.starts_with("--modifier-budget=") => {
                options.modifier_budget = parse_usize(
                    &arg["--modifier-budget=".len()..],
                    "--modifier-budget",
                    0,
                    12,
                )?;
            }
            _ => bail!("unknown argument '{arg}'; run with --help for usage"),
        }
        index += 1;
    }

    Ok(Some(options))
}

fn take_value<'a>(args: &'a [String], index: usize, flag: &str) -> anyhow::Result<&'a str> {
    args.get(index)
        .map(String::as_str)
        .with_context(|| format!("{flag} requires a value"))
}

fn parse_u32(value: &str, flag: &str, min: u32, max: u32) -> anyhow::Result<u32> {
    let parsed = value
        .parse::<u32>()
        .with_context(|| format!("parsing {flag} '{value}'"))?;
    if !(min..=max).contains(&parsed) {
        bail!("{flag} must be between {min} and {max}");
    }
    Ok(parsed)
}

fn parse_usize(value: &str, flag: &str, min: usize, max: usize) -> anyhow::Result<usize> {
    let parsed = value
        .parse::<usize>()
        .with_context(|| format!("parsing {flag} '{value}'"))?;
    if !(min..=max).contains(&parsed) {
        bail!("{flag} must be between {min} and {max}");
    }
    Ok(parsed)
}

fn parse_f32(value: &str, flag: &str) -> anyhow::Result<f32> {
    let parsed = value
        .parse::<f32>()
        .with_context(|| format!("parsing {flag} '{value}'"))?;
    if !parsed.is_finite() {
        bail!("{flag} must be finite");
    }
    Ok(parsed)
}

fn parse_u64(value: &str, flag: &str) -> anyhow::Result<u64> {
    let cleaned = value.replace('_', "");
    if let Some(hex) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).with_context(|| format!("parsing {flag} '{value}'"))
    } else {
        cleaned
            .parse::<u64>()
            .with_context(|| format!("parsing {flag} '{value}'"))
    }
}

fn print_help() {
    println!(
        r#"Render a contact-sheet matrix for every known planet archetype.

Usage:
  cargo run -p game-planet-visuals --bin render_planet_matrix -- [options]

Defaults:
  --quality ultra --renderer raster --cell-size 320 --columns 8 --time-days 128

Options:
  --output-dir <path>             Output directory. Default: target/planet-matrix/max-ultra
  --cell-size, --icon-size <px>   Icon size per planet. Default: 320. Range: 64..1024.
  --columns <count>               Contact sheet columns. Default: 8.
  --quality <preview|standard|ultra|max>
  --renderer <raster|hybrid>      Hybrid uses CPU path tracing per icon and is much slower.
  --seed <u64|0xhex>              Base deterministic seed.
  --time-days <days>              Seeded time snapshot for clouds/currents/waves.
  --modifier-budget <count>       Profile modifier budget. Default: 4.
  --threads <count>               Concurrent archetype render workers.

Outputs:
  planet-matrix.png               Contact sheet image.
  planet-matrix.html              Labeled browser matrix.
  planet-matrix.json              Settings and cell manifest.
  icons/*.png                     Individual archetype icons."#
    );
}
