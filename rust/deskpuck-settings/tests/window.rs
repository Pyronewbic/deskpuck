use deskpuck_settings::layout::{MIN_HEIGHT, MIN_WIDTH, width};
use deskpuck_settings::model::Tab;
use deskpuck_settings::viewport;
use eframe::egui::IconData;

#[test]
fn every_size_the_window_asks_for_is_finite_and_there_is_no_maximum() {
    for tab in Tab::ALL {
        let window = viewport(width(tab), None);
        // An unbounded maximum made Wayland compositors refuse the window
        // (xdg_toplevel invalid_size), so it never opened.
        assert_eq!(window.max_inner_size, None);
        for size in [window.inner_size, window.min_inner_size] {
            let size = size.expect("set");
            assert!(size.x.is_finite() && size.y.is_finite(), "{size:?}");
        }
        let (min, initial) = (window.min_inner_size.unwrap(), window.inner_size.unwrap());
        assert!(min.x <= initial.x && min.y <= initial.y, "{tab:?}");
        assert_eq!(min.y, MIN_HEIGHT, "the shortest height the core ever asks for");
        assert_eq!(window.app_id.as_deref(), Some("deskpuck"), "matches deskpuck.desktop");
    }
}

#[test]
fn the_window_can_shrink_to_the_narrowest_tab() {
    let narrowest = Tab::ALL.map(width).into_iter().fold(f32::INFINITY, f32::min);
    assert_eq!(narrowest, MIN_WIDTH);
    assert_eq!(viewport(width(Tab::Buttons), None).min_inner_size.unwrap().x, MIN_WIDTH);
    assert!(width(Tab::Buttons) > width(Tab::General), "two columns need more room");
}

#[test]
fn the_icon_is_set_when_there_is_one() {
    assert!(viewport(440.0, None).icon.is_none());
    let icon = IconData { rgba: vec![0; 4], width: 1, height: 1 };
    assert!(viewport(440.0, Some(icon)).icon.is_some());
}
