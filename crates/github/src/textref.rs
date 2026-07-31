#[derive(Debug, Default, PartialEq)]
pub struct TextRefs {
    pub blocked_by: Vec<u64>,
    pub blocks: Vec<u64>,
}

#[derive(Clone, Copy)]
struct Fence {
    marker: u8,
    length: usize,
}

fn fence_at_start(line: &str) -> Option<Fence> {
    let marker = *line.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }

    let length = line.bytes().take_while(|byte| *byte == marker).count();
    (length >= 3).then_some(Fence { marker, length })
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

    for raw in body.lines() {
        let trimmed = raw.trim_start();
        match (open_fence, fence_at_start(trimmed)) {
            (None, Some(fence)) => {
                open_fence = Some(fence);
                continue;
            }
            (Some(open), Some(close))
                if close.marker == open.marker && close.length >= open.length =>
            {
                open_fence = None;
                continue;
            }
            (Some(_), _) => continue,
            (None, None) => {}
        }

        let stripped = strip_leading_markers(trimmed);
        let lower = stripped.to_ascii_lowercase();
        let bucket = if lower.starts_with("blocked by") {
            &mut refs.blocked_by
        } else if lower.starts_with("blocks") {
            &mut refs.blocks
        } else {
            continue;
        };

        for (index, character) in stripped.char_indices() {
            if character == '#' {
                let digits: String = stripped[index + 1..]
                    .chars()
                    .take_while(|character| character.is_ascii_digit())
                    .collect();
                if let Ok(number) = digits.parse::<u64>() {
                    bucket.push(number);
                }
            }
        }
    }

    refs.blocked_by.sort_unstable();
    refs.blocked_by.dedup();
    refs.blocks.sort_unstable();
    refs.blocks.dedup();
    refs
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
}
