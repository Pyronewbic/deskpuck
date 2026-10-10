use deskpuck_core::window::{MIN_HEIGHT, moved_top, wanted_height};

#[test]
fn the_height_fits_the_content_up_to_most_of_the_screen() {
    let (header, footer) = (50.0, 60.0);
    assert_eq!(wanted_height(400.0, header, footer, Some(1000.0)), 510.0);
    assert_eq!(wanted_height(740.0, header, footer, Some(1000.0)), 850.0, "exactly at the cap");
    assert_eq!(
        wanted_height(741.0, header, footer, Some(1000.0)),
        850.0,
        "above it, content scrolls"
    );
    assert_eq!(wanted_height(5000.0, header, footer, Some(1000.0)), 850.0);
    assert_eq!(wanted_height(400.0, header, footer, Some(100.0)), MIN_HEIGHT, "tiny monitor");
    assert_eq!(wanted_height(10.0, 0.0, 0.0, None), MIN_HEIGHT);
}

#[test]
fn an_unknown_screen_size_caps_nothing() {
    for monitor in [None, Some(0.0), Some(-5.0), Some(f32::NAN), Some(f32::INFINITY)] {
        assert_eq!(wanted_height(2000.0, 50.0, 60.0, monitor), 2110.0, "{monitor:?}");
    }
}

#[test]
fn the_window_is_centred_once_then_only_lifted_to_stay_on_screen() {
    assert_eq!(moved_top(true, 500.0, 400.0, 1000.0), Some(300.0));
    assert_eq!(moved_top(true, 0.0, 1200.0, 1000.0), Some(0.0), "taller than the monitor");
    assert_eq!(moved_top(false, 100.0, 400.0, 1000.0), None, "a user's placement is kept");
    assert_eq!(moved_top(false, 600.0, 400.0, 1000.0), None, "exactly at the bottom");
    assert_eq!(moved_top(false, 601.0, 400.0, 1000.0), Some(600.0));
    assert_eq!(moved_top(false, 300.0, 1200.0, 1000.0), Some(0.0));
}

#[test]
fn an_unknown_screen_size_moves_nothing() {
    for monitor in [0.0, -5.0, f32::NAN, f32::INFINITY] {
        assert_eq!(moved_top(true, 500.0, 400.0, monitor), None, "{monitor}");
        assert_eq!(moved_top(false, 500.0, 400.0, monitor), None, "{monitor}");
    }
    assert_eq!(moved_top(false, 601.0, 400.0, 1000.0), Some(600.0), "a real monitor still lifts");
}
