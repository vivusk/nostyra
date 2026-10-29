use super::*;

#[test]
fn suppression_ends_on_release_only() {
    let (a, b) = (1 << 8, 1);
    assert_eq!(unsuppress_released(a | b, a | b), a | b, "both still held");
    assert_eq!(unsuppress_released(a | b, a), a, "B released");
    assert_eq!(unsuppress_released(a, 0) & a, 0, "a re-press is not hidden");
}
