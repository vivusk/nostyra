use super::*;

#[test]
fn suppression_ends_on_release_only() {
    let (a, b) = (1 << 8, 1);
    assert_eq!(unsuppress_released(a | b, a | b), a | b, "both still held");
    assert_eq!(unsuppress_released(a | b, a), a, "B released");
    assert_eq!(unsuppress_released(a, 0) & a, 0, "a re-press is not hidden");
}

#[test]
fn pads_fill_ports_zero_to_three_and_a_fifth_is_ignored() {
    let mut ports: [Option<u32>; PORT_COUNT] = Default::default();
    for pad in 0..5u32 {
        match first_free_port(&ports) {
            Some(slot) => ports[slot] = Some(pad),
            None => assert_eq!(pad, 4, "only the fifth pad finds no port"),
        }
    }
    assert_eq!(ports, [Some(0), Some(1), Some(2), Some(3)]);
    ports[1] = None; // pad 1 unplugged: the next pad takes its port
    assert_eq!(first_free_port(&ports), Some(1));
}
