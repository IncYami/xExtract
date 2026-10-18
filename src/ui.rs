use crate::app::{FileStatus, LogEntry, XExtractApp};
use crate::config_loader::ConfigLoader;
use crate::domain::config::TrackFilter;
use crate::icons;
use crate::mkv_tool::OutputStream;
use std::{path::Path, process::Command};

const PRIMARY: egui::Color32 = egui::Color32::from_rgb(111, 168, 255);
const SECONDARY: egui::Color32 = egui::Color32::from_rgb(170, 130, 255);
const SUCCESS: egui::Color32 = egui::Color32::from_rgb(105, 205, 145);
const ERROR: egui::Color32 = egui::Color32::from_rgb(235, 105, 105);
const EXTRACT_BUTTON: egui::Color32 = egui::Color32::from_rgb(54, 106, 190);
const STOP_BUTTON: egui::Color32 = egui::Color32::from_rgb(180, 56, 56);

pub fn draw(app: &mut XExtractApp, ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(10.0);
                draw_top_sections(app, ui);
                ui.add_space(10.0);
                draw_files(app, ui);
                ui.add_space(10.0);
                draw_progress(app, ui);
                ui.add_space(10.0);
                draw_logs(app, ui);
                ui.add_space(10.0);
            });
    });
}

fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            // Fill the frame's actual content area instead of estimating its
            // width; the frame's stroke also occupies horizontal space.
            ui.take_available_width();
            content(ui);
        });
}

fn draw_top_sections(app: &mut XExtractApp, ui: &mut egui::Ui) {
    if ui.available_width() >= 760.0 {
        ui.columns(2, |columns| {
            draw_controls(app, &mut columns[0]);
            draw_profile_details(app, &mut columns[1]);
        });
    } else {
        draw_controls(app, ui);
        ui.add_space(10.0);
        draw_profile_details(app, ui);
    }
}

fn draw_controls(app: &mut XExtractApp, ui: &mut egui::Ui) {
    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading(format!("{} xExtract", icons::FILM_STRIP));
            ui.separator();
            ui.label("MKV Track Extractor");
        });
        ui.add_space(6.0);
        let config_location = app
            .config_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Built-in defaults".to_owned());
        ui.horizontal(|ui| {
            let metadata_width = (ui.available_width() - 100.0).max(120.0);
            ui.vertical(|ui| {
                ui.set_max_width(metadata_width);
                ui.small(format!("MKVToolNix: {}", app.mkv_version));
                ui.label(
                    egui::RichText::new(format!("Config: {config_location}"))
                        .small()
                        .weak(),
                )
                .on_hover_text(&config_location);
            });
            if ui
                .add(egui::Button::new(format!("{}  Config", icons::GEAR)).small())
                .on_hover_text(
                    "Open the active config in the default text editor. Changes apply after restart.",
                )
                .clicked()
            {
                open_config_file(app);
            }
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let can_start =
                !app.running && !app.files.is_empty() && !app.config.profiles.is_empty();
            if ui
                .add_enabled(
                    can_start,
                    egui::Button::new(
                        egui::RichText::new(format!("{}  Extract", icons::PLAY))
                            .color(egui::Color32::WHITE),
                    )
                    .fill(EXTRACT_BUTTON)
                    .min_size(egui::vec2(108.0, 32.0)),
                )
                .clicked()
            {
                app.start_batch();
            }

            if ui
                .add_enabled(
                    app.running && !app.cancel_requested,
                    egui::Button::new(
                        egui::RichText::new(format!("{}  Stop", icons::STOP))
                            .color(egui::Color32::WHITE),
                    )
                    .fill(STOP_BUTTON)
                    .min_size(egui::vec2(108.0, 32.0)),
                )
                .clicked()
            {
                app.stop_batch();
            }
        });
    });
}

fn draw_profile_details(app: &mut XExtractApp, ui: &mut egui::Ui) {
    card(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.strong(format!("{}  Profile", icons::GEAR));
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.strong("Profile:");
                let profile_name = app
                    .config
                    .profiles
                    .get(app.selected_profile)
                    .map(|profile| profile.name.as_str())
                    .unwrap_or("No profile available");
                let profile_enabled = !app.running && !app.config.profiles.is_empty();
                ui.add_enabled_ui(profile_enabled, |ui| {
                    egui::ComboBox::from_id_salt("profile_selector")
                        .selected_text(profile_name)
                        .width(180.0)
                        .show_ui(ui, |ui| {
                            for (index, profile) in app.config.profiles.iter().enumerate() {
                                ui.selectable_value(
                                    &mut app.selected_profile,
                                    index,
                                    &profile.name,
                                );
                            }
                        });
                });
            });
        });
        ui.add_space(8.0);

        let Some(profile) = app.config.profiles.get(app.selected_profile) else {
            ui.vertical_centered(|ui| {
                ui.label("No profile is configured.");
            });
            return;
        };

        ui.horizontal_wrapped(|ui| {
            draw_profile_value(
                ui,
                "Mode",
                if profile.archive_mode {
                    "Archive"
                } else {
                    "Extract"
                },
                true,
            );
            draw_filter_value(ui, "Video", profile.video_filter());
            draw_filter_value(ui, "Audio", profile.audio_filter());
            draw_filter_value(ui, "Subtitles", profile.subtitle_filter());
            draw_profile_value(
                ui,
                "Chapters",
                if profile.chapters_enabled() {
                    "On"
                } else {
                    "Off"
                },
                profile.chapters_enabled(),
            );
            draw_profile_value(
                ui,
                "Attachments",
                if profile.attachments_enabled() {
                    "On"
                } else {
                    "Off"
                },
                profile.attachments_enabled(),
            );
        });

        let output = profile
            .output_dir
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or("Source folder");
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!("Output: {output}"))
                .small()
                .weak(),
        );
    });
}

fn draw_filter_value(ui: &mut egui::Ui, name: &str, filter: TrackFilter) {
    let value = if !filter.enabled {
        "Off".to_owned()
    } else if filter.languages.is_empty() {
        "All".to_owned()
    } else {
        filter.languages.join(", ")
    };
    draw_profile_value(ui, name, &value, filter.enabled);
}

fn draw_profile_value(ui: &mut egui::Ui, name: &str, value: &str, enabled: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(format!("{name}:")).weak());
        if enabled {
            ui.colored_label(SUCCESS, value);
        } else {
            ui.weak(value);
        }
    });
}

fn draw_files(app: &mut XExtractApp, ui: &mut egui::Ui) {
    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.strong(format!(
                "{}  MKV files ({})",
                icons::FOLDER_OPEN,
                app.files.len()
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !app.running,
                        egui::Button::new(format!("{}  Clear files", icons::TRASH_SIMPLE)),
                    )
                    .clicked()
                {
                    app.clear_files();
                }
                if ui
                    .add_enabled(
                        !app.running,
                        egui::Button::new(format!("{}  Add folder", icons::FOLDER_PLUS)),
                    )
                    .clicked()
                    && let Some(folder) = rfd::FileDialog::new().pick_folder()
                {
                    app.add_folder(&folder);
                }
                if ui
                    .add_enabled(
                        !app.running,
                        egui::Button::new(format!("{}  Add files", icons::FILE_PLUS)),
                    )
                    .clicked()
                    && let Some(files) = rfd::FileDialog::new()
                        .add_filter("Matroska", &["mkv"])
                        .pick_files()
                {
                    app.add_files(files);
                }
            });
        });
        ui.add_space(4.0);
        ui.small("Drag .mkv files into the window or use the buttons above.");
        ui.add_space(6.0);

        egui::ScrollArea::vertical()
            .max_height(220.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut clicked = None;
                let mut removed = None;
                for (index, item) in app.files.iter().enumerate() {
                    let selected = app.selected_file == Some(index);
                    let name = item
                        .path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("file.mkv");
                    let color = status_color(&item.status);

                    ui.horizontal(|ui| {
                        let (icon, filled) = status_icon(&item.status);
                        let icon_text = egui::RichText::new(icon).size(16.0).color(color);
                        let icon_text = if filled {
                            icon_text.family(egui::FontFamily::Name("phosphor_fill".into()))
                        } else {
                            icon_text
                        };
                        ui.label(icon_text);
                        let name_width = (ui.available_width() - 100.0).max(0.0);
                        let response = ui.add_sized(
                            [name_width, ui.spacing().interact_size.y],
                            egui::Button::selectable(selected, name).truncate(),
                        );
                        if response.clicked() {
                            clicked = Some(index);
                        }
                        ui.small(format_size(item.size));
                        if ui
                            .add_enabled(!app.running, egui::Button::new(icons::X))
                            .on_hover_text("Remove file")
                            .clicked()
                        {
                            removed = Some(index);
                        }
                    });
                    ui.separator();
                }
                if let Some(index) = clicked {
                    app.selected_file = Some(index);
                }
                if let Some(index) = removed {
                    app.remove_file(index);
                }
                if app.files.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label("No MKV files added.");
                    });
                }
            });
    });
}

fn draw_progress(app: &XExtractApp, ui: &mut egui::Ui) {
    let total = app.files.len();
    let completed = app
        .files
        .iter()
        .filter(|item| matches!(item.status, FileStatus::Completed))
        .count();
    let errors = app
        .files
        .iter()
        .filter(|item| matches!(item.status, FileStatus::Error))
        .count();
    let overall = if total == 0 {
        0.0
    } else {
        app.files
            .iter()
            .map(|item| f32::from(item.progress))
            .sum::<f32>()
            / total as f32
            / 100.0
    };
    let current = app
        .files
        .iter()
        .find(|item| matches!(item.status, FileStatus::Analyzing | FileStatus::Extracting));
    let current_progress = current.map_or(0.0, |item| f32::from(item.progress) / 100.0);
    card(ui, |ui| {
        ui.strong(format!("{}  Progress", icons::CLOCK));
        ui.add_space(8.0);
        progress_row(ui, "Current file", current_progress, PRIMARY);
        ui.add_space(8.0);
        progress_row(
            ui,
            &format!("Batch: {completed}/{total} completed · {errors} errors"),
            overall,
            SECONDARY,
        );
    });
}

fn open_config_file(app: &mut XExtractApp) {
    let path = match app.config_path.clone() {
        Some(path) => path,
        None => match ConfigLoader::create_user_config() {
            Ok(path) => path,
            Err(error) => {
                log_config_error(app, format!("Could not create the user config: {error}"));
                return;
            }
        },
    };
    app.config_path = Some(path.clone());

    if let Err(error) = open_with_default_editor(&path) {
        log_config_error(
            app,
            format!("Could not open config in a text editor: {error}"),
        );
    }
}

fn log_config_error(app: &mut XExtractApp, line: String) {
    app.logs.push(LogEntry {
        file_name: "Configuration".to_owned(),
        stream: OutputStream::App,
        line,
    });
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn open_with_default_editor(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    let launcher = "xdg-open";
    #[cfg(target_os = "macos")]
    let launcher = "open";
    #[cfg(target_os = "windows")]
    let launcher = "explorer.exe";

    Command::new(launcher).arg(path).spawn().map(|_| ())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn open_with_default_editor(_path: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "opening files in the default text editor is not supported on this platform",
    ))
}

fn draw_logs(app: &mut XExtractApp, ui: &mut egui::Ui) {
    card(ui, |ui| {
        egui::CollapsingHeader::new(format!(
            "{}  Logs ({})",
            icons::TERMINAL_WINDOW,
            app.logs.len()
        ))
        .id_salt("mkv_tool_logs")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !app.logs.is_empty(),
                        egui::Button::new(format!("{}  Copy", icons::COPY)),
                    )
                    .on_hover_text("Copy logs to the clipboard")
                    .clicked()
                {
                    ui.ctx().copy_text(format_logs(&app.logs));
                }
                if ui
                    .add_enabled(
                        !app.logs.is_empty(),
                        egui::Button::new(format!("{}  Clear logs", icons::TRASH_SIMPLE)),
                    )
                    .clicked()
                {
                    app.logs.clear();
                }
            });

            egui::ScrollArea::vertical()
                .id_salt("mkv_tool_log_output")
                .max_height(190.0)
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if app.logs.is_empty() {
                        ui.weak("MKVToolNix output will appear here.");
                        return;
                    }

                    let mut previous_file = None;
                    for entry in &app.logs {
                        if previous_file != Some(entry.file_name.as_str()) {
                            ui.add_space(4.0);
                            ui.strong(&entry.file_name);
                            previous_file = Some(entry.file_name.as_str());
                        }

                        let stream = match entry.stream {
                            OutputStream::Stdout => "stdout",
                            OutputStream::Stderr => "stderr",
                            OutputStream::App => "app",
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.weak(format!("[{stream}]"));
                            let line = egui::RichText::new(&entry.line).monospace();
                            if entry.stream == OutputStream::App {
                                ui.colored_label(ERROR, line);
                            } else {
                                ui.label(line);
                            }
                        });
                    }
                });
        });
    });
}

fn format_logs(logs: &[LogEntry]) -> String {
    let mut lines = Vec::with_capacity(logs.len() * 2);
    let mut previous_file = None;

    for entry in logs {
        if previous_file != Some(entry.file_name.as_str()) {
            if previous_file.is_some() {
                lines.push(String::new());
            }
            lines.push(entry.file_name.clone());
            previous_file = Some(entry.file_name.as_str());
        }

        let stream = match entry.stream {
            OutputStream::Stdout => "stdout",
            OutputStream::Stderr => "stderr",
            OutputStream::App => "app",
        };
        lines.push(format!("[{stream}] {}", entry.line));
    }

    lines.join("\n")
}

fn progress_row(ui: &mut egui::Ui, label: &str, value: f32, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.colored_label(color, format!("{}%", (value * 100.0).round() as u8));
        });
    });
    ui.add(
        egui::ProgressBar::new(value)
            .desired_width(ui.available_width())
            .fill(color),
    );
}

fn status_color(status: &FileStatus) -> egui::Color32 {
    match status {
        FileStatus::Completed => SUCCESS,
        FileStatus::Extracting => SECONDARY,
        FileStatus::Error => ERROR,
        FileStatus::Stopped => egui::Color32::from_rgb(220, 170, 90),
        _ => ui_text_color(),
    }
}

fn ui_text_color() -> egui::Color32 {
    egui::Color32::from_rgb(190, 195, 210)
}

fn status_icon(status: &FileStatus) -> (&'static str, bool) {
    match status {
        FileStatus::Pending => (icons::HOURGLASS, false),
        FileStatus::Analyzing => (icons::MAGNIFYING_GLASS, false),
        FileStatus::Ready => (icons::CHECK_CIRCLE, false),
        FileStatus::Extracting => (icons::ARROW_CLOCKWISE, false),
        FileStatus::Completed => (icons::CHECK_CIRCLE, true),
        FileStatus::Stopped => (icons::STOP, false),
        FileStatus::Error => (icons::X_CIRCLE, true),
    }
}

fn format_size(size: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}
