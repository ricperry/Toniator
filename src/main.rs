mod ui;

use gtk::prelude::*;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Default)]
pub struct CliOptions {
    demo: bool,
    demo_adjusted: bool,
    demo_curves: bool,
    screenshot: Option<std::path::PathBuf>,
    export_svg: Option<std::path::PathBuf>,
    export_png: Option<std::path::PathBuf>,
    save_document: Option<std::path::PathBuf>,
    save_treatment: Option<std::path::PathBuf>,
    preset: Option<std::path::PathBuf>,
    artwork: Option<std::path::PathBuf>,
    document: Option<std::path::PathBuf>,
    compare_source: bool,
    arrange_motif: bool,
    edit_shape: bool,
    curved_shape: bool,
    source_mapping: Option<u32>,
    independent_shapes: bool,
    artifact_zoom: Option<f64>,
    artifact_inspector_width: Option<i32>,
    artifact_window_size: Option<(i32, i32)>,
    artifact_resize_window: Option<(i32, i32)>,
    artifact_controls_hidden: bool,
    artifact_controls_shown: bool,
    allocation_report: Option<std::path::PathBuf>,
    indicator_state: Option<String>,
    indicator_report: Option<std::path::PathBuf>,
    preview_surface: Option<String>,
    export_background: Option<String>,
    expand_document: bool,
}

fn validated_source_mapping_index(value: Option<u32>) -> Result<u32, &'static str> {
    value
        .filter(|index| *index <= 4)
        .ok_or("--source-mapping must be an index from 0 through 4")
}

fn validated_appearance_artifact(
    value: Option<&str>,
    flag: &'static str,
    named: &str,
) -> Result<String, String> {
    let value = value.ok_or_else(|| format!("{flag} requires {named} or #RRGGBBAA"))?;
    if value == named {
        return Ok(value.to_owned());
    }
    value
        .strip_prefix('#')
        .filter(|hex| hex.len() == 8 && u32::from_str_radix(hex, 16).is_ok())
        .map(|_| value.to_owned())
        .ok_or_else(|| format!("{flag} must be {named} or #RRGGBBAA"))
}

impl CliOptions {
    pub fn artifact_mode(&self) -> bool {
        self.screenshot.is_some()
            || self.export_svg.is_some()
            || self.export_png.is_some()
            || self.save_document.is_some()
            || self.save_treatment.is_some()
            || self.preset.is_some()
            || self.artwork.is_some()
            || self.document.is_some()
            || self.compare_source
            || self.arrange_motif
            || self.edit_shape
            || self.curved_shape
            || self.source_mapping.is_some()
            || self.independent_shapes
            || self.artifact_zoom.is_some()
            || self.artifact_inspector_width.is_some()
            || self.artifact_window_size.is_some()
            || self.artifact_resize_window.is_some()
            || self.artifact_controls_hidden
            || self.artifact_controls_shown
            || self.allocation_report.is_some()
            || self.indicator_state.is_some()
            || self.indicator_report.is_some()
            || self.preview_surface.is_some()
            || self.export_background.is_some()
            || self.expand_document
    }

    pub fn loads_example(&self) -> bool {
        self.demo
            || self.demo_curves
            || self.export_svg.is_some()
            || self.export_png.is_some()
            || self.save_document.is_some()
            || self.save_treatment.is_some()
            || self.preset.is_some()
            || self.compare_source
            || self.arrange_motif
            || self.edit_shape
            || self.curved_shape
            || self.source_mapping.is_some()
            || self.independent_shapes
            || self.artifact_zoom.is_some()
            || self.artifact_inspector_width.is_some()
            || self.artifact_window_size.is_some()
            || self.artifact_resize_window.is_some()
            || self.artifact_controls_hidden
            || self.artifact_controls_shown
            || self.allocation_report.is_some()
            || self.indicator_state.is_some()
            || self.indicator_report.is_some()
            || self.preview_surface.is_some()
            || self.export_background.is_some()
            || self.expand_document
    }

    fn application_flags(&self) -> gtk::gio::ApplicationFlags {
        if self.artifact_mode() {
            gtk::gio::ApplicationFlags::NON_UNIQUE
        } else {
            gtk::gio::ApplicationFlags::empty()
        }
    }

    fn parse() -> Self {
        let mut options = Self::default();
        let mut arguments = std::env::args_os().skip(1);
        while let Some(argument) = arguments.next() {
            match argument.to_str() {
                Some("--demo") => options.demo = true,
                Some("--demo-adjusted") => {
                    options.demo = true;
                    options.demo_adjusted = true;
                }
                Some("--demo-curves") => options.demo_curves = true,
                Some("--screenshot") => {
                    options.screenshot = arguments.next().map(std::path::PathBuf::from)
                }
                Some("--export-svg") => {
                    options.export_svg = arguments.next().map(std::path::PathBuf::from)
                }
                Some("--export-png") => {
                    options.export_png = arguments.next().map(std::path::PathBuf::from)
                }
                Some("--save-document") => {
                    options.save_document = arguments.next().map(std::path::PathBuf::from)
                }
                Some("--save-treatment") => {
                    options.save_treatment = arguments.next().map(std::path::PathBuf::from)
                }
                Some("--preset") => options.preset = arguments.next().map(std::path::PathBuf::from),
                Some("--artwork") => {
                    options.artwork = arguments.next().map(std::path::PathBuf::from)
                }
                Some("--document") => {
                    options.document = arguments.next().map(std::path::PathBuf::from)
                }
                Some("--compare-source") => options.compare_source = true,
                Some("--arrange-motif") => options.arrange_motif = true,
                Some("--edit-shape") => options.edit_shape = true,
                Some("--curved-shape") => options.curved_shape = true,
                Some("--source-mapping") => {
                    let mapping = arguments
                        .next()
                        .and_then(|value| value.to_str().and_then(|value| value.parse().ok()));
                    options.source_mapping = match validated_source_mapping_index(mapping) {
                        Ok(mapping) => Some(mapping),
                        Err(error) => {
                            eprintln!("{error}");
                            std::process::exit(2);
                        }
                    };
                }
                Some("--independent-shapes") => options.independent_shapes = true,
                Some("--zoom") => {
                    options.artifact_zoom = arguments
                        .next()
                        .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
                }
                Some("--inspector-width") => {
                    options.artifact_inspector_width = arguments
                        .next()
                        .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
                }
                Some("--window-size") => {
                    options.artifact_window_size = arguments.next().and_then(|value| {
                        let value = value.to_str()?;
                        let (width, height) = value.split_once('x')?;
                        Some((width.parse().ok()?, height.parse().ok()?))
                    })
                }
                Some("--resize-window") => {
                    options.artifact_resize_window = arguments.next().and_then(|value| {
                        let value = value.to_str()?;
                        let (width, height) = value.split_once('x')?;
                        Some((width.parse().ok()?, height.parse().ok()?))
                    })
                }
                Some("--hide-controls") => options.artifact_controls_hidden = true,
                Some("--show-controls") => options.artifact_controls_shown = true,
                Some("--allocation-report") => {
                    options.allocation_report = arguments.next().map(std::path::PathBuf::from)
                }
                Some("--indicator-state") => {
                    options.indicator_state = arguments
                        .next()
                        .and_then(|value| value.to_str().map(str::to_owned))
                }
                Some("--indicator-report") => {
                    options.indicator_report = arguments.next().map(std::path::PathBuf::from)
                }
                Some("--preview-surface") => {
                    let value = arguments
                        .next()
                        .and_then(|value| value.to_str().map(str::to_owned));
                    options.preview_surface = match validated_appearance_artifact(
                        value.as_deref(),
                        "--preview-surface",
                        "checkerboard",
                    ) {
                        Ok(value) => Some(value),
                        Err(error) => {
                            eprintln!("{error}");
                            std::process::exit(2);
                        }
                    };
                }
                Some("--export-background") => {
                    let value = arguments
                        .next()
                        .and_then(|value| value.to_str().map(str::to_owned));
                    options.export_background = match validated_appearance_artifact(
                        value.as_deref(),
                        "--export-background",
                        "none",
                    ) {
                        Ok(value) => Some(value),
                        Err(error) => {
                            eprintln!("{error}");
                            std::process::exit(2);
                        }
                    };
                }
                Some("--expand-document") => options.expand_document = true,
                Some("--help") | Some("-h") => {
                    println!(
                        "Toniator native GTK application\n\n  --demo                 Open built-in artwork\n  --preview-surface V    Artifact preview: checkerboard or #RRGGBBAA\n  --export-background V  Artifact export: none or #RRGGBBAA\n  --expand-document      Expand Document controls for screenshots\n  --screenshot PATH      Save the actual application window as PNG\n  --export-svg PATH      Export the demo as editable SVG\n  --export-png PATH      Export the demo as a PNG image"
                    );
                    std::process::exit(0);
                }
                _ => {}
            }
        }
        options
    }

    pub fn indicator_phase(&self) -> Option<f64> {
        match self.indicator_state.as_deref() {
            Some("source") => Some(0.0),
            Some("active") => Some(0.5),
            Some("rendered") => Some(1.0),
            _ => None,
        }
    }
}

fn main() -> gtk::glib::ExitCode {
    let options = CliOptions::parse();
    let application = adw::Application::builder()
        .application_id("com.toniator.Toniator")
        .flags(options.application_flags())
        .build();
    let controller: Rc<RefCell<Option<Rc<ui::AppUi>>>> = Rc::new(RefCell::new(None));
    let activation_controller = Rc::clone(&controller);
    application.connect_activate(move |application| {
        if let Some(ui) = activation_controller.borrow().as_ref() {
            ui.present();
            return;
        }
        let ui = ui::AppUi::new(application, options.clone());
        ui.present();
        activation_controller.borrow_mut().replace(ui);
    });
    let exit_code = application.run_with_args(&["toniator"]);
    artifact_exit_code(
        exit_code,
        controller
            .borrow()
            .as_ref()
            .is_some_and(|ui| ui.cli_artifact_failed()),
    )
}

fn artifact_exit_code(
    application_exit_code: gtk::glib::ExitCode,
    artifact_failed: bool,
) -> gtk::glib::ExitCode {
    if application_exit_code == gtk::glib::ExitCode::SUCCESS && artifact_failed {
        gtk::glib::ExitCode::FAILURE
    } else {
        application_exit_code
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CliOptions, artifact_exit_code, validated_appearance_artifact,
        validated_source_mapping_index,
    };
    use std::path::PathBuf;

    #[test]
    fn demo_launches_are_normal_but_output_runs_are_isolated() {
        let mut options = CliOptions {
            demo: true,
            ..Default::default()
        };
        assert!(!options.artifact_mode());
        assert!(options.application_flags().is_empty());
        options.demo_adjusted = true;
        assert!(!options.artifact_mode());
        options.demo_curves = true;
        assert!(options.loads_example());
        assert!(!options.artifact_mode());
        options.screenshot = Some(PathBuf::from("capture.png"));
        assert!(options.artifact_mode());
        assert!(
            options
                .application_flags()
                .contains(gtk::gio::ApplicationFlags::NON_UNIQUE)
        );
    }

    #[test]
    fn save_and_export_load_demo_content_but_screenshot_only_stays_quiet() {
        let screenshot = CliOptions {
            screenshot: Some(PathBuf::from("start.png")),
            ..Default::default()
        };
        assert!(!screenshot.loads_example());

        let save = CliOptions {
            save_document: Some(PathBuf::from("document.toniator")),
            ..Default::default()
        };
        assert!(save.loads_example());

        let export = CliOptions {
            export_svg: Some(PathBuf::from("result.svg")),
            ..Default::default()
        };
        assert!(export.loads_example());

        let preset = CliOptions {
            preset: Some(PathBuf::from("ComicBook.tntr")),
            ..Default::default()
        };
        assert!(preset.loads_example());
        assert!(preset.artifact_mode());

        let compare = CliOptions {
            compare_source: true,
            ..Default::default()
        };
        assert!(compare.loads_example());
        assert!(compare.artifact_mode());
    }

    #[test]
    fn requested_artifact_failure_makes_an_otherwise_successful_run_fail() {
        assert_eq!(
            artifact_exit_code(gtk::glib::ExitCode::SUCCESS, true),
            gtk::glib::ExitCode::FAILURE
        );
        assert_eq!(
            artifact_exit_code(gtk::glib::ExitCode::SUCCESS, false),
            gtk::glib::ExitCode::SUCCESS
        );
        assert_eq!(
            artifact_exit_code(gtk::glib::ExitCode::FAILURE, false),
            gtk::glib::ExitCode::FAILURE
        );
    }

    #[test]
    fn source_mapping_cli_accepts_all_five_table_indices() {
        for index in 0..=4 {
            assert_eq!(validated_source_mapping_index(Some(index)), Ok(index));
        }
        assert!(validated_source_mapping_index(Some(5)).is_err());
        assert!(validated_source_mapping_index(None).is_err());
    }

    #[test]
    fn appearance_artifact_flags_are_strict_before_the_application_starts() {
        assert_eq!(
            validated_appearance_artifact(
                Some("checkerboard"),
                "--preview-surface",
                "checkerboard"
            )
            .unwrap(),
            "checkerboard"
        );
        assert_eq!(
            validated_appearance_artifact(Some("#12345680"), "--export-background", "none")
                .unwrap(),
            "#12345680"
        );
        for value in [None, Some("#123456"), Some("#nothex!!"), Some("white")] {
            assert!(
                validated_appearance_artifact(value, "--preview-surface", "checkerboard").is_err()
            );
        }
    }
}
