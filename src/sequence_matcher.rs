//! A minimal port of Python's `difflib.SequenceMatcher(None, a, b).ratio()`.
//!
//! autojump's fuzzy matching relies on this exact similarity score (threshold
//! 0.6), so we reproduce the Ratcliff/Obershelp algorithm as difflib
//! implements it: recursively take the longest common block (earliest in `a`,
//! then earliest in `b` on ties) and recurse on both sides.
//!
//! difflib's "autojunk" heuristic only kicks in for `b` of 200+ elements, which
//! a single path component never realistically reaches, so it is not modelled.

/// Similarity in `[0.0, 1.0]`: `2 * matches / (len(a) + len(b))`.
pub fn ratio(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let total = a.len() + b.len();
    if total == 0 {
        return 1.0;
    }
    2.0 * matching_chars(&a, &b) as f64 / total as f64
}

fn matching_chars(a: &[char], b: &[char]) -> usize {
    let mut matched = 0;
    let mut queue = vec![(0, a.len(), 0, b.len())];
    while let Some((alo, ahi, blo, bhi)) = queue.pop() {
        let (i, j, k) = find_longest_match(a, b, alo, ahi, blo, bhi);
        if k == 0 {
            continue;
        }
        matched += k;
        if alo < i && blo < j {
            queue.push((alo, i, blo, j));
        }
        if i + k < ahi && j + k < bhi {
            queue.push((i + k, ahi, j + k, bhi));
        }
    }
    matched
}

/// Longest common block of `a[alo..ahi]` and `b[blo..bhi]` as `(i, j, len)`.
fn find_longest_match(
    a: &[char],
    b: &[char],
    alo: usize,
    ahi: usize,
    blo: usize,
    bhi: usize,
) -> (usize, usize, usize) {
    let (mut besti, mut bestj, mut bestk) = (alo, blo, 0);
    // prev[j - blo + 1] = length of the common suffix ending at a[i - 1], b[j]
    let width = bhi - blo + 1;
    let mut prev = vec![0usize; width];
    let mut cur = vec![0usize; width];
    for (i, ca) in a.iter().enumerate().take(ahi).skip(alo) {
        for (j, cb) in b.iter().enumerate().take(bhi).skip(blo) {
            let idx = j - blo + 1;
            cur[idx] = if ca == cb { prev[idx - 1] + 1 } else { 0 };
            let k = cur[idx];
            if k > bestk {
                besti = i + 1 - k;
                bestj = j + 1 - k;
                bestk = k;
            }
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    (besti, bestj, bestk)
}

#[cfg(test)]
mod tests {
    use super::ratio;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn matches_python_difflib() {
        // Expected values computed with CPython's difflib.SequenceMatcher.
        assert!(close(ratio("", ""), 1.0));
        assert!(close(ratio("abc", ""), 0.0));
        assert!(close(ratio("abcd", "bcde"), 0.75));
        assert!(close(ratio("foo", "foobar"), 2.0 * 3.0 / 9.0));
        assert!(close(ratio("qabxcd", "abycdf"), 2.0 * 4.0 / 12.0));
        assert!(close(ratio("dcumnts", "documents"), 2.0 * 7.0 / 16.0));
        assert!(close(ratio("中文目录", "中文"), 2.0 * 2.0 / 6.0));
    }
}
