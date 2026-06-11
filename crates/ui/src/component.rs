use crate::text::TextEngine;
use crate::util::{Axis, Color, Rect, Size, SizeRecommendation, SizeSource, Sizing};
use crate::window::Scene;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Component {
    kind: Kind,
    rect: Rect,
}

#[derive(Clone)]
enum Kind {
    Sized(SizedComponent),
    Fill(Fill),
    Text(Text),
    Outline(Outline),
    Button(Button),
    List(List),
    Scrollable(Scrollable),
}

impl std::fmt::Debug for Kind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sized(value) => formatter.debug_tuple("Sized").field(value).finish(),
            Self::Fill(value) => formatter.debug_tuple("Fill").field(value).finish(),
            Self::Text(value) => formatter.debug_tuple("Text").field(value).finish(),
            Self::Outline(value) => formatter.debug_tuple("Outline").field(value).finish(),
            Self::Button(value) => formatter.debug_tuple("Button").field(value).finish(),
            Self::List(value) => formatter.debug_tuple("List").field(value).finish(),
            Self::Scrollable(value) => formatter.debug_tuple("Scrollable").field(value).finish(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SizedComponent {
    x: SizeSource,
    y: SizeSource,
    child: Option<Box<Component>>,
}

#[derive(Clone, Debug)]
pub struct Fill {
    color: Color,
    child: Box<Component>,
}

#[derive(Clone, Debug)]
pub struct Outline {
    color: Color,
    gap: f32,
    width: f32,
    child: Box<Component>,
}

#[derive(Clone, Debug)]
pub struct Text {
    value: String,
}

#[derive(Clone)]
pub struct Button {
    on_state_change: Arc<dyn Fn(&mut Component, ButtonState)>,
    on_activate: Arc<dyn Fn()>,
    state: ButtonState,
    child: Box<Component>,
}

impl std::fmt::Debug for Button {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Button")
            .field("state", &self.state)
            .field("child", &self.child)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    pub focused: bool,
    pub pressed: bool,
}

#[derive(Clone, Debug)]
pub struct List {
    axis: Axis,
    children: Vec<ListChild>,
}

#[derive(Clone, Debug)]
struct ListChild {
    sizing: Sizing,
    component: Component,
}

#[derive(Clone, Debug)]
pub struct Scrollable {
    axis: Axis,
    child: Box<Component>,
}

impl Component {
    pub fn sized(x: SizeSource, y: SizeSource, child: Option<Component>) -> Self {
        Self::new(Kind::Sized(SizedComponent {
            x,
            y,
            child: child.map(Box::new),
        }))
    }

    pub fn fill(color: Color, child: Component) -> Self {
        Self::new(Kind::Fill(Fill {
            color,
            child: Box::new(child),
        }))
    }

    pub fn outline(color: Color, gap: f32, width: f32, child: Component) -> Self {
        Self::new(Kind::Outline(Outline {
            color,
            gap: gap.max(0.0),
            width: width.max(0.0),
            child: Box::new(child),
        }))
    }

    pub fn text(value: impl Into<String>) -> Self {
        Self::new(Kind::Text(Text {
            value: value.into(),
        }))
    }

    pub fn button(
        child: Component,
        on_state_change: impl Fn(&mut Component, ButtonState) + 'static,
    ) -> Self {
        Self::button_with_action(child, on_state_change, || {})
    }

    pub fn button_with_action(
        child: Component,
        on_state_change: impl Fn(&mut Component, ButtonState) + 'static,
        on_activate: impl Fn() + 'static,
    ) -> Self {
        Self::new(Kind::Button(Button {
            on_state_change: Arc::new(on_state_change),
            on_activate: Arc::new(on_activate),
            state: ButtonState::default(),
            child: Box::new(child),
        }))
    }

    pub fn list<const N: usize>(axis: Axis, children: [(Sizing, Component); N]) -> Self {
        Self::new(Kind::List(List {
            axis,
            children: children
                .into_iter()
                .map(|(sizing, component)| ListChild { sizing, component })
                .collect(),
        }))
    }

    pub fn scrollable(axis: Axis, child: Component) -> Self {
        Self::new(Kind::Scrollable(Scrollable {
            axis,
            child: Box::new(child),
        }))
    }

    pub fn layout(&mut self, recommendation: SizeRecommendation) -> Size {
        let size = match &mut self.kind {
            Kind::Sized(sized) => sized.layout(recommendation),
            Kind::Fill(fill) => fill.child.layout(recommendation),
            Kind::Text(text) => text.layout(),
            Kind::Outline(outline) => outline.child.layout(recommendation),
            Kind::Button(button) => button.child.layout(recommendation),
            Kind::List(list) => list.layout(recommendation),
            Kind::Scrollable(scrollable) => scrollable.layout(recommendation),
        };
        self.rect.width = size.width;
        self.rect.height = size.height;
        size
    }

    pub fn place(&mut self, rect: Rect) {
        self.rect = rect;
        match &mut self.kind {
            Kind::Sized(sized) => sized.place(),
            Kind::Text(_) => {}
            Kind::Fill(fill) => fill
                .child
                .place(Rect::new(0.0, 0.0, rect.width, rect.height)),
            Kind::Outline(outline) => {
                outline
                    .child
                    .place(Rect::new(0.0, 0.0, rect.width, rect.height))
            }
            Kind::Button(button) => {
                button
                    .child
                    .place(Rect::new(0.0, 0.0, rect.width, rect.height))
            }
            Kind::List(list) => list.place(rect.size()),
            Kind::Scrollable(scrollable) => scrollable.place(rect.size()),
        }
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn child_mut(&mut self) -> Option<&mut Component> {
        match &mut self.kind {
            Kind::Sized(sized) => sized.child.as_deref_mut(),
            Kind::Fill(fill) => Some(&mut fill.child),
            Kind::Outline(outline) => Some(&mut outline.child),
            Kind::Button(button) => Some(&mut button.child),
            Kind::Scrollable(scrollable) => Some(&mut scrollable.child),
            Kind::Text(_) | Kind::List(_) => None,
        }
    }

    pub fn set_fill_color(&mut self, color: Color) -> bool {
        let Kind::Fill(fill) = &mut self.kind else {
            return false;
        };
        fill.color = color;
        true
    }

    pub fn set_outline_width(&mut self, width: f32) -> bool {
        let Kind::Outline(outline) = &mut self.kind else {
            return false;
        };
        outline.width = width.max(0.0);
        true
    }

    fn new(kind: Kind) -> Self {
        Self {
            kind,
            rect: Rect::default(),
        }
    }

    pub(crate) fn paint(&self, scene: &mut Scene, offset_x: f32, offset_y: f32) {
        let x = offset_x + self.rect.x;
        let y = offset_y + self.rect.y;
        match &self.kind {
            Kind::Sized(sized) => {
                if let Some(child) = &sized.child {
                    child.paint(scene, x, y);
                }
            }
            Kind::Fill(fill) => {
                scene.fill_rect(
                    Rect::new(x, y, self.rect.width, self.rect.height),
                    fill.color,
                );
                fill.child.paint(scene, x, y);
            }
            Kind::Text(text) => scene.draw_text(x, y + 2.0, &text.value, Color::BLACK),
            Kind::Outline(outline) => {
                outline.child.paint(scene, x, y);
                let outset = outline.gap + outline.width;
                scene.stroke_rect(
                    Rect::new(
                        x - outset,
                        y - outset,
                        self.rect.width + outset * 2.0,
                        self.rect.height + outset * 2.0,
                    ),
                    outline.width,
                    outline.color,
                );
            }
            Kind::Button(button) => button.child.paint(scene, x, y),
            Kind::List(list) => {
                for child in &list.children {
                    child.component.paint(scene, x, y);
                }
            }
            Kind::Scrollable(scrollable) => {
                scrollable.child.paint(scene, x, y);
                let bar_color = Color::rgb(0xc0, 0xc0, 0xc0);
                match scrollable.axis {
                    Axis::Vertical => scene.fill_rect(
                        Rect::new(
                            x + self.rect.width - SCROLLBAR_SIZE,
                            y,
                            SCROLLBAR_SIZE,
                            self.rect.height,
                        ),
                        bar_color,
                    ),
                    Axis::Horizontal => scene.fill_rect(
                        Rect::new(
                            x,
                            y + self.rect.height - SCROLLBAR_SIZE,
                            self.rect.width,
                            SCROLLBAR_SIZE,
                        ),
                        bar_color,
                    ),
                }
            }
        }
    }

    pub(crate) fn set_button_focus(&mut self, target: Option<usize>, cursor: &mut usize) -> bool {
        let mut changed = false;
        match &mut self.kind {
            Kind::Button(button) => {
                let focused = target == Some(*cursor);
                *cursor += 1;
                changed |= button.set_state(focused, button.state.pressed);
            }
            Kind::Sized(sized) => {
                if let Some(child) = &mut sized.child {
                    changed |= child.set_button_focus(target, cursor);
                }
            }
            Kind::Fill(fill) => changed |= fill.child.set_button_focus(target, cursor),
            Kind::Outline(outline) => changed |= outline.child.set_button_focus(target, cursor),
            Kind::List(list) => {
                for child in &mut list.children {
                    changed |= child.component.set_button_focus(target, cursor);
                }
            }
            Kind::Scrollable(scrollable) => {
                changed |= scrollable.child.set_button_focus(target, cursor)
            }
            Kind::Text(_) => {}
        }
        changed
    }

    pub(crate) fn focused_button(&self, cursor: &mut usize) -> (Option<usize>, usize) {
        let mut focused = None;
        match &self.kind {
            Kind::Button(button) => {
                if button.state.focused {
                    focused = Some(*cursor);
                }
                *cursor += 1;
            }
            Kind::Sized(sized) => {
                if let Some(child) = &sized.child {
                    focused = child.focused_button(cursor).0;
                }
            }
            Kind::Fill(fill) => focused = fill.child.focused_button(cursor).0,
            Kind::Outline(outline) => focused = outline.child.focused_button(cursor).0,
            Kind::List(list) => {
                for child in &list.children {
                    let child_focused = child.component.focused_button(cursor).0;
                    focused = focused.or(child_focused);
                }
            }
            Kind::Scrollable(scrollable) => focused = scrollable.child.focused_button(cursor).0,
            Kind::Text(_) => {}
        }
        (focused, *cursor)
    }

    pub(crate) fn button_at(
        &self,
        point: (f32, f32),
        offset: (f32, f32),
        cursor: &mut usize,
    ) -> Option<usize> {
        let origin = (offset.0 + self.rect.x, offset.1 + self.rect.y);
        if !self.contains(point, origin) {
            *cursor += self.button_count();
            return None;
        }
        match &self.kind {
            Kind::Button(_) => {
                let index = *cursor;
                *cursor += 1;
                Some(index)
            }
            Kind::Sized(sized) => sized
                .child
                .as_ref()
                .and_then(|child| child.button_at(point, origin, cursor)),
            Kind::Fill(fill) => fill.child.button_at(point, origin, cursor),
            Kind::Outline(outline) => outline.child.button_at(point, origin, cursor),
            Kind::List(list) => list
                .children
                .iter()
                .find_map(|child| child.component.button_at(point, origin, cursor)),
            Kind::Scrollable(scrollable) => scrollable.child.button_at(point, origin, cursor),
            Kind::Text(_) => None,
        }
    }

    fn button_count(&self) -> usize {
        match &self.kind {
            Kind::Button(_) => 1,
            Kind::Sized(sized) => sized.child.as_ref().map_or(0, |child| child.button_count()),
            Kind::Fill(fill) => fill.child.button_count(),
            Kind::Outline(outline) => outline.child.button_count(),
            Kind::List(list) => list
                .children
                .iter()
                .map(|child| child.component.button_count())
                .sum(),
            Kind::Scrollable(scrollable) => scrollable.child.button_count(),
            Kind::Text(_) => 0,
        }
    }

    fn contains(&self, point: (f32, f32), origin: (f32, f32)) -> bool {
        point.0 >= origin.0
            && point.1 >= origin.1
            && point.0 < origin.0 + self.rect.width
            && point.1 < origin.1 + self.rect.height
    }

    pub(crate) fn set_button_pressed(
        &mut self,
        targets: [Option<usize>; 2],
        cursor: &mut usize,
    ) -> bool {
        let mut changed = false;
        match &mut self.kind {
            Kind::Button(button) => {
                let pressed = targets.contains(&Some(*cursor));
                *cursor += 1;
                changed |= button.set_state(button.state.focused, pressed);
            }
            Kind::Sized(sized) => {
                if let Some(child) = &mut sized.child {
                    changed |= child.set_button_pressed(targets, cursor);
                }
            }
            Kind::Fill(fill) => changed |= fill.child.set_button_pressed(targets, cursor),
            Kind::Outline(outline) => changed |= outline.child.set_button_pressed(targets, cursor),
            Kind::List(list) => {
                for child in &mut list.children {
                    changed |= child.component.set_button_pressed(targets, cursor);
                }
            }
            Kind::Scrollable(scrollable) => {
                changed |= scrollable.child.set_button_pressed(targets, cursor)
            }
            Kind::Text(_) => {}
        }
        changed
    }

    pub(crate) fn activate_button(&self, target: usize, cursor: &mut usize) -> bool {
        match &self.kind {
            Kind::Button(button) => {
                let matches = *cursor == target;
                *cursor += 1;
                if matches {
                    (button.on_activate)();
                }
                matches
            }
            Kind::Sized(sized) => sized
                .child
                .as_ref()
                .is_some_and(|child| child.activate_button(target, cursor)),
            Kind::Fill(fill) => fill.child.activate_button(target, cursor),
            Kind::Outline(outline) => outline.child.activate_button(target, cursor),
            Kind::List(list) => list
                .children
                .iter()
                .any(|child| child.component.activate_button(target, cursor)),
            Kind::Scrollable(scrollable) => scrollable.child.activate_button(target, cursor),
            Kind::Text(_) => false,
        }
    }
}

impl Button {
    fn set_state(&mut self, focused: bool, pressed: bool) -> bool {
        let state = ButtonState { focused, pressed };
        if self.state == state {
            return false;
        }
        self.state = state;
        (self.on_state_change)(&mut self.child, state);
        true
    }
}

impl SizedComponent {
    fn layout(&mut self, recommendation: SizeRecommendation) -> Size {
        let child_size = self
            .child
            .as_mut()
            .map(|child| child.layout(recommendation))
            .unwrap_or(Size::ZERO);
        Size::new(
            Self::axis_size(self.x, recommendation.width, child_size.width),
            Self::axis_size(self.y, recommendation.height, child_size.height),
        )
    }

    fn axis_size(source: SizeSource, parent: Option<f32>, child: f32) -> f32 {
        match source {
            SizeSource::Parent => parent.unwrap_or(0.0),
            SizeSource::Child => child,
            SizeSource::Zero => 0.0,
        }
    }

    fn place(&mut self) {
        if let Some(child) = &mut self.child {
            let size = child.rect.size();
            child.place(Rect::new(0.0, 0.0, size.width, size.height));
        }
    }
}

impl Text {
    fn layout(&self) -> Size {
        TextEngine::new()
            .map(|engine| engine.measure(&self.value))
            .unwrap_or_else(|| Size::new(self.value.chars().count() as f32 * 10.0, 20.0))
    }
}

impl List {
    fn layout(&mut self, recommendation: SizeRecommendation) -> Size {
        let axis = self.axis;
        let mut intrinsic_main: f32 = 0.0;
        let mut max_cross: f32 = 0.0;
        let mut fr_total: f32 = 0.0;

        for child in &mut self.children {
            match child.sizing {
                Sizing::Intrinsic => {
                    let size = child.component.layout(recommendation);
                    intrinsic_main += size.main(axis);
                    max_cross = max_cross.max(size.cross(axis));
                }
                Sizing::Fr(value) => fr_total += value.max(0.0),
            }
        }

        let remaining = recommendation
            .main(axis)
            .map(|main| (main - intrinsic_main).max(0.0));
        let mut fr_main: f32 = 0.0;

        for child in &mut self.children {
            if let Sizing::Fr(value) = child.sizing {
                let share = remaining.map(|remaining| {
                    if fr_total > 0.0 {
                        remaining * value.max(0.0) / fr_total
                    } else {
                        0.0
                    }
                });
                let size = child
                    .component
                    .layout(recommendation.with_main(axis, share));
                fr_main += size.main(axis);
                max_cross = max_cross.max(size.cross(axis));
            }
        }

        Size::from_axes(axis, intrinsic_main + fr_main, max_cross)
    }

    fn place(&mut self, size: Size) {
        let axis = self.axis;
        let mut cursor = 0.0;
        for child in &mut self.children {
            let child_size = child.component.rect.size();
            let rect = match axis {
                Axis::Horizontal => Rect::new(cursor, 0.0, child_size.width, size.height),
                Axis::Vertical => Rect::new(0.0, cursor, size.width, child_size.height),
            };
            child.component.place(rect);
            cursor += child_size.main(axis);
        }
    }
}

const SCROLLBAR_SIZE: f32 = 20.0;

impl Scrollable {
    fn layout(&mut self, recommendation: SizeRecommendation) -> Size {
        let viewport = Size::new(
            recommendation.width.unwrap_or(0.0),
            recommendation.height.unwrap_or(0.0),
        );
        let child_recommendation = match self.axis {
            Axis::Vertical => SizeRecommendation::new(
                Some((viewport.width - SCROLLBAR_SIZE).max(0.0)),
                Some(viewport.height),
            ),
            Axis::Horizontal => SizeRecommendation::new(
                Some(viewport.width),
                Some((viewport.height - SCROLLBAR_SIZE).max(0.0)),
            ),
        };
        self.child.layout(child_recommendation);
        viewport
    }

    fn place(&mut self, size: Size) {
        let child_size = self.child.rect.size();
        self.child
            .place(Rect::new(0.0, 0.0, child_size.width, child_size.height));
        match self.axis {
            Axis::Vertical => {
                self.child.rect.width = self.child.rect.width.min(size.width - SCROLLBAR_SIZE);
            }
            Axis::Horizontal => {
                self.child.rect.height = self.child.rect.height.min(size.height - SCROLLBAR_SIZE);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::Scene;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    #[test]
    fn vertical_list_measures_intrinsic_then_fr_children() {
        let mut list = Component::list(
            Axis::Vertical,
            [
                (Sizing::Intrinsic, Component::text("Demo")),
                (
                    Sizing::fr(1.0),
                    Component::sized(SizeSource::Parent, SizeSource::Parent, None),
                ),
            ],
        );

        let size = list.layout(SizeRecommendation::exact(800.0, 600.0));

        assert_eq!(size, Size::new(800.0, 600.0));
    }

    #[test]
    fn scrollable_passes_finite_viewport_recommendation_to_child() {
        let mut root = Component::scrollable(
            Axis::Vertical,
            Component::list(
                Axis::Vertical,
                [(
                    Sizing::fr(1.0),
                    Component::fill(
                        Color::WHITE,
                        Component::sized(SizeSource::Parent, SizeSource::Parent, None),
                    ),
                )],
            ),
        );

        let size = root.layout(SizeRecommendation::exact(800.0, 600.0));

        assert_eq!(size, Size::new(800.0, 600.0));
        match root.kind {
            Kind::Scrollable(scrollable) => {
                assert_eq!(scrollable.child.rect().size(), Size::new(780.0, 600.0));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn horizontal_sized_can_copy_width_without_inflating_height() {
        let mut row = Component::list(
            Axis::Horizontal,
            [
                (
                    Sizing::Intrinsic,
                    Component::button(Component::text("Demo"), |_, _| {}),
                ),
                (
                    Sizing::fr(1.0),
                    Component::sized(SizeSource::Parent, SizeSource::Zero, None),
                ),
            ],
        );

        let size = row.layout(SizeRecommendation::exact(800.0, 600.0));

        assert_eq!(size.width, 800.0);
        assert!(size.height > 0.0);
        assert!(size.height < 600.0);
    }

    #[test]
    fn button_paints_only_its_child() {
        let mut button = Component::button(
            Component::fill(
                Color::WHITE,
                Component::sized(SizeSource::Parent, SizeSource::Parent, None),
            ),
            |_, _| {},
        );
        let size = button.layout(SizeRecommendation::exact(100.0, 40.0));
        button.place(Rect::new(0.0, 0.0, size.width, size.height));
        let mut scene = Scene::new(100, 40);

        button.paint(&mut scene, 0.0, 0.0);

        assert_eq!(scene.vertices.len(), 4);
        assert_eq!(scene.indices.len(), 6);
    }

    #[test]
    fn outline_paints_outside_its_bounds_without_affecting_layout() {
        let mut outline = Component::outline(
            Color::BLACK,
            2.0,
            2.0,
            Component::fill(
                Color::WHITE,
                Component::sized(SizeSource::Parent, SizeSource::Parent, None),
            ),
        );
        let size = outline.layout(SizeRecommendation::exact(100.0, 40.0));
        outline.place(Rect::new(0.0, 0.0, size.width, size.height));
        let mut scene = Scene::new(100, 40);

        outline.paint(&mut scene, 0.0, 0.0);

        assert_eq!(size, Size::new(100.0, 40.0));
        assert_eq!(
            outline.child_mut().unwrap().rect(),
            Rect::new(0.0, 0.0, 100.0, 40.0)
        );
        assert_eq!(scene.vertices[0].color, Color::WHITE.as_f32());
        assert_eq!(scene.vertices[4].color, Color::BLACK.as_f32());
        assert_eq!(scene.vertices[4].position, [-1.08, 1.2]);
    }

    #[test]
    fn button_mutates_retained_child_when_interaction_state_changes() {
        let states = Arc::new(Mutex::new(Vec::new()));
        let changed_states = states.clone();
        let mut button = Component::button(
            Component::fill(Color::WHITE, Component::text("Demo")),
            move |child, state| {
                changed_states.lock().unwrap().push(state);
                child.set_fill_color(if state.pressed {
                    Color::BLACK
                } else {
                    Color::WHITE
                });
            },
        );
        let child_address = match &button.kind {
            Kind::Button(button) => (&*button.child) as *const Component,
            _ => unreachable!(),
        };

        assert!(button.set_button_focus(Some(0), &mut 0));
        assert!(button.set_button_pressed([Some(0), None], &mut 0));

        assert_eq!(
            *states.lock().unwrap(),
            vec![
                ButtonState {
                    focused: true,
                    pressed: false,
                },
                ButtonState {
                    focused: true,
                    pressed: true,
                },
            ]
        );
        match &button.kind {
            Kind::Button(button) => {
                assert_eq!((&*button.child) as *const Component, child_address);
                match button.child.kind {
                    Kind::Fill(ref fill) => assert_eq!(fill.color, Color::BLACK),
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn activating_button_runs_its_action() {
        let activations = Arc::new(AtomicUsize::new(0));
        let action_activations = activations.clone();
        let button = Component::button_with_action(
            Component::text("Demo"),
            |_, _| {},
            move || {
                action_activations.fetch_add(1, Ordering::Relaxed);
            },
        );

        button.activate_button(0, &mut 0);

        assert_eq!(activations.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn sized_passes_recommendation_to_child_and_selects_each_axis() {
        let mut component = Component::sized(
            SizeSource::Parent,
            SizeSource::Child,
            Some(Component::sized(SizeSource::Zero, SizeSource::Parent, None)),
        );

        let size = component.layout(SizeRecommendation::exact(320.0, 240.0));

        assert_eq!(size, Size::new(320.0, 240.0));
        match &component.kind {
            Kind::Sized(sized) => {
                assert_eq!(
                    sized.child.as_ref().unwrap().rect().size(),
                    Size::new(0.0, 240.0)
                );
            }
            _ => unreachable!(),
        }
    }
}
