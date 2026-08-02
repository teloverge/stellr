use std::collections::BTreeMap;
use std::fmt::Write as _;

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};
use stellr_core::Status;
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

use crate::{MAX_SVG_BYTES, ReleaseStory, StoryEdge, validate_svg_safety};

const SVG_WIDTH: u32 = 1_200;
const SVG_HEIGHT: u32 = 675;
const PNG_WIDTH: u32 = 1_600;
const PNG_HEIGHT: u32 = 900;
const MAX_PNG_BYTES: usize = 1_536 * 1_024;
const MAX_MANIFEST_BYTES: usize = 1_024 * 1_024;
const CONTEXT_OPACITY: &str = "0.35";

/// Deterministic in-memory outputs for reviewing one final release scene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticPreview {
    pub svg: Vec<u8>,
    pub png: Vec<u8>,
    pub manifest: Vec<u8>,
    pub review_html: Vec<u8>,
}

/// A release story could not be rendered without violating the preview contract.
#[derive(Debug, Error)]
pub enum PreviewRenderError {
    #[error("release story is incomplete: {0}")]
    IncompleteStory(String),
    #[error("{kind} is {actual} bytes; the limit is {limit} bytes")]
    AssetTooLarge {
        kind: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("story manifest serialization failed: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error("SVG safety validation failed: {0}")]
    SvgSafety(String),
    #[error("SVG rasterization failed: {0}")]
    Rasterization(String),
}

#[derive(Debug, Clone)]
struct StaticScene {
    release_title: String,
    summary: String,
    nodes: Vec<StaticNode>,
    edges: Vec<StaticEdge>,
}

#[derive(Debug, Clone)]
struct StaticNode {
    issue_number: u64,
    label: String,
    x: u32,
    y: u32,
    status: Status,
    context: bool,
    label_above: bool,
}

#[derive(Debug, Clone)]
struct StaticEdge {
    blocker: u64,
    dependent: u64,
    from_x: u32,
    from_y: u32,
    to_x: u32,
    to_y: u32,
    resolved: bool,
    context: bool,
}

/// Renders the explicit final state of a release story without writing files.
pub fn render_static_preview(story: &ReleaseStory) -> Result<StaticPreview, PreviewRenderError> {
    let manifest = serde_json::to_vec(story)?;
    enforce_budget("manifest", manifest.len(), MAX_MANIFEST_BYTES)?;

    let scene = StaticScene::from_story(story)?;
    let svg = render_svg(&scene);
    enforce_budget("SVG", svg.len(), MAX_SVG_BYTES)?;
    validate_svg_safety(&svg).map_err(|error| PreviewRenderError::SvgSafety(error.to_string()))?;

    let png = rasterize_svg(&svg)?;
    enforce_budget("PNG", png.len(), MAX_PNG_BYTES)?;
    let review_html = render_review_html(&scene, &svg, &manifest).into_bytes();

    Ok(StaticPreview {
        svg: svg.into_bytes(),
        png,
        manifest,
        review_html,
    })
}

impl StaticScene {
    fn from_story(story: &ReleaseStory) -> Result<Self, PreviewRenderError> {
        if story.visible_issue_numbers.is_empty() {
            return Err(incomplete("visible constellation is empty"));
        }

        let issues = story
            .evidence
            .issues
            .iter()
            .map(|issue| (issue.number, issue))
            .collect::<BTreeMap<_, _>>();
        let statuses = story
            .final_statuses
            .iter()
            .map(|status| (status.issue_number, status.status))
            .collect::<BTreeMap<_, _>>();
        let coordinates = story
            .coordinates
            .iter()
            .map(|coordinate| (coordinate.issue_number, coordinate))
            .collect::<BTreeMap<_, _>>();

        let mut nodes = Vec::with_capacity(story.visible_issue_numbers.len());
        for (index, issue_number) in story.visible_issue_numbers.iter().enumerate() {
            let issue = issues.get(issue_number).ok_or_else(|| {
                incomplete(format!(
                    "visible issue #{issue_number} has no recorded evidence"
                ))
            })?;
            let status = statuses
                .get(issue_number)
                .copied()
                .flatten()
                .ok_or_else(|| {
                    incomplete(format!("visible issue #{issue_number} has no final status"))
                })?;
            let coordinate = coordinates.get(issue_number).ok_or_else(|| {
                incomplete(format!("visible issue #{issue_number} has no coordinate"))
            })?;
            if coordinate.x > SVG_WIDTH || coordinate.y > SVG_HEIGHT {
                return Err(incomplete(format!(
                    "visible issue #{issue_number} has an out-of-bounds coordinate"
                )));
            }
            nodes.push(StaticNode {
                issue_number: *issue_number,
                label: format!("#{issue_number} {}", bounded_title(&issue.title)),
                x: coordinate.x,
                y: coordinate.y,
                status,
                context: issue.milestone_id.as_deref() != Some(story.milestone.id.as_str()),
                label_above: index % 2 == 1,
            });
        }

        let by_number = nodes
            .iter()
            .map(|node| (node.issue_number, node))
            .collect::<BTreeMap<_, _>>();
        let edges = story
            .final_topology
            .iter()
            .map(|edge| static_edge(edge, &by_number))
            .collect::<Result<Vec<_>, _>>()?;

        let release_issue_count = nodes.iter().filter(|node| !node.context).count();
        let external_count = nodes.iter().filter(|node| node.context).count();
        let resolved_count = nodes
            .iter()
            .filter(|node| !node.context && node.status == Status::Resolved)
            .count();
        let summary = format!(
            "{} release {} · {} resolved · {} external {}",
            release_issue_count,
            plural(release_issue_count, "issue", "issues"),
            resolved_count,
            external_count,
            plural(external_count, "prerequisite", "prerequisites")
        );

        Ok(Self {
            release_title: format!("Release {}", story.release_version),
            summary,
            nodes,
            edges,
        })
    }
}

fn static_edge(
    edge: &StoryEdge,
    nodes: &BTreeMap<u64, &StaticNode>,
) -> Result<StaticEdge, PreviewRenderError> {
    let blocker = nodes.get(&edge.blocker).ok_or_else(|| {
        incomplete(format!(
            "edge {} -> {} has no visible blocker",
            edge.blocker, edge.dependent
        ))
    })?;
    let dependent = nodes.get(&edge.dependent).ok_or_else(|| {
        incomplete(format!(
            "edge {} -> {} has no visible dependent",
            edge.blocker, edge.dependent
        ))
    })?;
    Ok(StaticEdge {
        blocker: edge.blocker,
        dependent: edge.dependent,
        from_x: blocker.x,
        from_y: blocker.y,
        to_x: dependent.x,
        to_y: dependent.y,
        resolved: blocker.status == Status::Resolved,
        context: blocker.context || dependent.context,
    })
}

fn render_svg(scene: &StaticScene) -> String {
    let mut svg = String::with_capacity(8_192);
    let title = escape_xml(&scene.release_title);
    let summary = escape_xml(&scene.summary);
    write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{SVG_WIDTH}\" height=\"{SVG_HEIGHT}\" viewBox=\"0 0 {SVG_WIDTH} {SVG_HEIGHT}\" role=\"img\" aria-labelledby=\"release-title release-desc\"><title id=\"release-title\">{title}</title><desc id=\"release-desc\">{summary}. Final Stellr release constellation with directed blocker relationships.</desc><defs><marker id=\"arrow-resolved\" markerWidth=\"10\" markerHeight=\"10\" refX=\"9\" refY=\"5\" orient=\"auto\" markerUnits=\"userSpaceOnUse\"><path d=\"M0,1 L10,5 L0,9 z\" fill=\"#d9f3df\"/></marker><marker id=\"arrow-unresolved\" markerWidth=\"10\" markerHeight=\"10\" refX=\"9\" refY=\"5\" orient=\"auto\" markerUnits=\"userSpaceOnUse\"><path d=\"M0,1 L10,5 L0,9 z\" fill=\"#c8d5e8\"/></marker></defs><rect width=\"1200\" height=\"675\" fill=\"#000000\"/><g id=\"release-heading\" font-family=\"Roboto, sans-serif\"><text x=\"56\" y=\"70\" fill=\"#f4f7f5\" font-size=\"30\" font-weight=\"700\">{title}</text><text x=\"56\" y=\"103\" fill=\"#a2c1ac\" font-size=\"17\">How the release constellation came together</text><text x=\"56\" y=\"130\" fill=\"#94a3b8\" font-size=\"14\">{summary}</text></g><g id=\"dependencies\">"
    )
    .expect("writing to String cannot fail");

    for edge in &scene.edges {
        render_edge(&mut svg, edge);
    }
    svg.push_str("</g><g id=\"stars\" font-family=\"Roboto, sans-serif\">");
    for node in &scene.nodes {
        render_node(&mut svg, node);
    }
    svg.push_str("</g></svg>");
    svg
}

fn render_edge(svg: &mut String, edge: &StaticEdge) {
    let dx = edge.to_x as f64 - edge.from_x as f64;
    let dy = edge.to_y as f64 - edge.from_y as f64;
    let length = dx.hypot(dy).max(1.0);
    let unit_x = dx / length;
    let unit_y = dy / length;
    let from_x = edge.from_x as f64 + unit_x * 14.0;
    let from_y = edge.from_y as f64 + unit_y * 14.0;
    let to_x = edge.to_x as f64 - unit_x * 18.0;
    let to_y = edge.to_y as f64 - unit_y * 18.0;
    let bow = (length * 0.13).min(46.0);
    let control_x = (from_x + to_x) / 2.0 - unit_y * bow;
    let control_y = (from_y + to_y) / 2.0 + unit_x * bow;
    let (stroke, width, dash, marker) = if edge.resolved {
        ("#bee1c8", "3", "", "arrow-resolved")
    } else {
        (
            "#aec0da",
            "2.4",
            " stroke-dasharray=\"7 7\"",
            "arrow-unresolved",
        )
    };
    let opacity = if edge.context {
        format!(" opacity=\"{CONTEXT_OPACITY}\" data-prominence=\"context\"")
    } else {
        String::new()
    };
    write!(
        svg,
        "<path data-blocker=\"{}\" data-dependent=\"{}\" d=\"M {:.2} {:.2} Q {:.2} {:.2} {:.2} {:.2}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{width}\" stroke-linecap=\"round\"{dash} marker-end=\"url(#{marker})\"{opacity}/>",
        edge.blocker,
        edge.dependent,
        from_x,
        from_y,
        control_x,
        control_y,
        to_x,
        to_y
    )
    .expect("writing to String cannot fail");
}

fn render_node(svg: &mut String, node: &StaticNode) {
    let style = star_style(node.status);
    let opacity = if node.context {
        format!(" opacity=\"{CONTEXT_OPACITY}\" data-prominence=\"context\"")
    } else {
        String::new()
    };
    let completion = if node.status == Status::Resolved {
        "solid"
    } else {
        "hollow"
    };
    write!(
        svg,
        "<g data-issue=\"{}\" data-status=\"{}\" data-completion=\"{completion}\"{opacity}><circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" opacity=\"0.20\"/>",
        node.issue_number,
        status_name(node.status),
        node.x,
        node.y,
        style.glow_radius,
        style.glow
    )
    .expect("writing to String cannot fail");
    if completion == "solid" {
        write!(
            svg,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>",
            node.x, node.y, style.radius, style.core
        )
        .expect("writing to String cannot fail");
    } else {
        write!(
            svg,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#000000\" stroke=\"{}\" stroke-width=\"2.4\"/>",
            node.x, node.y, style.radius, style.core
        )
        .expect("writing to String cannot fail");
    }
    let label_y = if node.label_above {
        node.y.saturating_sub(style.radius as u32 + 22)
    } else {
        node.y + style.radius as u32 + 30
    };
    write!(
        svg,
        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"{}\" stroke=\"#000000\" stroke-width=\"4\" paint-order=\"stroke\" font-size=\"13\">{}</text></g>",
        node.x,
        label_y,
        style.label,
        escape_xml(&node.label)
    )
    .expect("writing to String cannot fail");
}

fn rasterize_svg(svg: &str) -> Result<Vec<u8>, PreviewRenderError> {
    let mut options = Options {
        font_family: "Roboto".to_owned(),
        ..Options::default()
    };
    let font_database = options.fontdb_mut();
    font_database.load_font_data(include_bytes!("../assets/fonts/Roboto-Regular.ttf").to_vec());
    font_database.set_sans_serif_family("Roboto");
    let tree = Tree::from_str(svg, &options)
        .map_err(|error| PreviewRenderError::Rasterization(error.to_string()))?;
    let mut pixmap = Pixmap::new(PNG_WIDTH, PNG_HEIGHT)
        .ok_or_else(|| PreviewRenderError::Rasterization("invalid PNG dimensions".to_owned()))?;
    let scale = PNG_WIDTH as f32 / SVG_WIDTH as f32;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|error| PreviewRenderError::Rasterization(error.to_string()))
}

fn render_review_html(scene: &StaticScene, svg: &str, manifest: &[u8]) -> String {
    let title = escape_xml(&scene.release_title);
    let summary = escape_xml(&scene.summary);
    let manifest = escape_xml(std::str::from_utf8(manifest).expect("JSON is UTF-8"));
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title} review</title><style>html{{color-scheme:dark;background:#000;font-family:Roboto,Arial,sans-serif}}body{{margin:0;padding:24px;color:#f4f7f5}}main{{max-width:1200px;margin:auto}}svg{{display:block;width:100%;height:auto;border:1px solid #243047}}details{{margin-top:20px}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;color:#a2c1ac}}</style></head><body><main><h1>{title}</h1><p>{summary}</p>{svg}<details><summary>Canonical story manifest</summary><pre>{manifest}</pre></details></main></body></html>"
    )
}

#[derive(Clone, Copy)]
struct StarStyle {
    core: &'static str,
    glow: &'static str,
    label: &'static str,
    radius: f32,
    glow_radius: u32,
}

fn star_style(status: Status) -> StarStyle {
    match status {
        Status::Resolved => StarStyle {
            core: "#b9d6c4",
            glow: "#5b9077",
            label: "#a2c1ac",
            radius: 8.0,
            glow_radius: 24,
        },
        Status::Frontier => StarStyle {
            core: "#8ad8ff",
            glow: "#2f9be0",
            label: "#b3e5ff",
            radius: 11.0,
            glow_radius: 49,
        },
        Status::Claimed => StarStyle {
            core: "#ffd873",
            glow: "#ffb020",
            label: "#ffe6a0",
            radius: 10.0,
            glow_radius: 36,
        },
        Status::Blocked => StarStyle {
            core: "#e2c3c3",
            glow: "#9a6f6f",
            label: "#d0b3b3",
            radius: 7.0,
            glow_radius: 20,
        },
        Status::OutOfScope => StarStyle {
            core: "#948da4",
            glow: "#6b6478",
            label: "#a89fb2",
            radius: 7.0,
            glow_radius: 18,
        },
    }
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Resolved => "resolved",
        Status::Frontier => "frontier",
        Status::Claimed => "claimed",
        Status::Blocked => "blocked",
        Status::OutOfScope => "out_of_scope",
    }
}

fn bounded_title(title: &str) -> String {
    let mut graphemes = title.graphemes(true);
    let bounded = graphemes.by_ref().take(40).collect::<String>();
    if graphemes.next().is_some() {
        format!("{bounded}…")
    } else {
        bounded
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

fn enforce_budget(
    kind: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), PreviewRenderError> {
    if actual > limit {
        return Err(PreviewRenderError::AssetTooLarge {
            kind,
            actual,
            limit,
        });
    }
    Ok(())
}

fn incomplete(detail: impl Into<String>) -> PreviewRenderError {
    PreviewRenderError::IncompleteStory(detail.into())
}
