mod comment_str_map;
pub mod comments;
pub mod lexer;
pub mod tokens;

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
