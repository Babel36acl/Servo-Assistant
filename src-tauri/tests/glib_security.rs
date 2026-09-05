#![cfg(target_os = "linux")]

use glib::prelude::*;

#[test]
fn variant_string_iteration_is_sound_with_optimizations() {
    let variant = vec!["alpha", "", "中文", "omega"].to_variant();
    let expected = ["alpha", "", "中文", "omega"];
    assert_eq!(variant.array_iter_str().unwrap().collect::<Vec<_>>(), expected);
    assert_eq!(variant.array_iter_str().unwrap().rev().collect::<Vec<_>>(),
               expected.into_iter().rev().collect::<Vec<_>>());
    assert_eq!(variant.array_iter_str().unwrap().nth(2), Some("中文"));
    assert_eq!(variant.array_iter_str().unwrap().nth_back(1), Some("中文"));
    assert_eq!(variant.array_iter_str().unwrap().last(), Some("omega"));
    let mut iter = variant.array_iter_str().unwrap();
    assert_eq!(iter.next(), Some("alpha"));
    assert_eq!(iter.next_back(), Some("omega"));
    assert_eq!(iter.next(), Some(""));
    assert_eq!(iter.next_back(), Some("中文"));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
}
