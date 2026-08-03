use std::collections::BTreeMap;
use std::fmt::Write as _;

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};
use stellr_core::Status;
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    IssueStatus, LifecycleEvent, MAX_SVG_BYTES, ReleaseBoundaries, ReleaseEvidence, ReleaseStory,
    StoryBeat, StoryEdge, validate_svg_safety,
};

const SVG_WIDTH: u32 = 1_200;
const SVG_HEIGHT: u32 = 675;
const PNG_WIDTH: u32 = 1_600;
const PNG_HEIGHT: u32 = 900;
pub(crate) const MAX_PNG_BYTES: usize = 1_536 * 1_024;
pub(crate) const MAX_MANIFEST_BYTES: usize = 1_024 * 1_024;
const CONTEXT_OPACITY: &str = "0.35";
const LOOP_MILLISECONDS: u32 = 12_000;
const REVEAL_MILLISECONDS: u32 = 1_000;
const REPLAY_MILLISECONDS: u32 = 8_000;
const FINAL_HOLD_MILLISECONDS: u32 = 2_000;
const SOFT_RESET_MILLISECONDS: u32 = 1_000;
const FINAL_HOLD_START_MILLISECONDS: u32 = 9_000;
const SOFT_RESET_START_MILLISECONDS: u32 = 11_000;
const TRANSITION_MILLISECONDS: u32 = 250;

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
    status: Option<Status>,
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

#[derive(Debug, Clone)]
struct Replay {
    initial_scene: StaticScene,
    beats: Vec<ReplayBeat>,
}

#[derive(Debug, Clone)]
struct ReplayBeat {
    index: usize,
    replay_offset_milliseconds: u32,
    activation_milliseconds: u32,
    source_event_ids: Vec<String>,
    changed_issues: Vec<u64>,
    primary_issue: u64,
    ready_issues: Vec<u64>,
    caption: &'static str,
    scene: StaticScene,
    newly_traversable_edges: Vec<StaticEdge>,
}

/// Renders the explicit final state of a release story without writing files.
pub fn render_static_preview(story: &ReleaseStory) -> Result<StaticPreview, PreviewRenderError> {
    let manifest = serde_json::to_vec(story)?;
    enforce_budget("manifest", manifest.len(), MAX_MANIFEST_BYTES)?;

    let scene = StaticScene::from_story(story)?;
    let replay = build_replay(story)?;
    validate_canonical_story(story)?;
    let svg = render_svg(&scene, &replay);
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

fn validate_canonical_story(story: &ReleaseStory) -> Result<(), PreviewRenderError> {
    let boundaries = ReleaseBoundaries {
        starting_cutoff: story
            .boundaries
            .previous_release
            .is_none()
            .then_some(story.boundaries.starting_cutoff),
        previous_release: story.boundaries.previous_release.clone(),
        ending_cutoff: Some(story.boundaries.ending_cutoff),
    };
    let evidence = ReleaseEvidence {
        repository: story.repository.clone(),
        release_version: story.release_version.clone(),
        milestone: story.milestone.clone(),
        issues: story.evidence.issues.clone(),
        events: story
            .evidence
            .events
            .iter()
            .map(|event| LifecycleEvent {
                provider_event_id: event.provider_event_id.clone(),
                occurred_at: event.occurred_at,
                issue_number: event.issue_number,
                kind: event.kind.clone(),
            })
            .collect(),
    };
    let canonical = ReleaseStory::build(evidence, boundaries).map_err(|error| {
        incomplete(format!(
            "release story cannot be reconstructed from evidence: {error}"
        ))
    })?;
    if canonical != *story {
        return Err(incomplete(
            "release story does not match the canonical replay derived from evidence",
        ));
    }
    Ok(())
}

impl StaticScene {
    fn from_story(story: &ReleaseStory) -> Result<Self, PreviewRenderError> {
        Self::from_statuses(story, &story.final_statuses, true)
    }

    fn from_statuses(
        story: &ReleaseStory,
        frame_statuses: &[IssueStatus],
        require_complete: bool,
    ) -> Result<Self, PreviewRenderError> {
        if story.visible_issue_numbers.is_empty() {
            return Err(incomplete("visible constellation is empty"));
        }

        let issues = story
            .evidence
            .issues
            .iter()
            .map(|issue| (issue.number, issue))
            .collect::<BTreeMap<_, _>>();
        let statuses = frame_statuses
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
            let status = statuses.get(issue_number).copied().ok_or_else(|| {
                incomplete(format!(
                    "visible issue #{issue_number} has no status entry in the rendered frame"
                ))
            })?;
            if require_complete && status.is_none() {
                return Err(incomplete(format!(
                    "visible issue #{issue_number} has no final status"
                )));
            }
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
            .filter(|node| !node.context && node.status == Some(Status::Resolved))
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

fn build_replay(story: &ReleaseStory) -> Result<Replay, PreviewRenderError> {
    if story.beats.is_empty() {
        return Err(incomplete("release story has no animation beats"));
    }

    let mut previous_statuses = status_map(&story.initial_statuses);
    let mut previous_offset = 0;
    let mut replay = Vec::with_capacity(story.beats.len());

    for (position, beat) in story.beats.iter().enumerate() {
        validate_beat_evidence(story, beat, position, previous_offset)?;
        let current_statuses = status_map(&beat.statuses);
        let changed_issues = story
            .visible_issue_numbers
            .iter()
            .copied()
            .filter(|issue_number| {
                previous_statuses.get(issue_number) != current_statuses.get(issue_number)
            })
            .collect::<Vec<_>>();
        if changed_issues.is_empty() {
            return Err(incomplete(format!(
                "animation beat {} contains no visible status change",
                beat.index
            )));
        }

        let primary_issue = changed_issues
            .iter()
            .copied()
            .min_by_key(|issue_number| {
                (
                    focus_priority(current_statuses.get(issue_number).copied().flatten()),
                    *issue_number,
                )
            })
            .expect("checked nonempty changed issues");
        let ready_issues = changed_issues
            .iter()
            .copied()
            .filter(|issue_number| {
                current_statuses.get(issue_number).copied().flatten() == Some(Status::Frontier)
            })
            .collect::<Vec<_>>();
        let scene = StaticScene::from_statuses(story, &beat.statuses, false)?;
        let newly_traversable_edges = scene
            .edges
            .iter()
            .filter(|edge| {
                edge.resolved
                    && previous_statuses.get(&edge.blocker).copied().flatten()
                        != Some(Status::Resolved)
            })
            .cloned()
            .collect();
        let caption = status_caption(current_statuses.get(&primary_issue).copied().flatten());

        replay.push(ReplayBeat {
            index: beat.index,
            replay_offset_milliseconds: beat.replay_offset_milliseconds,
            activation_milliseconds: REVEAL_MILLISECONDS + beat.replay_offset_milliseconds,
            source_event_ids: beat.source_event_ids.clone(),
            changed_issues,
            primary_issue,
            ready_issues,
            caption,
            scene,
            newly_traversable_edges,
        });
        previous_statuses = current_statuses;
        previous_offset = beat.replay_offset_milliseconds;
    }

    if replay.last().map(|beat| beat.activation_milliseconds) != Some(FINAL_HOLD_START_MILLISECONDS)
    {
        return Err(incomplete(
            "final animation beat must complete at the nine-second final hold",
        ));
    }

    Ok(Replay {
        initial_scene: StaticScene::from_statuses(story, &story.initial_statuses, false)?,
        beats: replay,
    })
}

fn validate_beat_evidence(
    story: &ReleaseStory,
    beat: &StoryBeat,
    position: usize,
    previous_offset: u32,
) -> Result<(), PreviewRenderError> {
    if beat.index != position {
        return Err(incomplete(format!(
            "animation beat index {} is out of sequence at position {position}",
            beat.index
        )));
    }
    if beat.replay_offset_milliseconds <= previous_offset
        || beat.replay_offset_milliseconds > REPLAY_MILLISECONDS
    {
        return Err(incomplete(format!(
            "animation beat {} has invalid replay offset {}",
            beat.index, beat.replay_offset_milliseconds
        )));
    }
    let evidence_ids = story
        .evidence
        .events
        .iter()
        .filter(|event| event.beat_index == Some(beat.index))
        .map(|event| event.provider_event_id.as_str())
        .collect::<Vec<_>>();
    let beat_ids = beat
        .source_event_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if beat_ids.is_empty() || beat_ids != evidence_ids {
        return Err(incomplete(format!(
            "animation beat {} does not map exactly to its manifest evidence",
            beat.index
        )));
    }
    Ok(())
}

fn status_map(statuses: &[IssueStatus]) -> BTreeMap<u64, Option<Status>> {
    statuses
        .iter()
        .map(|status| (status.issue_number, status.status))
        .collect()
}

fn focus_priority(status: Option<Status>) -> u8 {
    match status {
        Some(Status::Claimed) => 0,
        Some(Status::Resolved) => 1,
        Some(Status::Frontier) => 2,
        _ => 3,
    }
}

fn status_caption(status: Option<Status>) -> &'static str {
    match status {
        Some(Status::Claimed) => "Claimed",
        Some(Status::Resolved) => "Resolved",
        Some(Status::Frontier) => "Ready",
        Some(Status::Blocked) => "Blocked",
        Some(Status::OutOfScope) => "Out of scope",
        None => "Not created",
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
        resolved: blocker.status == Some(Status::Resolved),
        context: blocker.context || dependent.context,
    })
}

fn render_svg(scene: &StaticScene, replay: &Replay) -> String {
    let mut svg = String::with_capacity(32_768);
    let title = escape_xml(&scene.release_title);
    let summary = escape_xml(&scene.summary);
    let motion_css = render_motion_css(replay);
    let final_beat = replay
        .beats
        .last()
        .expect("validated nonempty replay")
        .index;
    write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{SVG_WIDTH}\" height=\"{SVG_HEIGHT}\" viewBox=\"0 0 {SVG_WIDTH} {SVG_HEIGHT}\" role=\"img\" aria-labelledby=\"release-title release-desc\"><title id=\"release-title\">{title}</title><desc id=\"release-desc\">{summary}. Twelve-second Stellr release replay with a meaningful final-state fallback and directed blocker relationships.</desc><style>{motion_css}</style><defs><marker id=\"arrow-resolved\" markerWidth=\"10\" markerHeight=\"10\" refX=\"9\" refY=\"5\" orient=\"auto\" markerUnits=\"userSpaceOnUse\"><path d=\"M0,1 L10,5 L0,9 z\" fill=\"#d9f3df\"/></marker><marker id=\"arrow-unresolved\" markerWidth=\"10\" markerHeight=\"10\" refX=\"9\" refY=\"5\" orient=\"auto\" markerUnits=\"userSpaceOnUse\"><path d=\"M0,1 L10,5 L0,9 z\" fill=\"#c8d5e8\"/></marker></defs><rect width=\"1200\" height=\"675\" fill=\"#000000\"/><g id=\"release-heading\" class=\"motion\" font-family=\"Roboto, sans-serif\"><text x=\"56\" y=\"70\" fill=\"#f4f7f5\" font-size=\"30\" font-weight=\"700\">{title}</text><text x=\"56\" y=\"103\" fill=\"#a2c1ac\" font-size=\"17\">How the release constellation came together</text><text x=\"56\" y=\"130\" fill=\"#94a3b8\" font-size=\"14\">{summary}</text></g><g id=\"final-scene\" class=\"motion\" data-static-state=\"final\" data-state-after-beat=\"{final_beat}\">"
    )
    .expect("writing to String cannot fail");
    render_scene(&mut svg, scene, true);
    write!(
        svg,
        "</g><g id=\"animated-replay\" data-loop-ms=\"{LOOP_MILLISECONDS}\" data-reveal-ms=\"{REVEAL_MILLISECONDS}\" data-replay-ms=\"{REPLAY_MILLISECONDS}\" data-final-hold-ms=\"{FINAL_HOLD_MILLISECONDS}\" data-soft-reset-ms=\"{SOFT_RESET_MILLISECONDS}\"><g id=\"frame-initial\" class=\"motion\" data-frame=\"initial\" opacity=\"0\">"
    )
    .expect("writing to String cannot fail");
    render_scene(&mut svg, &replay.initial_scene, false);
    svg.push_str("</g>");

    for beat in replay.beats.iter().take(replay.beats.len() - 1) {
        write!(
            svg,
            "<g id=\"frame-beat-{}\" class=\"motion\" data-state-after-beat=\"{}\" opacity=\"0\">",
            beat.index, beat.index
        )
        .expect("writing to String cannot fail");
        render_scene(&mut svg, &beat.scene, false);
        svg.push_str("</g>");
    }
    for beat in &replay.beats {
        render_beat_focus(&mut svg, beat);
    }
    svg.push_str("</g></svg>");
    svg
}

fn render_motion_css(replay: &Replay) -> String {
    let mut css = String::with_capacity(4_096);
    css.push_str(".motion{animation-duration:12s;animation-timing-function:linear;animation-iteration-count:infinite}#release-heading{animation-name:heading-loop}#final-scene{animation-name:frame-final}");
    for beat in replay.beats.iter().take(replay.beats.len() - 1) {
        write!(
            css,
            "#frame-beat-{}{{animation-name:frame-beat-{}}}",
            beat.index, beat.index
        )
        .expect("writing to String cannot fail");
    }
    css.push_str("#frame-initial{animation-name:frame-initial}");
    for beat in &replay.beats {
        write!(
            css,
            "#beat-focus-{}{{animation-name:beat-focus-{}}}",
            beat.index, beat.index
        )
        .expect("writing to String cannot fail");
        for edge in &beat.newly_traversable_edges {
            write!(
                css,
                "#edge-motion-{}-{}-{}{{animation-name:edge-motion-{}-{}-{}}}",
                beat.index, edge.blocker, edge.dependent, beat.index, edge.blocker, edge.dependent
            )
            .expect("writing to String cannot fail");
        }
    }
    css.push_str("@keyframes heading-loop{0%{opacity:.2}8.333%{opacity:1}91.667%{opacity:1}100%{opacity:.2}}");

    let first_activation = replay.beats[0].activation_milliseconds;
    write!(
        css,
        "@keyframes frame-initial{{0%{{opacity:.2}}8.333%{{opacity:1}}{}{{opacity:1}}{}{{opacity:0}}91.667%{{opacity:0}}100%{{opacity:.2}}}}",
        loop_percentage(first_activation.saturating_sub(TRANSITION_MILLISECONDS)),
        loop_percentage(first_activation)
    )
    .expect("writing to String cannot fail");

    for window in replay.beats.windows(2) {
        let beat = &window[0];
        let next = &window[1];
        append_visibility_keyframes(
            &mut css,
            &format!("frame-beat-{}", beat.index),
            beat.activation_milliseconds,
            next.activation_milliseconds,
        );
    }
    let final_activation = replay
        .beats
        .last()
        .expect("validated nonempty replay")
        .activation_milliseconds;
    write!(
        css,
        "@keyframes frame-final{{0%{{opacity:0}}{}{{opacity:0}}{}{{opacity:1}}91.667%{{opacity:1}}100%{{opacity:0}}}}",
        loop_percentage(final_activation.saturating_sub(TRANSITION_MILLISECONDS)),
        loop_percentage(final_activation)
    )
    .expect("writing to String cannot fail");

    for (position, beat) in replay.beats.iter().enumerate() {
        let end = replay
            .beats
            .get(position + 1)
            .map_or(SOFT_RESET_START_MILLISECONDS, |next| {
                next.activation_milliseconds
            });
        append_visibility_keyframes(
            &mut css,
            &format!("beat-focus-{}", beat.index),
            beat.activation_milliseconds,
            end,
        );
        for edge in &beat.newly_traversable_edges {
            write!(
                css,
                "@keyframes edge-motion-{}-{}-{}{{0%{{stroke-dashoffset:24}}{}{{stroke-dashoffset:24}}{}{{stroke-dashoffset:-24}}100%{{stroke-dashoffset:-24}}}}",
                beat.index,
                edge.blocker,
                edge.dependent,
                loop_percentage(beat.activation_milliseconds),
                loop_percentage(end)
            )
            .expect("writing to String cannot fail");
        }
    }
    css.push_str("@media (prefers-reduced-motion:reduce){#animated-replay{display:none}#release-heading{animation:none;opacity:1}#final-scene{animation:none;opacity:1}}");
    css
}

fn append_visibility_keyframes(css: &mut String, name: &str, start: u32, end: u32) {
    write!(
        css,
        "@keyframes {name}{{0%{{opacity:0}}{}{{opacity:0}}{}{{opacity:1}}{}{{opacity:1}}{}{{opacity:0}}100%{{opacity:0}}}}",
        loop_percentage(start.saturating_sub(TRANSITION_MILLISECONDS)),
        loop_percentage(start),
        loop_percentage(end.saturating_sub(TRANSITION_MILLISECONDS)),
        loop_percentage(end)
    )
    .expect("writing to String cannot fail");
}

fn loop_percentage(milliseconds: u32) -> String {
    format!(
        "{:.3}%",
        milliseconds as f64 * 100.0 / LOOP_MILLISECONDS as f64
    )
}

fn render_scene(svg: &mut String, scene: &StaticScene, final_scene: bool) {
    if final_scene {
        svg.push_str("<g id=\"dependencies\" data-role=\"dependencies\">");
    } else {
        svg.push_str("<g data-role=\"dependencies\">");
    }
    for edge in &scene.edges {
        render_edge(svg, edge);
    }
    if final_scene {
        svg.push_str("</g><g id=\"stars\" data-role=\"stars\" font-family=\"Roboto, sans-serif\">");
    } else {
        svg.push_str("</g><g data-role=\"stars\" font-family=\"Roboto, sans-serif\">");
    }
    for node in &scene.nodes {
        render_node(svg, node);
    }
    svg.push_str("</g>");
}

fn render_beat_focus(svg: &mut String, beat: &ReplayBeat) {
    let event_ids = escape_xml(&beat.source_event_ids.join(" "));
    let changed_issues = beat
        .changed_issues
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    let ready_issues = beat
        .ready_issues
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    write!(
        svg,
        "<g id=\"beat-focus-{}\" class=\"motion\" data-role=\"beat-focus\" data-beat=\"{}\" data-replay-offset-ms=\"{}\" data-event-ids=\"{event_ids}\" data-changed-issues=\"{changed_issues}\" data-primary-issue=\"{}\" data-ready-issues=\"{ready_issues}\" opacity=\"0\">",
        beat.index, beat.index, beat.replay_offset_milliseconds, beat.primary_issue
    )
    .expect("writing to String cannot fail");

    let primary = beat
        .scene
        .nodes
        .iter()
        .find(|node| node.issue_number == beat.primary_issue)
        .expect("primary issue belongs to beat scene");
    let primary_radius = primary
        .status
        .map(|status| star_style(status).radius)
        .unwrap_or(8.0)
        + 18.0;
    write!(
        svg,
        "<circle data-focus=\"current\" data-issue=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{primary_radius}\" fill=\"none\" stroke=\"#f4f7f5\" stroke-width=\"2.2\"/>",
        primary.issue_number, primary.x, primary.y
    )
    .expect("writing to String cannot fail");
    for issue_number in &beat.ready_issues {
        let ready = beat
            .scene
            .nodes
            .iter()
            .find(|node| node.issue_number == *issue_number)
            .expect("ready issue belongs to beat scene");
        let radius = ready
            .status
            .map(|status| star_style(status).radius)
            .unwrap_or(8.0)
            + 13.0;
        write!(
            svg,
            "<circle data-focus=\"ready\" data-issue=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{radius}\" fill=\"none\" stroke=\"#8ad8ff\" stroke-width=\"2\" stroke-dasharray=\"3 5\"/>",
            ready.issue_number, ready.x, ready.y
        )
        .expect("writing to String cannot fail");
    }
    let caption_y = if primary.label_above {
        primary.y + 48
    } else {
        primary.y.saturating_sub(42)
    };
    write!(
        svg,
        "<text data-issue=\"{}\" data-caption=\"{}\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"#f4f7f5\" stroke=\"#000000\" stroke-width=\"4\" paint-order=\"stroke\" font-family=\"Roboto, sans-serif\" font-size=\"12\">CURRENT · {} · {}</text>",
        primary.issue_number,
        escape_xml(beat.caption),
        primary.x,
        caption_y,
        escape_xml(beat.caption),
        escape_xml(&primary.label)
    )
    .expect("writing to String cannot fail");
    for edge in &beat.newly_traversable_edges {
        render_edge_motion(svg, beat.index, edge);
    }
    svg.push_str("</g>");
}

fn render_edge_motion(svg: &mut String, beat_index: usize, edge: &StaticEdge) {
    let geometry = edge_geometry(edge);
    let opacity = if edge.context {
        format!(" opacity=\"{CONTEXT_OPACITY}\" data-prominence=\"context\"")
    } else {
        String::new()
    };
    write!(
        svg,
        "<path id=\"edge-motion-{beat_index}-{}-{}\" class=\"motion\" data-motion=\"newly-traversable\" data-beat=\"{beat_index}\" data-blocker=\"{}\" data-dependent=\"{}\" d=\"{}\" fill=\"none\" stroke=\"#f4fff7\" stroke-width=\"3\" stroke-linecap=\"round\" stroke-dasharray=\"2 12\"{opacity}/>",
        edge.blocker,
        edge.dependent,
        edge.blocker,
        edge.dependent,
        geometry.path
    )
    .expect("writing to String cannot fail");
}

struct EdgeGeometry {
    path: String,
}

fn edge_geometry(edge: &StaticEdge) -> EdgeGeometry {
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
    EdgeGeometry {
        path: format!(
            "M {from_x:.2} {from_y:.2} Q {control_x:.2} {control_y:.2} {to_x:.2} {to_y:.2}"
        ),
    }
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
    let Some(status) = node.status else {
        write!(
            svg,
            "<g data-issue=\"{}\" data-status=\"not-created\" data-completion=\"hollow\" opacity=\"0\"></g>",
            node.issue_number
        )
        .expect("writing to String cannot fail");
        return;
    };
    let style = star_style(status);
    let opacity = if node.context {
        format!(" opacity=\"{CONTEXT_OPACITY}\" data-prominence=\"context\"")
    } else {
        String::new()
    };
    let completion = if status == Status::Resolved {
        "solid"
    } else {
        "hollow"
    };
    write!(
        svg,
        "<g data-issue=\"{}\" data-status=\"{}\" data-completion=\"{completion}\"{opacity}><circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" opacity=\"0.20\"/>",
        node.issue_number,
        status_name(status),
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
