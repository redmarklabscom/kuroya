use super::{parse_string_list_input_value, render_string_list_input};
use eframe::egui;

#[test]
fn string_list_parse_preserves_raw_git_lines_for_apply() {
    let values = parse_string_list_input_value(" fetch \n\npull ");

    assert_eq!(values, [" fetch ", "", "pull "]);
}

#[test]
fn string_list_render_keeps_raw_git_values_when_unchanged() {
    let ctx = egui::Context::default();
    let mut values = vec![" fetch ".to_owned(), "".to_owned(), "pull ".to_owned()];
    let original = values.clone();

    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            render_string_list_input(ui, &mut values, "fetch\npull", 2);
        });
    });

    assert_eq!(values, original);
}
