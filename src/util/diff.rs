#[derive(Debug, PartialEq, Eq)]
pub enum Diff<T> {
    Insert(usize, T), // Position, Token to insert
    Delete(usize),    // Position of deleted token
    Change(usize, T), // Position, Updated token
}

fn trace_vec_vec<T: std::fmt::Debug>(vec: &Vec<Vec<T>>) {
    for v in vec {
        tracing::warn!("{v:?}");
    }
}

type DpTable = Vec<Vec<(usize, bool)>>;

impl<T> Diff<T>
where
    T: std::fmt::Debug + PartialEq + Eq + Clone,
{
    fn get_dp_table(old: &impl AsRef<Vec<T>>, new: &impl AsRef<Vec<T>>) -> DpTable {
        let old_vec = old.as_ref();
        let new_vec = new.as_ref();
        // tracing::warn!("old: {old_vec:#?}\nnew: {new_vec:#?}");

        let m = old_vec.len();
        let n = new_vec.len();

        let mut dp: Vec<Vec<(usize, bool)>> = vec![vec![(0, false); n]; m];

        let get_cell_val = |matching: &bool, i: usize, k: usize, dp: &Vec<Vec<(usize, bool)>>| {
            if *matching {
                let left_top_cell_coords = i
                    .checked_sub(1)
                    .and_then(|v| k.checked_sub(1).and_then(|kv| Some((v, kv))));

                left_top_cell_coords
                    .and_then(|coords| {
                        dp.get(coords.0).and_then(|inner| {
                            inner.get(coords.1).and_then(|(count, _)| Some(*count))
                        })
                    })
                    .unwrap_or(0)
                    + 1
            } else {
                let left_cell_coords = k.checked_sub(1).and_then(|kv| Some((i, kv)));
                let top_cell_coords = i.checked_sub(1).and_then(|iv| Some((iv, k)));

                let left_val = left_cell_coords
                    .and_then(|coords| {
                        dp.get(coords.0)
                            .and_then(|inner| inner.get(coords.1).and_then(|v| Some(v.0)))
                    })
                    .unwrap_or(0);

                let top_val = top_cell_coords
                    .and_then(|coords| {
                        dp.get(coords.0)
                            .and_then(|inner| inner.get(coords.1).and_then(|v| Some(v.0)))
                    })
                    .unwrap_or(0);

                left_val.max(top_val)
            }
        };

        for (i, one) in (**old_vec).iter().enumerate() {
            for (k, new) in (**new_vec).iter().enumerate() {
                let matching = one == new;
                if matching {
                    // tracing::warn!("MATCH at i:{i} k:{k}\none {one:#?}\nnew {new:#?}");
                }
                let cell_val = get_cell_val(&matching, i, k, &dp);

                dp[i][k] = (cell_val, matching);
            }
        }
        dp
    }

    pub fn get_lcs(old: &impl AsRef<Vec<T>>, new: &impl AsRef<Vec<T>>) -> Vec<T> {
        let dp = Self::get_dp_table(old, new);

        let m = dp.len() - 1;
        let n = dp[0].len() - 1;
        let mut lcs = vec![];
        let mut expected_lcs_len = Option::<usize>::None;

        let (mut i, mut k) = (m, n);
        loop {
            let (count, is_match) = dp[i][k];
            if expected_lcs_len.is_none() {
                expected_lcs_len = Some(count);
            }

            if expected_lcs_len.is_some_and(|len| count != len - lcs.len()) {
                // tracing::warn!(
                //     "overshot, expecting to find {}, got {}",
                //     expected_lcs_len.unwrap() - lcs.len(),
                //     count
                // );
                if i == m {
                    i = 0;
                } else {
                    i += 1;
                }
                if k == n {
                    k = 0;
                } else {
                    k += 1;
                }
                continue;
            }

            if is_match {
                let v: T = old.as_ref()[i].to_owned();
                lcs.push(v);
                match i.checked_sub(1) {
                    Some(v) => {
                        i = v;
                        k -= 1;
                    }
                    None => break,
                };
            } else {
                match k.checked_sub(1) {
                    Some(v) => k = v,
                    None => {
                        match i.checked_sub(1) {
                            Some(v) => {
                                i = v;
                                k = n;
                            }
                            None => break,
                        };
                    }
                }
            }
        }

        lcs.reverse();

        assert_eq!(
            lcs.len(),
            expected_lcs_len.unwrap(),
            "lcs is not the expected length"
        );

        lcs
    }

    pub fn get_diffs(old: impl AsRef<Vec<T>>, new: impl AsRef<Vec<T>>) -> Vec<Self> {
        let lcs = Self::get_lcs(&old, &new);
        // tracing::warn!("lcs: {lcs:#?}");
        let mut diffs = vec![];
        let old_vec = old.as_ref();
        let new_vec = new.as_ref();

        let (mut i, mut k, mut lcs_idx) = (0, 0, 0);

        loop {
            let current_old = old_vec.get(i);
            let current_new = new_vec.get(k);
            let current_lcs = lcs.get(lcs_idx);

            match (current_old, current_new) {
                (Some(old), Some(new)) => {
                    if old == new {
                        if current_lcs == Some(old) {
                            i += 1;
                            k += 1;
                            lcs_idx += 1;
                            continue;
                        } else {
                            panic!("have not thought through this case");
                        }
                    } else {
                        // is old or new different from the lcs
                        // increment to idx of the different one, leave the others
                        // based on which one you're incrementing, either insert or delete

                        let new_is_different = current_lcs.is_some_and(|p| p != new);
                        let old_is_different = current_lcs.is_some_and(|p| p != old);

                        if new_is_different && old_is_different {
                            diffs.push(Diff::Change(i, new.to_owned()));
                            k += 1;
                            i += 1;
                        } else if new_is_different {
                            diffs.push(Diff::Insert(k, new.to_owned()));
                            k += 1;
                        } else if old_is_different {
                            diffs.push(Diff::Delete(i));
                            i += 1;
                        }
                    }
                }
                (None, Some(new)) => {
                    let dif = Diff::Insert(k, new.to_owned());
                    diffs.push(dif);
                    k += 1;
                }
                (Some(_old), None) => {
                    let dif = Diff::Delete(i);
                    diffs.push(dif);
                    i += 1;
                }
                _ => break,
            }
        }
        diffs
    }
}

mod tests {
    use crate::util::Diff;

    #[test]
    fn dp_table() {
        let string1: Vec<char> = "abxh".to_string().chars().into_iter().collect();
        let string2: Vec<char> = "abfhh".to_string().chars().into_iter().collect();
        let expected_table = vec![
            vec![(1, true), (1, false), (1, false), (1, false), (1, false)],
            vec![(1, false), (2, true), (2, false), (2, false), (2, false)],
            vec![(1, false), (2, false), (2, false), (2, false), (2, false)],
            vec![(1, false), (2, false), (2, false), (3, true), (3, true)],
        ];

        let table = Diff::get_dp_table(&string1, &string2);

        assert_eq!(expected_table, table);
    }

    #[test]
    fn lcs() {
        let string1: Vec<char> = "abxh".to_string().chars().into_iter().collect();
        let string2: Vec<char> = "abfhh".to_string().chars().into_iter().collect();

        let lcs = Diff::get_lcs(&string1, &string2)
            .into_iter()
            .collect::<String>();
        let expected = "abh".to_string();
        assert_eq!(lcs, expected);
    }
}
