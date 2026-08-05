use std::collections::HashMap;

use stellr_core::RawIssue;

#[derive(Debug, Default, PartialEq)]
pub struct TextRefs {
    pub blocked_by: Vec<u64>,
    pub blocks: Vec<u64>,
    pub parents: Vec<u64>,
}

#[derive(Clone, Copy)]
struct Fence {
    marker: u8,
    length: usize,
}

#[derive(Clone, Copy)]
enum RelationshipSection {
    BlockedBy,
    Blocks,
    Parent,
}

fn atx_heading(line: &str) -> Option<&str> {
    let indentation = line.bytes().take_while(|byte| *byte == b' ').count();
    if indentation > 3 {
        return None;
    }

    let line = &line[indentation..];
    let level = line.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    if line
        .as_bytes()
        .get(level)
        .is_some_and(|byte| !byte.is_ascii_whitespace())
    {
        return None;
    }

    let title = line[level..].trim();
    let without_markers = title.trim_end_matches('#');
    if without_markers.len() != title.len()
        && without_markers
            .chars()
            .last()
            .is_some_and(char::is_whitespace)
    {
        Some(without_markers.trim_end())
    } else {
        Some(title)
    }
}

fn extract_issue_references(line: &str, bucket: &mut Vec<u64>) {
    for (index, character) in line.char_indices() {
        if character == '#' {
            let left_is_boundary = line[..index].chars().next_back().is_none_or(|character| {
                !character.is_alphanumeric() && !matches!(character, '_' | '-' | '.' | '/' | '\\')
            });
            if !left_is_boundary {
                continue;
            }

            let suffix = &line[index + 1..];
            let digit_count = suffix.bytes().take_while(u8::is_ascii_digit).count();
            if digit_count == 0
                || suffix[digit_count..]
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_alphanumeric() || character == '_')
            {
                continue;
            }

            let digits = &suffix[..digit_count];
            if let Ok(number) = digits.parse::<u64>() {
                bucket.push(number);
            }
        }
    }
}

fn fence_at_start(line: &str, closing: bool) -> Option<Fence> {
    let indentation = line.bytes().take_while(|byte| *byte == b' ').count();
    if indentation > 3 {
        return None;
    }

    let line = &line[indentation..];
    let marker = *line.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }

    let length = line.bytes().take_while(|byte| *byte == marker).count();
    if length < 3 {
        return None;
    }

    let suffix = &line[length..];
    if closing {
        suffix
            .bytes()
            .all(|byte| byte == b' ' || byte == b'\t')
            .then_some(Fence { marker, length })
    } else if marker == b'`' && suffix.contains('`') {
        None
    } else {
        Some(Fence { marker, length })
    }
}

fn strip_container_prefixes(mut line: &str) -> &str {
    loop {
        let indentation = line.bytes().take_while(|byte| *byte == b' ').count();
        if indentation > 3 {
            return line;
        }

        let content = &line[indentation..];
        let Some(marker) = content.as_bytes().first().copied() else {
            return line;
        };
        if marker != b'-' && marker != b'*' && marker != b'>' {
            return line;
        }

        let rest = &content[1..];
        if marker != b'>' && !rest.is_empty() && !rest.as_bytes()[0].is_ascii_whitespace() {
            return line;
        }

        line = rest.strip_prefix([' ', '\t']).unwrap_or(rest);
    }
}

fn strip_leading_markers(mut line: &str) -> &str {
    line = line.trim_start();
    loop {
        let Some(marker) = line.as_bytes().first().copied() else {
            return line;
        };
        if marker != b'-' && marker != b'*' && marker != b'>' {
            return line;
        }

        let rest = &line[1..];
        if !rest.is_empty() && !rest.as_bytes()[0].is_ascii_whitespace() {
            return line;
        }
        line = rest.trim_start();
    }
}

pub fn scan(body: &str) -> TextRefs {
    let mut refs = TextRefs::default();
    let mut open_fence: Option<Fence> = None;
    let mut section: Option<RelationshipSection> = None;

    for raw in body.lines() {
        let fence_line = strip_container_prefixes(raw);
        let trimmed = raw.trim_start();
        match open_fence {
            None if let Some(fence) = fence_at_start(fence_line, false) => {
                open_fence = Some(fence);
                continue;
            }
            Some(open) => {
                if fence_at_start(fence_line, true)
                    .is_some_and(|close| close.marker == open.marker && close.length >= open.length)
                {
                    open_fence = None;
                }
                continue;
            }
            None => {}
        }

        if let Some(title) = atx_heading(fence_line) {
            section = match title.to_ascii_lowercase().as_str() {
                "blocked by" => Some(RelationshipSection::BlockedBy),
                "blocks" => Some(RelationshipSection::Blocks),
                "parent" => Some(RelationshipSection::Parent),
                _ => None,
            };
            continue;
        }

        let stripped = strip_leading_markers(trimmed);
        let lower = stripped.to_ascii_lowercase();
        let bucket = if lower.starts_with("blocked by") {
            Some(&mut refs.blocked_by)
        } else if lower.starts_with("blocks") {
            Some(&mut refs.blocks)
        } else {
            match section {
                Some(RelationshipSection::BlockedBy) => Some(&mut refs.blocked_by),
                Some(RelationshipSection::Blocks) => Some(&mut refs.blocks),
                Some(RelationshipSection::Parent) => Some(&mut refs.parents),
                None => None,
            }
        };

        if let Some(bucket) = bucket {
            extract_issue_references(stripped, bucket);
        }
    }

    refs.blocked_by.sort_unstable();
    refs.blocked_by.dedup();
    refs.blocks.sort_unstable();
    refs.blocks.dedup();
    refs.parents.sort_unstable();
    refs.parents.dedup();
    refs
}

pub(crate) fn enrich_relationships(issues: &mut [RawIssue]) {
    let mut inversions = Vec::new();

    for issue in issues.iter_mut() {
        let refs = scan(&issue.body);
        issue.blocked_by.extend(refs.blocked_by);
        inversions.extend(refs.blocks.into_iter().map(|target| (issue.number, target)));
        if issue.parent_issue.is_none() && refs.parents.len() == 1 {
            issue.parent_issue = Some(refs.parents[0]);
        }
    }

    let positions = issues
        .iter()
        .enumerate()
        .map(|(index, issue)| (issue.number, index))
        .collect::<HashMap<_, _>>();
    for (blocker, target) in inversions {
        if let Some(&index) = positions.get(&target) {
            issues[index].blocked_by.push(blocker);
        }
    }
    for issue in issues {
        issue.blocked_by.sort_unstable();
        issue.blocked_by.dedup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_blocked_by_and_blocks_lines() {
        let body = "Intro\n- Blocked by #12, #7\nBlocks #3\nblocked by #12\n";
        let refs = scan(body);
        assert_eq!(refs.blocked_by, vec![7, 12]);
        assert_eq!(refs.blocks, vec![3]);
    }

    #[test]
    fn ignores_fenced_code_blocks() {
        let body = "```\nBlocked by #99\n```\nBlocked by #4\n~~~\nBlocks #98\n~~~\n";
        let refs = scan(body);
        assert_eq!(refs.blocked_by, vec![4]);
        assert_eq!(refs.blocks, Vec::<u64>::new());
    }

    #[test]
    fn only_same_kind_fence_closes_code_block() {
        let body = "```\nBlocked by #99\n~~~\nBlocked by #98\n```\nBlocked by #4\n";
        let refs = scan(body);
        assert_eq!(refs.blocked_by, vec![4]);
    }

    #[test]
    fn closing_fence_must_be_at_least_as_long_as_opener() {
        let body = "````\nBlocked by #99\n```\nBlocked by #98\n````\nBlocked by #4\n";
        let refs = scan(body);
        assert_eq!(refs.blocked_by, vec![4]);
    }

    #[test]
    fn ignores_fences_after_quote_and_list_prefixes() {
        let body = "> ```\n> Blocked by #99\n> ```\n- ~~~~\n- Blocks #98\n- ~~~~\nBlocked by #4\nBlocks #3\n";
        let refs = scan(body);
        assert_eq!(refs.blocked_by, vec![4]);
        assert_eq!(refs.blocks, vec![3]);
    }

    #[test]
    fn ignores_fences_after_a_block_quote_marker_without_a_following_space() {
        let body = ">```\n> Blocked by #99\n>```\nBlocked by #4\n";
        let refs = scan(body);
        assert_eq!(refs.blocked_by, vec![4]);
    }

    #[test]
    fn does_not_treat_four_space_indented_fence_as_an_opening_fence() {
        let refs = scan("    ```\nBlocked by #99\n");
        assert_eq!(refs.blocked_by, vec![99]);
    }

    #[test]
    fn does_not_treat_backtick_info_with_backticks_as_an_opening_fence() {
        let refs = scan("``` rust`example\nBlocked by #99\n");
        assert_eq!(refs.blocked_by, vec![99]);
    }

    #[test]
    fn does_not_close_a_fence_when_non_whitespace_follows_the_marker_run() {
        let body = "```\nBlocked by #99\n```not-a-close\nBlocked by #98\n```\nBlocked by #4\n";
        let refs = scan(body);
        assert_eq!(refs.blocked_by, vec![4]);
    }

    #[test]
    fn does_not_close_a_fence_when_an_extra_carriage_return_precedes_crlf() {
        let body = "```\nBlocked by #99\n```\r\r\nBlocked by #98\n```\nBlocked by #4\n";
        let refs = scan(body);
        assert_eq!(refs.blocked_by, vec![4]);
    }

    #[test]
    fn strips_nested_markers_without_accepting_punctuation_runs() {
        let body = "> - Blocked by #9\n- > Blocks #3\n> > Blocked by #7\n---Blocked by #99\n*** Blocks #98\n";
        let refs = scan(body);
        assert_eq!(refs.blocked_by, vec![7, 9]);
        assert_eq!(refs.blocks, vec![3]);
    }

    #[test]
    fn mid_sentence_mentions_do_not_count() {
        let refs = scan("This is blocked by #5 in spirit.\n");
        assert_eq!(refs.blocked_by, Vec::<u64>::new());
    }

    #[test]
    fn scans_dependency_references_beneath_markdown_headings() {
        let refs = scan("   ## Blocked by\n\n- #17\n- #19\n## Blocks ##\n* #23\n");

        assert_eq!(refs.blocked_by, vec![17, 19]);
        assert_eq!(refs.blocks, vec![23]);
    }

    #[test]
    fn relationship_section_ends_at_the_next_heading() {
        let refs = scan("## Blocked by\n- #17\n## Acceptance criteria\n- #99\n");

        assert_eq!(refs.blocked_by, vec![17]);
    }

    #[test]
    fn four_space_indented_pseudo_heading_does_not_end_relationship_section() {
        let refs = scan("## Blocked by\n- #17\n    ## Acceptance criteria\n- #99\n");

        assert_eq!(refs.blocked_by, vec![17, 99]);
    }

    #[test]
    fn marker_only_atx_heading_ends_relationship_section() {
        let refs = scan("## Blocked by\n- #17\n##\n- #99\n");

        assert_eq!(refs.blocked_by, vec![17]);
    }

    #[test]
    fn relationship_sections_only_accept_standalone_same_repository_references() {
        let refs = scan(
            "## Blocked by\n- other/repo#17\n- #18oops\n- (#19), #20.\n## Parent\n- other/repo#21\n- #22oops\n",
        );

        assert_eq!(refs.blocked_by, vec![19, 20]);
        assert_eq!(refs.parents, Vec::<u64>::new());
    }

    #[test]
    fn section_references_still_ignore_fenced_examples_and_deduplicate_inline_refs() {
        let refs = scan("Blocked by #17\n## Blocked by\n- #17\n```\n- #99\n```\n- #19\n");

        assert_eq!(refs.blocked_by, vec![17, 19]);
    }

    #[test]
    fn scans_dependency_references_beneath_container_prefixed_headings() {
        let refs = scan(">    ## Blocked by\n> - #17\n- ## Blocks\n- - #23\n");

        assert_eq!(refs.blocked_by, vec![17]);
        assert_eq!(refs.blocks, vec![23]);
    }
}
