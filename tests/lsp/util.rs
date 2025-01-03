use std::sync::LazyLock;

use espx_lsp_server::util::Diff;

use crate::helpers::TEST_TRACING;

#[test]
fn diff_str() {
    LazyLock::force(&TEST_TRACING);

    let string1: Vec<char> = "abxh".to_string().chars().into_iter().collect();
    let string2: Vec<char> = "abfhh".to_string().chars().into_iter().collect();

    let diff = Diff::get_diffs(string1, string2);

    let expected = vec![Diff::Change(2, 'f'), Diff::Insert(4, 'h')];

    assert_eq!(diff, expected);

    let string1: Vec<char> = "abcdfghjqz".to_string().chars().into_iter().collect();
    let string2: Vec<char> = "abcdefgijkrxyz".to_string().chars().into_iter().collect();
    let diff = Diff::<char>::get_diffs(string1, string2);
    // abcdfgjz
    let expected = vec![
        Diff::Insert(4, 'e'),
        Diff::Change(6, 'i'),
        Diff::Change(8, 'k'),
        Diff::Insert(10, 'r'),
        Diff::Insert(11, 'x'),
        Diff::Insert(12, 'y'),
    ];

    assert_eq!(diff, expected);
}
