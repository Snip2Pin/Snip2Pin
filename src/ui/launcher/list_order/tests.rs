use super::*;

#[test]
fn window_focus_loss_cancels_the_active_list_drag() {
    let ctx = egui::Context::default();
    egui::DragAndDrop::set_payload(&ctx, DraggedPin(1));
    let mut output = ctx.run_ui(
        egui::RawInput {
            focused: false,
            ..Default::default()
        },
        |ui| {
            let _ = indices(ui.ctx(), &[]);
        },
    );
    output.textures_delta.clear();
    assert!(!egui::DragAndDrop::has_payload_of_type::<DraggedPin>(&ctx));
}

#[test]
fn storage_order_and_new_or_removed_pins_preserve_manual_order() {
    let mut order = Order::default();
    assert_eq!(order.indices(&[3, 2, 1]), [0, 1, 2]);
    assert!(order.move_to(3, 1, true));
    assert_eq!(order.0, [2, 1, 3]);
    assert_eq!(order.indices(&[1, 3, 2]), [2, 0, 1]);
    assert_eq!(order.0, [2, 1, 3]);
    order.indices(&[5, 4, 3, 2, 1]);
    assert_eq!(order.0, [5, 4, 2, 1, 3]);
    order.indices(&[5, 4, 3, 1]);
    assert_eq!(order.0, [5, 4, 1, 3]);
    assert!(order.move_to(3, 5, false));
    assert_eq!(order.0, [3, 5, 4, 1]);
    assert!(!order.move_to(3, 3, false));
    assert!(!order.move_to(8, 3, true));
    assert!(!order.move_to(3, 8, true));
    assert_eq!(order.0, [3, 5, 4, 1]);
    assert!(order.indices(&[]).is_empty());
    assert!(order.0.is_empty());
}

#[test]
fn closing_a_pin_releases_its_order_and_active_drag() {
    let ctx = egui::Context::default();
    ctx.data_mut(|data| {
        data.insert_temp(state_id(), Order(vec![3, 2, 1]));
    });
    egui::DragAndDrop::set_payload(&ctx, DraggedPin(2));
    forget(&ctx, 1);
    assert!(egui::DragAndDrop::has_payload_of_type::<DraggedPin>(&ctx));
    forget(&ctx, 2);
    assert!(!egui::DragAndDrop::has_payload_of_type::<DraggedPin>(&ctx));
    assert_eq!(
        ctx.data(|data| data.get_temp::<Order>(state_id()))
            .expect("remaining order")
            .0,
        [3]
    );
    forget(&ctx, 3);
    assert!(
        ctx.data(|data| data.get_temp::<Order>(state_id()))
            .is_none()
    );
}

#[test]
fn dragging_to_the_list_edge_reaches_and_reorders_the_last_of_sixty_rows() {
    let ctx = egui::Context::default();
    let texture = ctx.load_texture(
        "generated-long-order",
        egui::ColorImage::filled([8, 8], egui::Color32::WHITE),
        Default::default(),
    );
    let pins = (1..=60)
        .map(|id| PinEntry {
            id,
            name: "",
            texture: &texture,
            size: [8, 8],
            display_size: None,
            locked: false,
            selected: false,
        })
        .collect::<Vec<_>>();
    let frame = |events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(390.0, 300.0))),
                events,
                ..Default::default()
            },
            |ui| {
                assert!(super::super::pin_list_rows(ui, &pins, Locale::English, 224.0).is_none());
            },
        );
        output.textures_delta.clear();
        output
    };
    let wells = |output: &egui::FullOutput| {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.rect.size() == egui::Vec2::from(LAUNCHER.pin_thumbnail) =>
                {
                    Some((rect.rect, shape.clip_rect))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let _ = frame(vec![]);
    let initial = frame(vec![]);
    let (first, clip) = wells(&initial)[0];
    let from = Pos2::new(
        first.left() - LAUNCHER.pin_row_padding[0] / 2.0,
        first.center().y,
    );
    let to = Pos2::new(from.x, clip.bottom() - 4.0);
    let pointer = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame(vec![egui::Event::PointerMoved(from)]);
    let _ = frame(vec![pointer(from, true)]);
    let _ = frame(vec![egui::Event::PointerMoved(
        from + egui::vec2(0.0, 12.0),
    )]);
    let mut output = frame(vec![egui::Event::PointerMoved(to)]);
    let mut reached = false;
    for _ in 0..600 {
        if wells(&output)
            .last()
            .is_some_and(|(well, clip)| clip.contains_rect(*well))
        {
            reached = true;
            break;
        }
        output = frame(vec![]);
    }
    assert!(
        reached,
        "edge dragging must reveal the last row without dropping the payload"
    );
    let _ = frame(vec![pointer(to, false)]);
    let _ = frame(vec![]);
    let order = ctx
        .data(|data| data.get_temp::<Order>(state_id()))
        .expect("list order");
    assert_eq!(order.0.first(), Some(&2));
    assert_eq!(order.0.last(), Some(&1));
    assert!(!egui::DragAndDrop::has_any_payload(&ctx));
}
