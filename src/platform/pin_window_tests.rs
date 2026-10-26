#[test]
fn current_focus_uses_process_ownership_including_child_windows() {
    for (name, focus, nodes, expected) in [
        ("own menu", 10, vec![(10, Some(42), 0)], Some(true)),
        (
            "menu child with PID on ancestor",
            20,
            vec![(20, None, 10), (10, Some(42), 0)],
            Some(true),
        ),
        (
            "external app child",
            20,
            vec![(20, None, 30), (30, Some(99), 0)],
            Some(false),
        ),
        ("missing PID", 20, vec![(20, None, 2), (2, None, 0)], None),
        ("query failure", 20, vec![], None),
        ("no focus", 0, vec![], None),
        ("pointer root", 1, vec![], None),
    ] {
        let mut inspected = Vec::new();
        let actual = super::focus_owned_by(focus, 42, |window| {
            inspected.push(window);
            nodes
                .iter()
                .find(|node| node.0 == window)
                .map(|node| (node.1, node.2))
        });
        assert_eq!(actual, expected, "{name}");
        if focus <= 1 {
            assert!(inspected.is_empty(), "special focus values have no window");
        }
    }
}
