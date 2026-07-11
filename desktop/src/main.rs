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
    compare_source: bool,
    arrange_motif: bool,
}

impl CliOptions {
    pub fn artifact_mode(&self) -> bool {
        self.screenshot.is_some()
            || self.export_svg.is_some()
            || self.export_png.is_some()
            || self.save_document.is_some()
            || self.save_treatment.is_some()
            || self.preset.is_some()
            || self.compare_source
            || self.arrange_motif
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
                Some("--compare-source") => options.compare_source = true,
                Some("--arrange-motif") => options.arrange_motif = true,
                Some("--help") | Some("-h") => {
                    println!(
                        "Toniator native vertical slice\n\n  --demo                 Open built-in artwork\n  --demo-adjusted        Open an adjusted Lines example\n  --demo-curves          Open the useful default Curves treatment\n  --preset PATH          Apply a legacy treatment preset\n  --compare-source       Show source artwork for screenshot evidence\n  --arrange-motif        Show motif arrangement handles for evidence\n  --screenshot PATH      Save the actual application window as PNG\n  --export-svg PATH      Export the demo as editable SVG\n  --export-png PATH      Export the demo as a PNG image\n  --save-document PATH   Save the demo working document\n  --save-treatment PATH  Save the active treatment without artwork"
                    );
                    std::process::exit(0);
                }
                _ => {}
            }
        }
        options
    }
}

fn main() -> gtk::glib::ExitCode {
    let options = CliOptions::parse();
    let application = adw::Application::builder()
        .application_id("com.toniator.Toniator")
        .flags(options.application_flags())
        .build();
    let controller: Rc<RefCell<Option<Rc<ui::AppUi>>>> = Rc::new(RefCell::new(None));
    application.connect_activate(move |application| {
        if let Some(ui) = controller.borrow().as_ref() {
            ui.present();
            return;
        }
        let ui = ui::AppUi::new(application, options.clone());
        ui.present();
        controller.borrow_mut().replace(ui);
    });
    application.run_with_args(&["toniator"])
}

#[cfg(test)]
mod tests {
    use super::CliOptions;
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
}
