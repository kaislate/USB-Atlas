//! Renders the README screenshots offscreen (no display needed).
//!
//! Run with: `cargo test readme_screenshots -- --ignored`

use egui_kittest::Harness;

use super::{learn::Chapter, App, ViewMode};

type Setup = fn(&mut App);

#[test]
#[ignore = "writes docs/screenshots/*.png; run explicitly"]
fn readme_screenshots() {
    let shots: [(&str, Setup); 5] = [
        ("tree", |a| a.select_by_name("ultra fit")),
        ("map", |a| a.view = ViewMode::Map),
        ("learn-speeds", |a| {
            a.apply_launch_view(None, Some("speed comparison"))
        }),
        ("learn-companion", |a| {
            a.apply_launch_view(None, Some("companion"))
        }),
        ("learn-names", |a| {
            a.learn.open(Chapter::Names);
            a.view = ViewMode::Learn;
        }),
    ];
    std::fs::create_dir_all("docs/screenshots").unwrap();
    for (name, setup) in shots {
        let mut h = Harness::builder()
            .with_size([1360.0, 860.0])
            .wgpu()
            .build_eframe(|cc| {
                let mut a = App::new(cc, None);
                a.load_demo();
                a
            });
        setup(h.state_mut());
        // ~3 s of simulated time so entry animations settle.
        h.run_steps(180);
        let img = h.render().expect("offscreen render");
        img.save(format!("docs/screenshots/{name}.png")).unwrap();
    }
}
