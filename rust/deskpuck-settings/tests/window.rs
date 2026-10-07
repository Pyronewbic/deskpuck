use deskpuck_settings::viewport;
use eframe::egui::IconData;

#[test]
fn every_size_the_window_asks_for_is_finite_and_there_is_no_maximum() {
    let window = viewport(440.0, None);
    // An unbounded maximum made Wayland compositors refuse the window
    // (xdg_toplevel invalid_size), so it never opened.
    assert_eq!(window.max_inner_size, None);
    for size in [window.inner_size, window.min_inner_size] {
        let size = size.expect("set");
        assert!(size.x.is_finite() && size.y.is_finite(), "{size:?}");
    }
    let (min, initial) = (window.min_inner_size.unwrap(), window.inner_size.unwrap());
    assert!(min.x <= initial.x && min.y <= initial.y);
    assert_eq!(window.app_id.as_deref(), Some("deskpuck-settings"), "matches the .desktop file");
}

#[test]
fn the_icon_is_set_when_there_is_one() {
    assert!(viewport(440.0, None).icon.is_none());
    let icon = IconData { rgba: vec![0; 4], width: 1, height: 1 };
    assert!(viewport(440.0, Some(icon)).icon.is_some());
}
