use super::*;

#[test]
fn input_and_output_leads_extend_to_the_viewport_edge() {
    let viewport = [-5.0, -5.0, 5.0, 5.0];
    let input = Component {
        id: ComponentId(0),
        position: Point::new(0, 0),
        rotation: Rotation::Up,
        kind: ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(0),
        },
    };
    let output = Component {
        id: ComponentId(1),
        position: Point::new(-10, 0),
        rotation: Rotation::Right,
        kind: ComponentKind::Output {
            scale: Scale::ONE,
            id: OutputId::from_u128(0),
        },
    };

    let input_lead = DrawTriangle::component_lead(&input, viewport);
    assert_eq!(input_lead.len(), 2);
    assert!(input_lead
        .iter()
        .all(|triangle| triangle.color == DrawTriangle::INPUT_COLOR));
    assert!(input_lead
        .iter()
        .flat_map(|triangle| triangle.positions)
        .any(|position| position[1] == viewport[1]));

    let output_lead = DrawTriangle::component_lead(&output, viewport);
    assert_eq!(output_lead.len(), 2);
    assert!(output_lead
        .iter()
        .all(|triangle| triangle.color == DrawTriangle::OUTPUT_COLOR));
    assert!(output_lead
        .iter()
        .flat_map(|triangle| triangle.positions)
        .any(|position| position[0] == viewport[2]));
}
