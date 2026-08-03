//! Release-constellation artifacts for Stellr's README.

mod github_source;
mod preview;
mod story;

pub use github_source::{
    GithubReleaseHistorySource, LiveReleaseRequest, ReleaseHistoryError, ReleaseHistorySource,
    ReleaseWindowStart,
};
pub use preview::{PreviewRenderError, StaticPreview, render_static_preview};
pub use story::{
    ClosureReason, IssueSnapshot, IssueStatus, LifecycleEvent, LifecycleEventKind,
    MilestoneIdentity, NodeCoordinate, NormalizedLifecycleEvent, PreviousRelease, RecordedIssue,
    ReleaseBoundaries, ReleaseEvidence, ReleaseStory, SnapshotState, StartingSnapshot, StoryBeat,
    StoryBoundaries, StoryBuildError, StoryEdge, StoryEvidence, UtcTimestamp, UtcTimestampError,
};

use roxmltree::Document;
use thiserror::Error;

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
pub(crate) const MAX_SVG_BYTES: usize = 750 * 1024;

/// A reason an SVG cannot be published as a Stellr README artifact.
#[derive(Debug, Error)]
pub enum SvgSafetyError {
    /// The input is too large to be a release-showcase SVG.
    #[error("SVG is {actual} bytes; the limit is {limit} bytes")]
    TooLarge { actual: usize, limit: usize },
    /// The input is not well-formed XML.
    #[error("SVG XML is invalid: {0}")]
    InvalidXml(#[from] roxmltree::Error),
    /// The input uses markup or styling outside the safe publication subset.
    #[error("SVG contains forbidden {kind}: {detail}")]
    Forbidden { kind: &'static str, detail: String },
}

/// Validates the script-free, self-contained subset accepted for README SVGs.
///
/// Local fragment references such as `url(#paint)` and `href="#node"` are
/// allowed. Active elements, event handlers, external resources, CSS imports,
/// escaped CSS, and transform animation fail closed.
pub fn validate_svg_safety(svg: &str) -> Result<(), SvgSafetyError> {
    if svg.len() > MAX_SVG_BYTES {
        return Err(SvgSafetyError::TooLarge {
            actual: svg.len(),
            limit: MAX_SVG_BYTES,
        });
    }

    let lowercase = svg.to_ascii_lowercase();
    if lowercase.contains("<!doctype") {
        return forbidden("document type", "DOCTYPE declarations are not allowed");
    }

    let document = Document::parse(svg)?;
    let root = document.root_element();
    if !root.tag_name().name().eq_ignore_ascii_case("svg")
        || root.tag_name().namespace() != Some(SVG_NAMESPACE)
    {
        return forbidden("root element", "expected an SVG-namespace <svg> element");
    }

    for element in document.descendants().filter(roxmltree::Node::is_element) {
        let tag = element.tag_name();
        if tag.namespace() != Some(SVG_NAMESPACE) {
            return forbidden("element namespace", tag.namespace().unwrap_or("none"));
        }

        let local_name = tag.name().to_ascii_lowercase();
        if matches!(
            local_name.as_str(),
            "script"
                | "foreignobject"
                | "animate"
                | "animatetransform"
                | "animatemotion"
                | "set"
                | "iframe"
                | "object"
                | "embed"
                | "audio"
                | "video"
                | "canvas"
        ) {
            return forbidden("element", tag.name());
        }

        for attribute in element.attributes() {
            let name = attribute.name().to_ascii_lowercase();
            let value = attribute.value().trim();
            let value_lowercase = value.to_ascii_lowercase();

            if name.starts_with("on") {
                return forbidden("event handler", attribute.name());
            }
            if name == "base" {
                return forbidden("base URI", attribute.name());
            }
            if value_lowercase.contains("javascript:") {
                return forbidden("JavaScript URL", attribute.value());
            }
            if matches!(name.as_str(), "href" | "src") && !is_local_fragment_reference(value) {
                return forbidden("external resource", attribute.value());
            }
            if name == "style"
                || value_lowercase.contains("url")
                || value_lowercase.contains("@import")
            {
                validate_css(value)?;
            }
        }

        if local_name == "style" {
            validate_css(element.text().unwrap_or_default())?;
        }
    }

    Ok(())
}

fn is_local_fragment_reference(value: &str) -> bool {
    value
        .strip_prefix('#')
        .is_some_and(|fragment| !fragment.is_empty() && !fragment.chars().any(char::is_whitespace))
}

fn validate_css(css: &str) -> Result<(), SvgSafetyError> {
    let lowercase = css.to_ascii_lowercase();
    for forbidden_token in [
        "@import",
        "@font-face",
        "@-webkit-keyframes",
        "@-moz-keyframes",
        "@-o-keyframes",
        "expression(",
        "javascript:",
        "/*",
        "\\",
    ] {
        if lowercase.contains(forbidden_token) {
            return forbidden("CSS token", forbidden_token);
        }
    }
    validate_keyframe_properties(&lowercase)?;

    let mut search = lowercase.as_str();
    while let Some(position) = search.find("url") {
        let preceding_is_identifier = position > 0
            && search[..position]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_ascii_alphanumeric() || character == '-');
        let after_name = &search[position + 3..];
        let after_whitespace = after_name.trim_start();
        if preceding_is_identifier || !after_whitespace.starts_with('(') {
            search = after_name;
            continue;
        }

        let arguments = &after_whitespace[1..];
        let closing = arguments
            .find(')')
            .ok_or_else(|| SvgSafetyError::Forbidden {
                kind: "CSS URL",
                detail: "missing closing parenthesis".to_owned(),
            })?;
        let raw_target = arguments[..closing].trim();
        let target = raw_target
            .strip_prefix(['\'', '"'])
            .and_then(|unquoted| unquoted.strip_suffix(['\'', '"']))
            .unwrap_or(raw_target)
            .trim();
        if !is_local_fragment_reference(target) {
            return forbidden("external CSS URL", raw_target);
        }

        search = &arguments[closing + 1..];
    }

    Ok(())
}

fn validate_keyframe_properties(css: &str) -> Result<(), SvgSafetyError> {
    let mut cursor = 0;
    while let Some(relative_start) = css[cursor..].find("@keyframes") {
        let keyframes_start = cursor + relative_start;
        let after_keyword = keyframes_start + "@keyframes".len();
        let opening = css[after_keyword..]
            .find('{')
            .map(|relative| after_keyword + relative)
            .ok_or_else(|| SvgSafetyError::Forbidden {
                kind: "CSS keyframes",
                detail: "missing opening brace".to_owned(),
            })?;
        let closing = matching_brace(css, opening).ok_or_else(|| SvgSafetyError::Forbidden {
            kind: "CSS keyframes",
            detail: "missing closing brace".to_owned(),
        })?;
        validate_keyframe_body(&css[opening + 1..closing])?;
        cursor = closing + 1;
    }
    Ok(())
}

fn matching_brace(css: &str, opening: usize) -> Option<usize> {
    let mut depth = 0_u32;
    for (relative, character) in css[opening..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(opening + relative);
                }
            }
            _ => {}
        }
    }
    None
}

fn validate_keyframe_body(body: &str) -> Result<(), SvgSafetyError> {
    let mut declaration_start = None;
    for (index, character) in body.char_indices() {
        match (character, declaration_start) {
            ('{', None) => declaration_start = Some(index + 1),
            ('{', Some(_)) => return forbidden("CSS keyframes", "nested declaration block"),
            ('}', Some(start)) => {
                validate_keyframe_declarations(&body[start..index])?;
                declaration_start = None;
            }
            ('}', None) => return forbidden("CSS keyframes", "unexpected closing brace"),
            _ => {}
        }
    }
    if declaration_start.is_some() {
        return forbidden("CSS keyframes", "unterminated declaration block");
    }
    Ok(())
}

fn validate_keyframe_declarations(declarations: &str) -> Result<(), SvgSafetyError> {
    for declaration in declarations
        .split(';')
        .map(str::trim)
        .filter(|declaration| !declaration.is_empty())
    {
        let (property, _) =
            declaration
                .split_once(':')
                .ok_or_else(|| SvgSafetyError::Forbidden {
                    kind: "CSS keyframes",
                    detail: format!("invalid declaration '{declaration}'"),
                })?;
        let property = property.trim();
        if !matches!(
            property,
            "opacity" | "fill" | "stroke" | "stroke-dasharray" | "stroke-dashoffset"
        ) {
            return forbidden("CSS animation property", property);
        }
    }
    Ok(())
}

fn forbidden<T>(kind: &'static str, detail: impl Into<String>) -> Result<T, SvgSafetyError> {
    Err(SvgSafetyError::Forbidden {
        kind,
        detail: detail.into(),
    })
}
