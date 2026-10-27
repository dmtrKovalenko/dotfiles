//! Prefix-free hint labels: as many single keys as possible, then two-key
//! labels built on the least preferred keys, and so on.

/// `count` distinct labels over `keys`, shortest and best keys first. No
/// label is a prefix of another, so typing one never needs a confirmation.
pub fn generate(keys: &[char], count: usize) -> Vec<String> {
    if count == 0 || keys.is_empty() {
        return Vec::new();
    }
    let arity = keys.len().max(2);
    if keys.len() < 2 {
        return (1..=count).map(|n| keys[0].to_string().repeat(n)).collect();
    }
    let mut leaves: Vec<String> = keys.iter().map(char::to_string).collect();
    while leaves.len() < count {
        // Expand the last leaf among the shortest ones so the best keys keep
        // their single-key labels as long as possible.
        let shortest = leaves[0].len();
        let index = leaves
            .iter()
            .rposition(|leaf| leaf.len() == shortest)
            .expect("leaves is non-empty");
        let parent = leaves.remove(index);
        let needed = count - leaves.len();
        for key in keys.iter().take(needed.max(2).min(arity)) {
            leaves.push(format!("{parent}{key}"));
        }
    }
    leaves.sort_by_key(|leaf| (leaf.len(), rank(leaf, keys)));
    leaves.truncate(count);
    leaves
}

fn rank(label: &str, keys: &[char]) -> Vec<usize> {
    label
        .chars()
        .map(|c| keys.iter().position(|k| *k == c).unwrap_or(usize::MAX))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: &[char] = &['a', 's', 'd', 'f'];

    fn assert_prefix_free(labels: &[String]) {
        for (i, a) in labels.iter().enumerate() {
            for (j, b) in labels.iter().enumerate() {
                if i != j {
                    assert!(!b.starts_with(a.as_str()), "{a} is a prefix of {b}");
                }
            }
        }
    }

    #[test]
    fn few_targets_get_single_keys_in_preference_order() {
        assert_eq!(generate(KEYS, 3), vec!["a", "s", "d"]);
        assert_eq!(generate(KEYS, 4), vec!["a", "s", "d", "f"]);
        assert!(generate(KEYS, 0).is_empty());
    }

    #[test]
    fn one_more_target_than_keys_sacrifices_only_the_worst_key() {
        assert_eq!(generate(KEYS, 5), vec!["a", "s", "d", "fa", "fs"]);
    }

    #[test]
    fn labels_are_prefix_free_and_exactly_as_many_as_requested() {
        for count in [1, 4, 5, 7, 13, 16, 17, 64, 200] {
            let labels = generate(KEYS, count);
            assert_eq!(labels.len(), count, "count {count}");
            assert_prefix_free(&labels);
            let unique: std::collections::HashSet<&String> = labels.iter().collect();
            assert_eq!(unique.len(), count, "count {count}");
        }
    }

    #[test]
    fn labels_are_sorted_shortest_first_then_by_key_preference() {
        let labels = generate(KEYS, 7);
        assert_eq!(labels, vec!["a", "s", "d", "fa", "fs", "fd", "ff"]);
    }

    #[test]
    fn many_targets_never_exceed_three_keys_with_a_full_layout() {
        let keys: Vec<char> = "asdfwerzxvjkluopghtyb".chars().collect();
        let labels = generate(&keys, 400);
        assert!(labels.iter().all(|l| l.len() <= 3));
        assert_prefix_free(&labels);
    }
}
