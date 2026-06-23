use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

mod countries;
mod map_processor;

use countries::CONTINENTS;
use map_processor::{ProgressMessage, run_conversion};

#[derive(Clone)]
enum Screen {
    ContinentSelect {
        continent_index: usize,
    },
    CountrySelect {
        continent_index: usize,
        cursor_index: usize,
        selections: Vec<bool>,
    },
    PoiConfig {
        continent_index: usize,
        selections: Vec<bool>,
        include_fuel: bool,
        include_charging: bool,
        cursor_index: usize,
    },
    Progress {
        log_messages: Vec<String>,
        download_status: Vec<(String, u64, Option<u64>)>, // name, downloaded, total
        phase: String,
        phase_percentage: u8,
        complete: bool,
        total_tiles: usize,
        error: Option<String>,
    },
}

struct App {
    screen: Screen,
    progress_rx: Option<Receiver<ProgressMessage>>,
}

pub static MAP_CHARS: &[&str] = &[
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣠⣶⣶⡶⢖⣤⣤⣴⣶⣶⣶⣦⡤⠀⠀⠀⠀⢀⣀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⡀⠦⠡⠨⠅⠍⠻⠉⠘⠛⠻⣿⣿⣿⣿⣿⡷⠀⠀⠀⠀⠀⠙⠁⠀⠀⠀⠀⠀⡠⠄⠀⠀⠀⣀⣤⣽⣤⡄⠀⠀⠀⠀⠄⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⢀⣴⣤⣤⣄⣠⣄⣏⡷⣾⡄⣝⣌⣹⢲⢦⡀⠀⢙⣿⣿⣿⠿⠃⠀⠀⠀⠀⠀⢀⣤⣤⣄⡀⡀⣘⣀⣄⣿⣾⣿⣿⣿⣿⣿⣿⣿⣿⣴⣶⣶⣦⣤⣤⣠⣤⡂⠀",
    "⠀⠀⣩⣿⣿⢿⣿⣿⣿⣿⣿⣿⣿⣿⠛⠒⢤⣙⠎⠀⠘⠿⠋⠁⠀⠐⠃⠀⠀⠀⣴⣿⢫⣿⣷⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠿⣿⠿⠟⠈⠁",
    "⠀⠀⠀⠚⠀⠀⠀⠙⢿⣿⣿⣿⣿⣿⣶⣤⣾⣿⣿⣤⠀⠀⠀⠀⠀⠀⠀⢠⣣⣀⣱⣯⣼⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣋⠀⠀⠾⠂⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠈⣿⣿⣿⣿⣿⣿⣿⣿⡿⠮⠈⠀⠀⠀⠀⠀⠀⠀⢀⣘⡿⠿⡻⣿⡿⠛⠿⣟⢿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠿⠃⠁⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠹⣿⣿⣿⣿⣿⣿⠟⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢘⣿⣤⣤⡂⢑⠙⢛⣿⣿⣬⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡝⠑⡠⠜⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠝⣿⡏⢀⠈⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢠⣾⣿⣿⣿⣿⣿⣿⣯⢻⣿⣽⡝⠛⠿⣿⣿⠿⣿⣿⡿⠿⠃⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠉⠛⢆⡀⣀⣁⡀⠀⠀⠀⠀⠀⠀⠸⣿⣿⣿⣿⣿⣿⣿⣿⣧⣛⠋⠀⠀⠀⢿⠁⠀⠈⠻⠆⠀⠨⠄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣼⣿⣿⣿⣶⣀⠀⠀⠀⠀⠀⠈⠉⠁⢹⣿⣿⣿⣿⡿⠋⠀⠀⠀⠀⠀⠀⠀⠐⢥⡠⣴⣀⠁⢀⣀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠹⣿⣿⣿⣿⣿⣿⠃⠀⠀⠀⠀⠀⠀⠀⣻⣿⣿⣿⡇⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠐⠂⣈⠘⡑⠁⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⣿⠿⠃⠀⠀⠀⠀⠀⠀⠀⠀⢻⣿⣿⡯⠀⠿⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢠⣶⣿⣿⣿⣷⣄⠀⠀⠀⠈⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⠏⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠟⠛⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠸⠟⠛⠙⢿⣿⠏⠀⠀⠀⡀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣽⡏⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠐⠀⠀⠀⠔⠁⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠟⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
];

pub static MAP_IDS: &[&[u8]] = &[
    &[
        7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 7, 7, 7,
        7, 0, 0, 0, 7, 7, 7, 7, 7, 7, 7, 7, 7, 3, 3, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    ],
    &[
        7, 7, 7, 7, 7, 7, 7, 7, 7, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 7, 7, 7, 7,
        7, 0, 0, 7, 7, 7, 7, 7, 0, 0, 7, 7, 7, 3, 3, 3, 3, 3, 7, 7, 7, 7, 3, 7, 7, 7, 7, 7, 7, 7,
    ],
    &[
        7, 7, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 7, 1, 1, 1, 1, 1, 1, 7, 7, 7, 7,
        7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 7,
    ],
    &[
        7, 7, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 7, 1, 1, 1, 1, 7, 0, 0, 7, 7, 7,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    ],
    &[
        7, 7, 7, 1, 7, 7, 7, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 7, 7, 7, 7, 7, 7, 7, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 7, 7, 3, 3, 7, 7, 7, 7,
    ],
    &[
        7, 7, 7, 7, 7, 7, 7, 7, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 7, 7, 7, 7, 7, 7, 7, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 7, 7, 7, 7, 7, 7, 7,
    ],
    &[
        7, 7, 7, 7, 7, 7, 7, 7, 7, 1, 1, 1, 1, 1, 1, 1, 1, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 0, 0, 4,
        4, 0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 7, 7, 7, 7, 7, 7, 7, 7,
    ],
    &[
        7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 1, 1, 1, 6, 1, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    ],
    &[
        7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 6, 6, 6, 6, 6, 2, 2, 2, 7, 7, 7, 7, 7, 7, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 3, 3, 7, 7, 7, 3, 3, 7, 3, 3, 3, 7, 3, 3, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    ],
    &[
        7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 2, 2, 2, 2, 2, 2, 7, 7, 7, 7, 7, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 7, 7, 7, 7, 7, 7, 7, 3, 3, 3, 3, 3, 3, 3, 3, 7, 7, 7, 7, 7, 7, 7, 7,
    ],
    &[
        7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 2, 2, 2, 2, 2, 2, 2, 2, 7, 7, 7, 7, 7, 7,
        7, 4, 4, 4, 4, 4, 4, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 3, 3, 5, 3, 5, 5, 7, 7, 7, 7, 7, 7,
    ],
    &[
        7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 2, 2, 2, 2, 2, 2, 7, 7, 7, 7, 7, 7, 7,
        7, 4, 4, 4, 4, 7, 4, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 5, 5, 5, 5, 5, 5, 5, 7, 7, 7, 5, 7, 7,
    ],
    &[
        7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 2, 2, 2, 2, 7, 7, 7, 7, 7, 7, 7, 7, 7,
        7, 4, 4, 4, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 5, 5, 5, 5, 5, 5, 5, 7, 7, 7, 5, 7, 7,
    ],
    &[
        7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 2, 2, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
        7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 5, 7, 7, 7, 5, 5, 7, 7,
    ],
    &[
        7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 2, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
        7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    ],
];

fn get_map_lines(selected_continent_idx: usize) -> Vec<Line<'static>> {
    let highlight_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let default_style = Style::default().fg(Color::Cyan);
    let ocean_style = Style::default().fg(Color::DarkGray);

    let is_sel = |idx: usize| idx == selected_continent_idx;

    let mut lines = Vec::new();
    for (y, row_chars) in MAP_CHARS.iter().enumerate() {
        let mut spans = Vec::new();
        let mut current_idx: Option<u8> = None;
        let mut current_text = String::new();

        let row_ids = MAP_IDS[y];
        let char_vec: Vec<char> = row_chars.chars().collect();

        for (x, &ch) in char_vec.iter().enumerate() {
            let id = row_ids[x];
            let idx = if id == 7 { None } else { Some(id) };

            if idx == current_idx {
                current_text.push(ch);
            } else {
                if !current_text.is_empty() {
                    let style = match current_idx {
                        Some(i) => {
                            if is_sel(i as usize) {
                                highlight_style
                            } else {
                                default_style
                            }
                        }
                        None => ocean_style,
                    };
                    spans.push(Span::styled(current_text, style));
                }
                current_idx = idx;
                current_text = ch.to_string();
            }
        }

        if !current_text.is_empty() {
            let style = match current_idx {
                Some(i) => {
                    if is_sel(i as usize) {
                        highlight_style
                    } else {
                        default_style
                    }
                }
                None => ocean_style,
            };
            spans.push(Span::styled(current_text, style));
        }

        lines.push(Line::from(spans));
    }
    lines
}

fn download_file(
    url: &str,
    dest_path: &Path,
    country_name: &str,
    progress_tx: &Sender<ProgressMessage>,
) -> Result<(), String> {
    if dest_path.exists() {
        let _ = progress_tx.send(ProgressMessage::DownloadComplete {
            country: country_name.to_string(),
        });
        return Ok(());
    }

    let _ = progress_tx.send(ProgressMessage::DownloadStart {
        country: country_name.to_string(),
    });

    let client = reqwest::blocking::Client::builder()
        .user_agent("car-app-map-tool/0.1")
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let mut response = client
        .get(url)
        .send()
        .map_err(|e| format!("Request failed for {}: {}", url, e))?;

    if !response.status().is_success() {
        return Err(format!("Server returned HTTP {}", response.status()));
    }

    let total_size = response.content_length();
    let mut file =
        File::create(dest_path).map_err(|e| format!("Failed to create download file: {}", e))?;
    let mut buffer = [0; 8192];
    let mut downloaded = 0;

    use std::fs::File;
    loop {
        let n = response
            .read(&mut buffer)
            .map_err(|e| format!("Read error during download: {}", e))?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])
            .map_err(|e| format!("Write error during download: {}", e))?;
        downloaded += n as u64;

        let _ = progress_tx.send(ProgressMessage::DownloadProgress {
            country: country_name.to_string(),
            downloaded,
            total: total_size,
        });
    }

    let _ = progress_tx.send(ProgressMessage::DownloadComplete {
        country: country_name.to_string(),
    });
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // If arguments are passed, run in headless mode
    if args.len() > 1 {
        if args[1] == "--headless" || args[1] == "-h" {
            if args.len() < 3 {
                println!("Usage: map-tool --headless <input_pbf_file1> [input_pbf_file2] ...");
                std::process::exit(1);
            }

            let mut include_fuel = false;
            let mut include_charging = false;
            let mut use_diepenbeek = false;
            let mut pbf_files = Vec::new();
            for arg in &args[2..] {
                if arg == "--poi-fuel" {
                    include_fuel = true;
                } else if arg == "--poi-charging" {
                    include_charging = true;
                } else if arg == "--diepenbeek" {
                    use_diepenbeek = true;
                } else {
                    pbf_files.push(PathBuf::from(arg));
                }
            }

            // If neither is specified, default to including both
            if !include_fuel && !include_charging {
                include_fuel = true;
                include_charging = true;
            }

            let bbox = if use_diepenbeek {
                Some((33703, 33805, 21919, 22005))
            } else {
                None
            };

            println!("Running map-tool in headless mode...");
            println!("Input PBFs: {:?}", pbf_files);
            println!(
                "Include Fuel: {}, Include Charging Stations: {}",
                include_fuel, include_charging
            );
            if use_diepenbeek {
                println!("Diepenbeek bounding box active: {:?}", bbox);
            }

            let output_file = Path::new("target/assets/map.mbtiles");
            println!("Output PMTiles: {:?}", output_file);

            let (tx, rx) = channel();

            // Spawn thread to log progress to stdout
            let progress_handle = thread::spawn(move || {
                while let Ok(msg) = rx.recv() {
                    match msg {
                        ProgressMessage::Pass1Progress {
                            file_name,
                            percentage,
                            ..
                        } => {
                            print!("\rPass 1: Parsing {} ({}%)", file_name, percentage);
                            let _ = std::io::stdout().flush();
                        }
                        ProgressMessage::Pass2Progress {
                            file_name,
                            percentage,
                            ..
                        } => {
                            print!("\rPass 2: Parsing {} ({}%)", file_name, percentage);
                            let _ = std::io::stdout().flush();
                        }
                        ProgressMessage::TileGenProgress { current, total } => {
                            let percent = ((current as f64 / total as f64) * 100.0) as u8;
                            print!("\rPass 3: Clipping Geometries ({}%)", percent);
                            let _ = std::io::stdout().flush();
                        }
                        ProgressMessage::Complete { total_tiles, .. } => {
                            println!("\nSUCCESS: Saved {} tiles.", total_tiles);
                        }
                        ProgressMessage::Error(err) => {
                            println!("\nERROR: {}", err);
                        }
                        _ => {}
                    }
                }
            });

            let result = run_conversion(
                &pbf_files,
                output_file,
                include_fuel,
                include_charging,
                bbox,
                tx,
            );
            let _ = progress_handle.join();

            match result {
                Ok(_) => {
                    println!("Conversion completed successfully.");
                    return Ok(());
                }
                Err(e) => {
                    println!("Conversion failed: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            println!("Unknown arguments. Usage:");
            println!("  map-tool              (Runs interactive TUI)");
            println!("  map-tool --headless <input_pbf_file1> [input_pbf_file2] ...");
            std::process::exit(1);
        }
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App {
        screen: Screen::ContinentSelect { continent_index: 0 },
        progress_rx: None,
    };

    let res = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error running map downloader & converter: {}", err);
    }
    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| draw_ui(f, &app))?;

        // Update progress channel messages if active
        if let Some(ref rx) = app.progress_rx {
            while let Ok(msg) = rx.try_recv() {
                if let Screen::Progress {
                    ref mut log_messages,
                    ref mut download_status,
                    ref mut phase,
                    ref mut phase_percentage,
                    ref mut complete,
                    ref mut total_tiles,
                    ref mut error,
                } = app.screen
                {
                    match msg {
                        ProgressMessage::DownloadStart { country } => {
                            log_messages.push(format!("Starting download of {}...", country));
                            download_status.push((country, 0, None));
                        }
                        ProgressMessage::DownloadProgress {
                            country,
                            downloaded,
                            total,
                        } => {
                            if let Some(idx) =
                                download_status.iter().position(|(c, _, _)| c == &country)
                            {
                                download_status[idx] = (country, downloaded, total);
                            }
                        }
                        ProgressMessage::DownloadComplete { country } => {
                            log_messages.push(format!("Finished download of {}!", country));
                            if let Some(idx) =
                                download_status.iter().position(|(c, _, _)| c == &country)
                            {
                                let total = download_status[idx].2;
                                download_status[idx].1 = total.unwrap_or(download_status[idx].1);
                            }
                        }
                        ProgressMessage::Pass1Start { total_files } => {
                            log_messages.push(format!(
                                "Pass 1/2: Extracting ways & POI nodes from {} PBF files...",
                                total_files
                            ));
                            *phase = "Pass 1: Extracting Roads & Water".to_string();
                        }
                        ProgressMessage::Pass1Progress {
                            file_index: _,
                            file_name,
                            percentage,
                        } => {
                            *phase = format!("Pass 1: Parsing {} ({}%)", file_name, percentage);
                            *phase_percentage = percentage;
                        }
                        ProgressMessage::Pass2Start { total_files } => {
                            log_messages.push(format!(
                                "Pass 2/2: Extracting node coordinates from {} PBF files...",
                                total_files
                            ));
                            *phase = "Pass 2: Extracting Node Coordinates".to_string();
                        }
                        ProgressMessage::Pass2Progress {
                            file_index: _,
                            file_name,
                            percentage,
                        } => {
                            *phase = format!("Pass 2: Parsing {} ({}%)", file_name, percentage);
                            *phase_percentage = percentage;
                        }
                        ProgressMessage::TileGenStart => {
                            log_messages
                                .push("Pass 3: Projecting and clipping geometries...".to_string());
                            *phase = "Pass 3: Clipping Geometries".to_string();
                            *phase_percentage = 0;
                        }
                        ProgressMessage::TileGenProgress { current, total } => {
                            let percent = ((current as f64 / total as f64) * 100.0) as u8;
                            *phase = format!("Pass 3: Clipping Geometries ({}%)", percent);
                            *phase_percentage = percent;
                        }
                        ProgressMessage::WritingStart => {
                            log_messages.push("Writing PMTiles archive...".to_string());
                            *phase = "Pass 4: Writing output.pmtiles".to_string();
                            *phase_percentage = 0;
                        }
                        ProgressMessage::Complete {
                            output_file,
                            total_tiles: tiles,
                        } => {
                            log_messages.push(format!(
                                "Successfully saved {} tiles to {:?}",
                                tiles, output_file
                            ));
                            *phase = "Conversion Complete!".to_string();
                            *phase_percentage = 100;
                            *complete = true;
                            *total_tiles = tiles;
                        }
                        ProgressMessage::Error(err_msg) => {
                            log_messages.push(format!("ERROR: {}", err_msg));
                            *error = Some(err_msg);
                        }
                    }
                }
            }
        }

        // Poll for inputs
        #[allow(clippy::collapsible_if)]
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.screen {
                        Screen::ContinentSelect {
                            ref mut continent_index,
                        } => match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Up if *continent_index > 0 => {
                                *continent_index -= 1;
                            }
                            KeyCode::Down if *continent_index < CONTINENTS.len() - 1 => {
                                *continent_index += 1;
                            }
                            KeyCode::Enter => {
                                let selections =
                                    vec![false; CONTINENTS[*continent_index].countries.len()];
                                app.screen = Screen::CountrySelect {
                                    continent_index: *continent_index,
                                    cursor_index: 0,
                                    selections,
                                };
                            }
                            _ => {}
                        },
                        Screen::CountrySelect {
                            continent_index,
                            ref mut cursor_index,
                            ref mut selections,
                        } => match key.code {
                            KeyCode::Esc => {
                                app.screen = Screen::ContinentSelect { continent_index };
                            }
                            KeyCode::Up if *cursor_index > 0 => {
                                *cursor_index -= 1;
                            }
                            KeyCode::Down if *cursor_index < selections.len() - 1 => {
                                *cursor_index += 1;
                            }
                            KeyCode::Char(' ') => {
                                selections[*cursor_index] = !selections[*cursor_index];
                            }
                            KeyCode::Char('a') => {
                                selections.fill(true);
                            }
                            KeyCode::Char('n') => {
                                selections.fill(false);
                            }
                            KeyCode::Enter => {
                                // Find selected countries
                                let mut has_selected = false;
                                for &sel in selections.iter() {
                                    if sel {
                                        has_selected = true;
                                        break;
                                    }
                                }

                                if has_selected {
                                    app.screen = Screen::PoiConfig {
                                        continent_index,
                                        selections: selections.clone(),
                                        include_fuel: true,
                                        include_charging: true,
                                        cursor_index: 0,
                                    };
                                }
                            }
                            _ => {}
                        },
                        Screen::PoiConfig {
                            continent_index,
                            ref selections,
                            ref mut include_fuel,
                            ref mut include_charging,
                            ref mut cursor_index,
                        } => match key.code {
                            KeyCode::Esc => {
                                app.screen = Screen::CountrySelect {
                                    continent_index,
                                    cursor_index: 0,
                                    selections: selections.clone(),
                                };
                            }
                            KeyCode::Up if *cursor_index > 0 => {
                                *cursor_index -= 1;
                            }
                            KeyCode::Down if *cursor_index < 2 => {
                                *cursor_index += 1;
                            }
                            KeyCode::Char(' ') => {
                                if *cursor_index == 0 {
                                    *include_fuel = !*include_fuel;
                                } else if *cursor_index == 1 {
                                    *include_charging = !*include_charging;
                                }
                            }
                            KeyCode::Enter => {
                                if *cursor_index == 0 {
                                    *include_fuel = !*include_fuel;
                                } else if *cursor_index == 1 {
                                    *include_charging = !*include_charging;
                                } else if *cursor_index == 2 {
                                    // Find selected countries
                                    let mut selected = Vec::new();
                                    for (idx, &sel) in selections.iter().enumerate() {
                                        if sel {
                                            selected
                                                .push(&CONTINENTS[continent_index].countries[idx]);
                                        }
                                    }

                                    if !selected.is_empty() {
                                        let (tx, rx) = channel();
                                        app.progress_rx = Some(rx);

                                        let mut paths = Vec::new();
                                        let countries_info: Vec<(String, String)> = selected
                                            .iter()
                                            .map(|c| {
                                                (c.name.to_string(), c.geofabrik_path.to_string())
                                            })
                                            .collect();

                                        let inc_fuel = *include_fuel;
                                        let inc_charging = *include_charging;

                                        app.screen = Screen::Progress {
                                            log_messages: vec!["Initializing...".to_string()],
                                            download_status: Vec::new(),
                                            phase: "Downloading Map Extracts".to_string(),
                                            phase_percentage: 0,
                                            complete: false,
                                            total_tiles: 0,
                                            error: None,
                                        };

                                        // Spawn processing thread
                                        thread::spawn(move || {
                                            let download_dir = Path::new("target/data/tmp");
                                            let _ = fs::create_dir_all(download_dir);

                                            for (name, path) in countries_info {
                                                let url = format!(
                                                    "https://download.geofabrik.de/{}-latest.osm.pbf",
                                                    path
                                                );
                                                let file_name = format!(
                                                    "{}-latest.osm.pbf",
                                                    path.replace('/', "_")
                                                );
                                                let dest_path = download_dir.join(file_name);

                                                if let Err(e) =
                                                    download_file(&url, &dest_path, &name, &tx)
                                                {
                                                    let _ = tx.send(ProgressMessage::Error(
                                                        format!("Download failed: {}", e),
                                                    ));
                                                    return;
                                                }
                                                paths.push(dest_path);
                                            }

                                            let output_file =
                                                Path::new("target/assets/map.mbtiles");
                                            let _ =
                                                fs::create_dir_all(output_file.parent().unwrap());

                                            if let Err(e) = run_conversion(
                                                &paths,
                                                output_file,
                                                inc_fuel,
                                                inc_charging,
                                                None,
                                                tx.clone(),
                                            ) {
                                                let _ = tx.send(ProgressMessage::Error(e));
                                            }
                                        });
                                    }
                                }
                            }
                            _ => {}
                        },
                        Screen::Progress {
                            ref complete,
                            ref error,
                            ..
                        } => {
                            if (*complete || error.is_some()) && (key.code == KeyCode::Enter || key.code == KeyCode::Esc) {
                                app.progress_rx = None;
                                app.screen = Screen::ContinentSelect { continent_index: 0 };
                            }
                        }
                    }
                }
            }
        }
    }
}

fn draw_ui(f: &mut Frame<'_>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Main Area
                Constraint::Length(3), // Footer
            ]
            .as_ref(),
        )
        .split(f.size());

    // 1. Header
    let header_text = vec![Line::from(vec![
        Span::styled(
            " OSM Map Downloader & Converter (car-app) ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" - Pure Rust Pipeline", Style::default().fg(Color::Gray)),
    ])];
    let header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(header, chunks[0]);

    // 2. Main Content
    match app.screen {
        Screen::ContinentSelect { continent_index } => {
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
                .split(chunks[1]);

            // Left: Continent List
            let mut list_items = Vec::new();
            for (idx, continent) in CONTINENTS.iter().enumerate() {
                let style = if idx == continent_index {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let indicator = if idx == continent_index { " > " } else { "   " };
                list_items.push(ListItem::new(Span::styled(
                    format!("{}{}", indicator, continent.name),
                    style,
                )));
            }

            let list = List::new(list_items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Select Continent "),
            );
            f.render_widget(list, main_chunks[0]);

            // Right: World Map
            let map_lines = get_map_lines(continent_index);
            let map_paragraph = Paragraph::new(map_lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" World Map Visualizer "),
                )
                .wrap(Wrap { trim: false });
            f.render_widget(map_paragraph, main_chunks[1]);
        }
        Screen::CountrySelect {
            continent_index,
            cursor_index,
            ref selections,
        } => {
            let continent = &CONTINENTS[continent_index];

            // Render country list
            let mut list_items = Vec::new();
            for (idx, country) in continent.countries.iter().enumerate() {
                let style = if idx == cursor_index {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let indicator = if idx == cursor_index { " > " } else { "   " };
                let checkbox = if selections[idx] { "[x] " } else { "[ ] " };

                list_items.push(ListItem::new(Span::styled(
                    format!("{}{}{}", indicator, checkbox, country.name),
                    style,
                )));
            }

            let list =
                List::new(list_items).block(Block::default().borders(Borders::ALL).title(format!(
                    " Select Countries in {} (Space to toggle, 'Enter' to proceed) ",
                    continent.name
                )));
            f.render_widget(list, chunks[1]);
        }
        Screen::PoiConfig {
            include_fuel,
            include_charging,
            cursor_index,
            ..
        } => {
            let mut list_items = Vec::new();

            // Fuel option
            let fuel_style = if cursor_index == 0 {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let fuel_indicator = if cursor_index == 0 { " > " } else { "   " };
            let fuel_checkbox = if include_fuel { "[x] " } else { "[ ] " };
            list_items.push(ListItem::new(Span::styled(
                format!(
                    "{}{}{}Include Gas Stations (fuel)",
                    fuel_indicator, fuel_checkbox, ""
                ),
                fuel_style,
            )));

            // Charging option
            let charging_style = if cursor_index == 1 {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let charging_indicator = if cursor_index == 1 { " > " } else { "   " };
            let charging_checkbox = if include_charging { "[x] " } else { "[ ] " };
            list_items.push(ListItem::new(Span::styled(
                format!(
                    "{}{}{}Include EV Charging Stations (charging_station)",
                    charging_indicator, charging_checkbox, ""
                ),
                charging_style,
            )));

            // Proceed Option
            let proceed_style = if cursor_index == 2 {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let proceed_indicator = if cursor_index == 2 { " > " } else { "   " };
            list_items.push(ListItem::new(Span::styled(
                format!("{}[ Proceed to Generation ]", proceed_indicator),
                proceed_style,
            )));

            let list = List::new(list_items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Configure Map Points of Interest (POIs) "),
            );
            f.render_widget(list, chunks[1]);
        }
        Screen::Progress {
            ref log_messages,
            ref download_status,
            ref phase,
            phase_percentage,
            complete,
            total_tiles,
            ref error,
        } => {
            let progress_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Length(3), // Phase & Progress Bar
                        Constraint::Min(4),    // Log messages
                    ]
                    .as_ref(),
                )
                .split(chunks[1]);

            // Phase / Progress Bar
            let mut text = vec![Line::from(vec![
                Span::styled("Phase: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(phase, Style::default().fg(Color::Green)),
            ])];

            if !complete && error.is_none() {
                let width = progress_chunks[0].width.saturating_sub(4) as usize;
                let filled = (width * phase_percentage as usize) / 100;
                let bar = format!(
                    "[{}{}] {}%",
                    "=".repeat(filled),
                    " ".repeat(width.saturating_sub(filled)),
                    phase_percentage
                );
                text.push(Line::from(vec![Span::styled(
                    bar,
                    Style::default().fg(Color::Yellow),
                )]));
            } else if let Some(err_msg) = error {
                text.push(Line::from(vec![
                    Span::styled(
                        "FAILED: ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(err_msg, Style::default().fg(Color::Red)),
                ]));
            } else {
                text.push(Line::from(vec![
                    Span::styled(
                        "SUCCESS! ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("Generated {} tiles.", total_tiles),
                        Style::default().fg(Color::Green),
                    ),
                ]));
            }

            let progress_para = Paragraph::new(text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Progress Status "),
            );
            f.render_widget(progress_para, progress_chunks[0]);

            // Logs / Details
            let mut log_items = Vec::new();

            // Show download progress if downloading
            if !download_status.is_empty() {
                log_items.push(ListItem::new(Line::from(vec![Span::styled(
                    "Downloads:",
                    Style::default().add_modifier(Modifier::UNDERLINED),
                )])));
                for (name, downloaded, total) in download_status {
                    let total_str = match total {
                        Some(t) => format!("{:.1} MB", *t as f64 / 1024.0 / 1024.0),
                        None => "Unknown size".to_string(),
                    };
                    let dl_mb = *downloaded as f64 / 1024.0 / 1024.0;

                    let dl_bar_str = if let Some(t) = total {
                        let bar_w = 20_usize;
                        let bar_filled = ((*downloaded as f64 / *t as f64) * bar_w as f64) as usize;
                        format!(
                            " [{}{}]",
                            "=".repeat(bar_filled),
                            " ".repeat(bar_w.saturating_sub(bar_filled))
                        )
                    } else {
                        "".to_string()
                    };

                    log_items.push(ListItem::new(Line::from(vec![
                        Span::styled(format!(" - {}: ", name), Style::default().fg(Color::Cyan)),
                        Span::styled(
                            format!("{:.1} MB / {}{}", dl_mb, total_str, dl_bar_str),
                            Style::default(),
                        ),
                    ])));
                }
                log_items.push(ListItem::new(Line::from(vec![Span::raw("")])));
            }

            log_items.push(ListItem::new(Line::from(vec![Span::styled(
                "Log Output:",
                Style::default().add_modifier(Modifier::UNDERLINED),
            )])));

            // Show last 20 log lines
            let start = log_messages.len().saturating_sub(20);
            for log in &log_messages[start..] {
                let style = if log.starts_with("ERROR:") {
                    Style::default().fg(Color::Red)
                } else if log.starts_with("SUCCESS:") {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Gray)
                };
                log_items.push(ListItem::new(Span::styled(log.clone(), style)));
            }

            let log_list = List::new(log_items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Execution Details "),
            );
            f.render_widget(log_list, progress_chunks[1]);
        }
    }

    // 3. Footer
    let footer_text = match &app.screen {
        Screen::ContinentSelect { .. } => vec![Line::from(vec![
            Span::styled("▲/▼", Style::default().fg(Color::Yellow)),
            Span::raw(" Move | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" Select Continent | "),
            Span::styled("Q/Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" Exit"),
        ])],
        Screen::CountrySelect { .. } => vec![Line::from(vec![
            Span::styled("▲/▼", Style::default().fg(Color::Yellow)),
            Span::raw(" Move | "),
            Span::styled("Space", Style::default().fg(Color::Yellow)),
            Span::raw(" Toggle | "),
            Span::styled("A", Style::default().fg(Color::Yellow)),
            Span::raw(" Select All | "),
            Span::styled("N", Style::default().fg(Color::Yellow)),
            Span::raw(" Clear | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" Next | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" Back"),
        ])],
        Screen::PoiConfig { .. } => vec![Line::from(vec![
            Span::styled("▲/▼", Style::default().fg(Color::Yellow)),
            Span::raw(" Move | "),
            Span::styled("Space", Style::default().fg(Color::Yellow)),
            Span::raw(" Toggle POI | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" Select / Start Generation | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" Back"),
        ])],
        Screen::Progress {
            complete, error, ..
        } => {
            if *complete || error.is_some() {
                vec![Line::from(vec![
                    Span::styled("Enter/Esc", Style::default().fg(Color::Yellow)),
                    Span::raw(" Return to Main Screen"),
                ])]
            } else {
                vec![Line::from(vec![
                    Span::styled(
                        "Please wait...",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    ),
                    Span::raw(" Map generation is running in the background."),
                ])]
            }
        }
    };

    let footer = Paragraph::new(footer_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(footer, chunks[2]);
}
