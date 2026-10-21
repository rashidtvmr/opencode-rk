#[path = "../src/native_box_style.rs"]
mod native_box_style;

use native_box_style::{
    BRAILLE_FRAMES, BorderStyle, BoxAppearance, BoxId, BoxOverride, IOS_SQUIRCLE_APPROXIMATION,
    LOADER_FRAME_MS, MAX_PADDING, MAX_RADIUS, RadiusControl, RadiusKey, RadiusPreset, frame,
    radius_after_key, radius_from_pointer,
};
use std::collections::BTreeMap;

const SHARP: [char; 4] = ['┌', '┐', '└', '┘'];
const ROUNDED: [char; 4] = ['╭', '╮', '╰', '╯'];

#[test]
fn default_matches_existing_single_border_geometry() {
    let appearance = BoxAppearance::default();
    assert_eq!(appearance.preset, RadiusPreset::Sharp);
    assert_eq!(appearance.radius, 0);
    assert_eq!(appearance.border_style, BorderStyle::Single);
    assert_eq!(appearance.thickness, 1);
    assert_eq!(appearance.padding, 0);
    assert!(appearance.overrides.is_empty());
    for id in BoxId::ALL {
        assert_eq!(appearance.corner_glyphs(id).to_array(), SHARP);
    }
}

#[test]
fn global_preset_merges_with_per_box_overrides() {
    let appearance: BoxAppearance = serde_json::from_value(serde_json::json!({
        "preset": "custom",
        "radius": 30,
        "padding": 1,
        "overrides": {
            "chatcomposer": { "preset": "rounded", "padding": 2 },
            "user": { "radius": 0 },
            "dialog": { "border_style": "double", "thickness": 2, "padding": 4 }
        }
    }))
    .expect("valid appearance config");

    let composer = appearance.effective(BoxId::ChatComposer);
    assert_eq!(composer.preset, RadiusPreset::Rounded);
    assert_eq!(composer.radius, 30);
    assert_eq!(composer.padding, 2);
    assert_eq!(
        appearance.corner_glyphs(BoxId::ChatComposer).to_array(),
        ROUNDED
    );

    let user = appearance.effective(BoxId::User);
    assert_eq!(user.preset, RadiusPreset::Custom);
    assert_eq!(user.radius, 0);
    assert_eq!(appearance.corner_glyphs(BoxId::User).to_array(), SHARP);

    let assistant = appearance.effective(BoxId::Assistant);
    assert_eq!(assistant.radius, 30);
    assert_eq!(assistant.padding, 1);
    assert_eq!(
        appearance.corner_glyphs(BoxId::Assistant).to_array(),
        ROUNDED
    );

    let dialog = appearance.effective(BoxId::Dialog);
    assert_eq!(dialog.border_style, BorderStyle::Double);
    assert_eq!(dialog.thickness, 2);
    assert_eq!(dialog.padding, 4);
}

#[test]
fn sharp_rounded_and_squircle_glyphs_are_honest() {
    let sharp = BoxAppearance::default();
    assert_eq!(sharp.corner_glyphs(BoxId::ChatComposer).to_array(), SHARP);

    let rounded = BoxAppearance {
        preset: RadiusPreset::Rounded,
        ..BoxAppearance::default()
    };
    assert_eq!(
        rounded.corner_glyphs(BoxId::ChatComposer).to_array(),
        ROUNDED
    );

    // The iOS preset is only a terminal-cell approximation: radius has no
    // intermediate glyphs and never claims real continuous curvature.
    let squircle = BoxAppearance {
        preset: RadiusPreset::IosSquircle,
        radius: 0,
        ..BoxAppearance::default()
    };
    assert_eq!(
        squircle.corner_glyphs(BoxId::ChatComposer).to_array(),
        ROUNDED
    );
    let squircle_max = BoxAppearance {
        radius: MAX_RADIUS,
        ..squircle
    };
    assert_eq!(
        squircle_max.corner_glyphs(BoxId::ChatComposer).to_array(),
        ROUNDED
    );
    assert!(IOS_SQUIRCLE_APPROXIMATION.contains("approximation"));
    assert!(IOS_SQUIRCLE_APPROXIMATION.contains("no continuous curvature"));

    // Custom is a two-state CLI approximation: 0 is sharp, any positive
    // radius uses the rounded corner cells.
    for radius in [1, 25, 50] {
        let custom = BoxAppearance {
            preset: RadiusPreset::Custom,
            radius,
            ..BoxAppearance::default()
        };
        assert_eq!(
            custom.corner_glyphs(BoxId::ChatComposer).to_array(),
            ROUNDED
        );
    }
    let custom_zero = BoxAppearance {
        preset: RadiusPreset::Custom,
        radius: 0,
        ..BoxAppearance::default()
    };
    assert_eq!(
        custom_zero.corner_glyphs(BoxId::ChatComposer).to_array(),
        SHARP
    );

    // Preset wins over the numeric radius; radius is a future pixel hint.
    let sharp_with_radius = BoxAppearance {
        radius: 50,
        ..BoxAppearance::default()
    };
    assert_eq!(
        sharp_with_radius
            .corner_glyphs(BoxId::ChatComposer)
            .to_array(),
        SHARP
    );
    assert_eq!(
        sharp_with_radius.effective(BoxId::Menu).normalized_radius(),
        1.0
    );
}

#[test]
fn pointer_drag_clamps_and_maps_to_logical_radius() {
    assert_eq!(radius_from_pointer(51, -10), 0);
    assert_eq!(radius_from_pointer(51, 0), 0);
    assert_eq!(radius_from_pointer(51, 25), 25);
    assert_eq!(radius_from_pointer(51, 50), MAX_RADIUS);
    assert_eq!(radius_from_pointer(51, 999), MAX_RADIUS);
    assert_eq!(radius_from_pointer(0, 10), 0);
    assert_eq!(radius_from_pointer(1, 10), 0);
    assert_eq!(radius_from_pointer(2, 0), 0);
    assert_eq!(radius_from_pointer(2, 1), MAX_RADIUS);
    assert_eq!(radius_from_pointer(101, 50), 25);
}

#[test]
fn keyboard_and_control_model_clamp() {
    assert_eq!(radius_after_key(0, RadiusKey::Decrease), 0);
    assert_eq!(radius_after_key(49, RadiusKey::Increase), 50);
    assert_eq!(radius_after_key(50, RadiusKey::Increase), 50);
    assert_eq!(radius_after_key(25, RadiusKey::Home), 0);
    assert_eq!(radius_after_key(25, RadiusKey::End), 50);
    assert_eq!(radius_after_key(200, RadiusKey::Decrease), 49);

    let mut control = RadiusControl::new(99);
    assert_eq!(control.radius(), MAX_RADIUS);
    control.key(RadiusKey::Home);
    assert_eq!(control.radius(), 0);
    control.pointer(51, 25);
    assert_eq!(control.radius(), 25);
    control.key(RadiusKey::End);
    assert_eq!(control.radius(), MAX_RADIUS);
}

#[test]
fn serde_rejects_out_of_bounds_and_unknown_boxes() {
    for invalid in [
        serde_json::json!({ "radius": 51 }),
        serde_json::json!({ "padding": 5 }),
        serde_json::json!({ "thickness": 0 }),
        serde_json::json!({ "thickness": 3 }),
        serde_json::json!({ "overrides": { "popup": {} } }),
        serde_json::json!({ "overrides": { "chatcomposer": { "radius": 51 } } }),
        serde_json::json!({ "overrides": { "chatcomposer": { "padding": 5 } } }),
        serde_json::json!({ "unknown": true }),
        serde_json::json!({ "overrides": { "chatcomposer": { "unknown": true } } }),
    ] {
        assert!(
            serde_json::from_value::<BoxAppearance>(invalid.clone()).is_err(),
            "must reject {invalid}"
        );
    }

    let valid = serde_json::json!({
        "preset": "custom",
        "radius": 50,
        "padding": 4,
        "thickness": 2,
        "overrides": { "sidebar": { "radius": 0, "padding": 0 } }
    });
    let parsed: BoxAppearance = serde_json::from_value(valid.clone()).expect("bounds inclusive");
    assert_eq!(parsed.radius, 50);
    assert_eq!(parsed.padding, MAX_PADDING);
    assert_eq!(serde_json::to_value(&parsed).expect("serializes"), valid);
    assert_eq!(parsed.validate(), Ok(()));
}

#[test]
fn every_box_key_round_trips_and_unknown_values_stay_bounded() {
    let overrides = BoxId::ALL
        .into_iter()
        .map(|id| {
            (
                id,
                BoxOverride {
                    preset: Some(RadiusPreset::Rounded),
                    radius: Some(0),
                    border_style: Some(BorderStyle::Single),
                    thickness: Some(1),
                    padding: Some(0),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let appearance = BoxAppearance {
        overrides,
        ..BoxAppearance::default()
    };
    let json = serde_json::to_string(&appearance).expect("serializes");
    let parsed: BoxAppearance = serde_json::from_str(&json).expect("round-trips");
    assert_eq!(parsed, appearance);

    for id in BoxId::ALL {
        assert_eq!(
            parsed.effective(id).corner_glyphs().to_array(),
            ROUNDED,
            "{id:?}"
        );
    }

    let mut corrupt = BoxAppearance::default();
    corrupt.radius = 51;
    assert!(corrupt.validate().is_err());
    corrupt.radius = 0;
    corrupt.padding = 5;
    assert!(corrupt.validate().is_err());
    corrupt.padding = 0;
    corrupt.thickness = 0;
    assert!(corrupt.validate().is_err());
    corrupt.thickness = 1;
    corrupt.overrides.insert(
        BoxId::Tool,
        BoxOverride {
            radius: Some(51),
            ..BoxOverride::default()
        },
    );
    assert!(corrupt.validate().is_err());
}

#[test]
fn loader_frames_are_deterministic_bounded_braille() {
    for elapsed in [0, 1, 79, 80, 159, 800, 12_345, u64::MAX] {
        let cell = frame(elapsed);
        assert!(
            ('\u{2800}'..='\u{28FF}').contains(&cell),
            "{elapsed}: {cell:?}"
        );
        assert_eq!(cell, frame(elapsed));
    }
    assert_eq!(frame(0), BRAILLE_FRAMES[0]);
    assert_eq!(frame(LOADER_FRAME_MS), BRAILLE_FRAMES[1]);
    assert_eq!(
        frame(LOADER_FRAME_MS * BRAILLE_FRAMES.len() as u64),
        BRAILLE_FRAMES[0]
    );
}
