use std::{fs, path::Path};

use stellr_showcase::{SvgSafetyError, validate_svg_safety};

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the showcase crate should live under crates/")
}

#[test]
fn readme_offers_animated_and_static_constellation_paths() {
    let readme = fs::read_to_string(repository_root().join("README.md"))
        .expect("the repository README should be readable");

    for required in [
        "<picture>",
        "media=\"(prefers-reduced-motion: reduce)\"",
        "srcset=\"docs/assets/readme-showcase/compatibility-probe.png\"",
        "src=\"docs/assets/readme-showcase/compatibility-probe.svg\"",
        "alt=\"Stellr release constellation compatibility probe\"",
        "[View the static release constellation](docs/assets/readme-showcase/compatibility-probe.png)",
    ] {
        assert!(
            readme.contains(required),
            "README delivery contract is missing {required:?}"
        );
    }
}

#[test]
fn probe_svg_is_script_free_self_contained_and_motion_safe() {
    let svg = fs::read_to_string(
        repository_root().join("docs/assets/readme-showcase/compatibility-probe.svg"),
    )
    .expect("the compatibility probe SVG should be readable");

    for required in [
        "viewBox=\"0 0 1200 675\"",
        "<title",
        "<desc",
        "@keyframes",
        "animation-duration: 12s",
        "animation-iteration-count: infinite",
        "@media (prefers-reduced-motion: reduce)",
    ] {
        assert!(svg.contains(required), "probe SVG is missing {required:?}");
    }

    validate_svg_safety(&svg).expect("probe SVG should pass the exporter safety gate");
}

#[test]
fn probe_status_captions_follow_the_replayed_state() {
    let svg = fs::read_to_string(
        repository_root().join("docs/assets/readme-showcase/compatibility-probe.svg"),
    )
    .expect("the compatibility probe SVG should be readable");

    for (node, states) in [
        ("a", &["blocked", "resolved"][..]),
        ("b", &["blocked", "ready", "resolved"][..]),
        ("c", &["blocked", "ready", "resolved"][..]),
    ] {
        for state in states {
            assert!(
                svg.contains(&format!("class=\"status-{node}-{state} animated\"")),
                "probe SVG is missing the {node} {state} caption layer"
            );
            assert!(
                svg.contains(&format!("@keyframes status-{node}-{state}")),
                "probe SVG is missing timing for the {node} {state} caption"
            );
        }
    }

    assert!(
        svg.contains("animation-timing-function: step-end;"),
        "status captions should switch discretely instead of overlapping"
    );
}

#[test]
fn svg_safety_gate_rejects_active_and_external_content() {
    for unsafe_svg in [
        r#"<svg xmlns="http://www.w3.org/2000/svg" OnLoad="alert(1)" />"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><ScRiPt /></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject /></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><image href=' https://example.test/a.png' /></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><use xlink:href='https://example.test/a.svg#node' /></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><set attributeName="href" to="https://example.test/a.svg" /></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><style>@IMPORT 'https://example.test/a.css';</style></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.x { fill: URL( 'https://example.test/a.svg#paint' ); }</style></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><style>@keyframes drift { from { transform: translate(0 0); } to { transform: translate(10px, 0); } }</style></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><style>@keyframes drift { from { x: 0; } to { x: 10px; } }</style></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><style>@keyframes drift { from { y: 0; } to { y: 10px; } }</style></svg>"#,
    ] {
        assert!(
            validate_svg_safety(unsafe_svg).is_err(),
            "unsafe SVG unexpectedly passed: {unsafe_svg}"
        );
    }
}

#[test]
fn svg_safety_gate_fails_closed_on_malformed_or_oversized_input() {
    assert!(validate_svg_safety("<svg").is_err());

    let oversized = " ".repeat(750 * 1024 + 1);
    assert!(matches!(
        validate_svg_safety(&oversized),
        Err(SvgSafetyError::TooLarge { .. })
    ));
}

#[test]
fn probe_png_is_a_1600_by_900_static_poster() {
    let png =
        fs::read(repository_root().join("docs/assets/readme-showcase/compatibility-probe.png"))
            .expect("the compatibility probe PNG should be readable");

    assert_eq!(
        png.get(..8),
        Some(&[137, 80, 78, 71, 13, 10, 26, 10][..]),
        "probe poster must use the PNG signature"
    );
    assert_eq!(&png[12..16], b"IHDR", "probe poster must begin with IHDR");

    let width = u32::from_be_bytes(png[16..20].try_into().expect("PNG width bytes"));
    let height = u32::from_be_bytes(png[20..24].try_into().expect("PNG height bytes"));
    assert_eq!((width, height), (1600, 900));
}
