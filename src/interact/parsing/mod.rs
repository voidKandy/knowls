mod comment_str_map;
pub mod comments;
pub mod lexer;
pub mod tokens;
pub use comment_str_map::language_ext_from_uri;

/// Returns Ordering::Equal if the position is within the range, otherwise denotes which direction
/// it is out of range
pub fn cmp_pos_range(range: &lsp_types::Range, pos: &lsp_types::Position) -> std::cmp::Ordering {
    if pos.line < range.start.line
        || pos.character < range.start.character && pos.line == range.start.line
    {
        return std::cmp::Ordering::Less;
    }

    if pos.line > range.end.line
        || pos.character > range.end.character && pos.line == range.end.line
    {
        return std::cmp::Ordering::Greater;
    }

    std::cmp::Ordering::Equal
}

pub fn ranges_overlap(range: &lsp_types::Range, other: &lsp_types::Range) -> bool {
    let start1 = &range.start;
    let end1 = &range.end;
    let start2 = &other.start;
    let end2 = &other.end;

    (start1.line < end2.line)
        || (start1.line == end2.line && start1.character < end2.character)
            && (start2.line < end1.line
                || (start2.line == end1.line && start2.character < end1.character))
}
