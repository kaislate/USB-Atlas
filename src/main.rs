#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod demo;
mod descriptors;
mod details;
mod export;
mod insights;
mod model;
mod platform;
mod tree;
mod usbids;

use std::path::PathBuf;

const HELP: &str = "\
Usage: usbtree [options] [snapshot.json]

  (no options)          Start the GUI with the live USB topology
  snapshot.json         Start the GUI showing a saved snapshot
  --demo                Start the GUI with built-in demo data
  --report [file]       Write a text report (stdout if no file)
  --html <file>         Write an HTML report
  --json [file]         Write a JSON snapshot (stdout if no file)
  --no-hex              Omit hex dumps from reports
  --help                Show this help
";

/// Release builds use the GUI subsystem; reattach to the parent console so
/// command-line output is visible.
fn attach_console() {
    #[cfg(all(windows, not(debug_assertions)))]
    unsafe {
        use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn write_out(path: Option<&String>, content: &str) -> std::io::Result<()> {
    match path {
        Some(p) => std::fs::write(p, content),
        None => {
            use std::io::Write;
            std::io::stdout().write_all(content.as_bytes())
        }
    }
}

fn arg_value(args: &[String], flag: &str) -> Option<Option<String>> {
    let i = args.iter().position(|a| a == flag)?;
    Some(args.get(i + 1).filter(|v| !v.starts_with("--")).cloned())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let hex = !args.iter().any(|a| a == "--no-hex");

    let cli = ["--help", "-h", "/?", "--report", "--html", "--json"];
    if args.iter().any(|a| cli.contains(&a.as_str())) {
        attach_console();
        if args.iter().any(|a| a == "--help" || a == "-h" || a == "/?") {
            print!("{HELP}");
            return Ok(());
        }
        let snap = if args.iter().any(|a| a == "--demo") { demo::snapshot() } else { platform::scan() };
        if let Some(out) = arg_value(&args, "--report") {
            write_out(out.as_ref(), &details::full_report(&snap, hex))?;
        }
        if let Some(out) = arg_value(&args, "--json") {
            write_out(out.as_ref(), &serde_json::to_string_pretty(&snap)?)?;
        }
        if let Some(out) = arg_value(&args, "--html") {
            let Some(path) = out else {
                eprintln!("--html needs a file name");
                std::process::exit(2);
            };
            std::fs::write(path, export::html_report(&snap, hex))?;
        }
        return Ok(());
    }

    let open: Option<PathBuf> = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from);
    let demo = args.iter().any(|a| a == "--demo");

    let icon = Some(app::icon::app_icon());
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(app::APP_NAME)
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([760.0, 480.0])
        .with_drag_and_drop(true)
        .with_app_id("usbtree");
    if let Some(i) = icon {
        viewport = viewport.with_icon(std::sync::Arc::new(i));
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        app::APP_NAME,
        options,
        Box::new(move |cc| {
            let mut a = app::App::new(cc, open);
            if demo {
                a.load_demo();
            }
            Ok(Box::new(a))
        }),
    )?;
    Ok(())
}
