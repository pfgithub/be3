use super::*;

mod a_canvas_lays_out_only_the_items_the_view_can_see;
mod a_canvas_places_its_items_at_the_view_it_is_given;
mod a_canvas_without_a_view_places_its_items_from_its_own_corner;
mod a_closure_child_receives_the_handle_its_slot_hands_over;
mod a_component_function_returns_its_base_node;
mod a_disabled_button_prop_tracks_a_signal_and_blocks_clicks_while_true;
mod a_dynamic_child_can_fill_its_available_height;
mod a_hidden_show_gives_its_share_of_the_space_to_its_visible_siblings;
mod a_keyed_view_rebuilds_only_when_its_key_changes;
mod a_missing_reactive_prop_panics_instead_of_defaulting;
mod a_missing_required_prop_panics_at_the_view_that_wrote_it;
mod a_multi_root_view_fills_a_children_prop_in_order;
mod a_nested_container_reports_its_own_width_not_the_windows;
mod a_reactive_sizing_attribute_moves_a_child_between_fixed_and_percent;
mod a_reactive_tree_can_nest_builder_calls_without_threading_the_document;
mod a_selection_handle_takes_a_tap_before_the_button_it_covers;
mod a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame;
mod a_simulated_mouse_click_lands_where_the_trackpad_moved_its_cursor;
mod a_skipped_element_keeps_the_repaint_deadline_it_asked_for;
mod a_stack_becomes_a_column_when_its_container_gets_narrow;
mod a_stack_built_inside_a_show_still_measures_the_container_above_it;
mod a_tag_can_take_a_node_ref_and_a_test_id_slot_at_once;
mod a_theme_provider_restyles_its_subtree_when_its_theme_changes;
mod a_two_finger_drag_on_the_simulated_trackpad_scrolls_smoothly;
mod a_virtual_list_in_a_stacked_stack_only_builds_the_items_in_view;
mod a_virtual_scroll_only_builds_the_items_in_view;
mod a_virtual_scroll_row_can_build_reactive_content_during_dispatch;
mod accessibility_exposes_and_operates_a_button;
mod accessibility_reports_and_steps_a_slider;
mod an_empty_field_shows_its_placeholder_until_something_is_typed;
mod an_empty_view_builds_a_children_prop_with_nothing_in_it;
mod arrow_down_on_a_closed_select_trigger_opens_it_and_highlights_the_first_option;
mod arrow_keys_in_a_select_search_box_move_the_highlighted_option_without_editing_the_search_text;
mod arrow_keys_move_a_visible_highlight_through_an_open_context_menu;
mod arrow_keys_step_the_focused_slider;
mod arrow_keys_walk_the_rows_of_the_inspector_tree;
mod backspace_deletes_the_character_before_the_caret;
mod children_written_between_show_tags_are_not_built_until_it_is_shown;
mod choosing_the_e_ink_theme_in_the_inspector_restyles_the_document;
mod clicking_a_checkbox_toggles_it;
mod clicking_a_row_collapses_its_children;
mod clicking_a_row_selects_the_node_it_lists;
mod clicking_a_switch_moves_its_knob_and_survives_a_tab_round_trip;
mod clicking_a_tab_selects_the_panel_it_names;
mod clicking_an_accordion_header_hides_its_content;
mod clicking_outside_an_open_select_popup_closes_it_without_clicking_through;
mod clicking_the_middle_of_a_placeholder_puts_the_caret_at_the_start;
mod clicking_the_padding_around_a_button_label_activates_it;
mod clicking_the_start_of_a_text_input_puts_the_caret_before_the_text;
mod compacting_virtual_rows_clamps_the_scroll_anchor_at_the_end;
mod ctrl_a_selects_everything_so_typing_replaces_the_value;
mod ctrl_shift_f_moves_focus_between_the_inspector_and_the_document;
mod ctrl_shift_i_opens_and_closes_the_inspector;
mod ctrl_z_undoes_what_was_typed_into_a_text_input;
mod double_clicking_a_word_selects_it_so_typing_replaces_it;
mod dragging_a_slider_moves_its_value;
mod dragging_the_end_handle_of_a_double_tapped_word_extends_the_selection;
mod dragging_the_inspector_edge_resizes_the_panel;
mod editing_one_row_of_a_keyed_list_leaves_every_node_in_place;
mod enabling_touch_emulation_in_the_inspector_does_not_paint_a_pointer;
mod enter_activates_the_focused_button;
mod enter_confirms_the_highlighted_select_option_and_closes_the_popup;
mod enter_on_an_inspector_row_selects_it_and_toggles_its_children;
mod enter_toggles_the_focused_checkbox;
mod escape_closes_an_open_select_popup_and_returns_focus_to_the_trigger;
mod evicting_a_virtual_scroll_row_disposes_its_effects;
mod finding_a_node_by_its_test_id;
mod flashing_changed_elements_outlines_the_node_that_changed;
mod flashing_repaints_outlines_only_the_region_whose_shapes_changed;
mod flipping_a_switch_can_replace_the_items_of_a_scroll;
mod for_each_reuses_nodes_for_keys_that_persist_across_an_update;
mod holding_the_caret_handle_past_the_edge_of_a_narrow_input_keeps_scrolling;
mod holding_the_simulated_left_button_drags_while_another_finger_moves_the_cursor;
mod hovering_a_context_menu_item_moves_keyboard_focus_to_it;
mod hovering_a_menu_item_with_children_opens_its_submenu_without_a_click;
mod hovering_a_row_highlights_the_node_it_lists;
mod hovering_a_select_option_moves_the_keyboard_highlight;
mod jumping_up_a_virtual_scroll_only_builds_the_items_in_view;
mod opening_a_select_focuses_its_search_box_and_highlights_the_selected_option;
mod painting_skips_the_elements_outside_the_damaged_region;
mod percent_children_of_an_unbounded_list_use_their_intrinsic_length;
mod percent_sized_children_still_size_an_intrinsic_lists_height;
mod performance_measurements_report_work_and_cache_hits;
mod picking_a_node_leaves_the_document_alone;
mod picking_a_node_reveals_it_in_the_tree;
mod picking_a_node_scrolls_the_inspector_tree_to_its_row;
mod quadruple_clicking_selects_everything_so_typing_replaces_the_value;
mod removing_a_keyed_node_drops_the_test_ids_it_registered;
mod removing_a_node_runs_the_cleanups_its_components_registered;
mod removing_a_node_stops_the_effects_that_were_built_for_it;
mod resizing_a_virtual_scroll_reuses_visible_items;
mod resizing_an_element_damages_where_it_was_and_where_it_moved_to;
mod resizing_rows_preserves_the_scroll_anchor;
mod right_arrow_opens_a_submenu_and_left_arrow_closes_it_and_refocuses_the_parent_item;
mod right_click_opens_a_context_menu_at_the_cursor_position;
mod scrolling_a_virtual_scroll_replaces_the_items_in_view;
mod scrolling_a_virtual_scroll_reuses_overlapping_items;
mod selecting_a_leaf_item_in_a_nested_context_menu_closes_the_whole_menu_stack;
mod setting_the_value_of_a_text_input_reports_the_change;
mod shift_arrow_selects_the_character_that_typing_then_replaces;
mod shift_tab_moves_focus_to_the_previous_button;
mod show_lazily_builds_and_toggles_its_child_when_the_condition_changes;
mod simulating_a_device_pixel_ratio_in_the_inspector_changes_the_pixels_per_point;
mod sizing_attributes_on_the_roots_of_a_multi_root_view_are_honoured;
mod swiping_the_simulated_middle_button_scrolls_in_ticks;
mod tab_focus_stays_in_the_active_document_when_the_inspector_is_open;
mod tab_is_trapped_inside_an_open_context_menu;
mod tab_moves_focus_from_one_text_input_to_the_next;
mod tab_moves_focus_to_the_next_button;
mod tabs_collapse_into_a_select_when_their_container_is_narrow;
mod tapping_a_checkbox_with_touch_toggles_it;
mod tapping_inside_a_selection_in_a_select_search_box_opens_its_menu;
mod tapping_inside_a_touch_selection_opens_a_menu_that_copies_it;
mod tapping_the_caret_handle_opens_a_menu_that_asks_the_host_to_paste;
mod the_frame_output_reports_the_region_whose_shapes_changed;
mod the_inspector_follows_nodes_added_to_the_document;
mod the_inspector_keeps_its_native_size_while_a_pixel_ratio_is_simulated;
mod the_inspector_keeps_the_rows_of_nodes_that_survive_an_update;
mod the_inspector_lists_the_document_tree;
mod the_inspector_shows_document_performance;
mod the_inspector_shows_the_accesskit_tree;
mod the_inspector_shows_the_base_nodes_of_a_styled_component;
mod the_left_and_right_arrows_collapse_and_expand_an_inspector_row;
mod the_scroll_position_is_reported_to_its_listener;
mod the_simulated_keyboard_types_into_the_focused_input;
mod touch_dragging_a_scroll_moves_it_without_activating_a_row;
mod touch_dragging_across_a_text_input_does_not_select_its_text;
mod touch_overscroll_bands_without_hovering_a_row;
mod triple_clicking_selects_the_line_so_typing_replaces_the_value;
mod typing_in_a_select_search_box_filters_options_case_insensitively;
mod typing_in_the_inspector_tree_jumps_to_a_matching_row;
mod typing_into_a_focused_text_input_inserts_the_text;
mod typing_into_an_empty_field_does_not_pick_up_its_placeholder;
mod typing_past_the_end_of_a_narrow_text_input_scrolls_the_caret_into_view;
mod view_attributes_can_be_written_without_braces;
mod view_attributes_can_pun_a_bare_name_as_its_own_value;
mod view_children_can_pick_fixed_and_percent_sizing;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::color::Color32;
use crate::context::Context;
use crate::geometry::{Pos2, Vec2, pos2};
use crate::input::{Event, Key, Modifiers, PointerButton, RawInput};
use crate::input::{TouchId, TouchPhase};

use crate::base::list::{Direction, ItemSize};
use crate::inspector::Inspector;
use crate::reactive::{
    ClickCallback, Column, Frame, NodeRef, Spacer, Text, VirtualList, build, intrinsic,
    with_document,
};
use crate::styled;
use crate::unstyled;
use beui_macros::{component, view};

const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);
const WIDE_VIEWPORT: Vec2 = Vec2::new(1000.0, 600.0);
const VIRTUAL_ITEM_COUNT: usize = 10_000;
const VIRTUAL_ITEM_HEIGHT: f32 = 20.0;

pub(crate) struct Harness {
    context: Context,
    document: Document,
    viewport: Vec2,
}

impl Harness {
    pub(crate) fn new(document: Document) -> Self {
        Self {
            context: Context::new(),
            document,
            viewport: VIEWPORT,
        }
    }

    pub(crate) fn sized(document: Document, viewport: Vec2) -> Self {
        Self {
            viewport,
            ..Self::new(document)
        }
    }

    pub(crate) fn viewport_mut(&mut self) -> &mut Vec2 {
        &mut self.viewport
    }

    pub(crate) fn frame(&mut self, events: Vec<Event>) -> crate::FrameOutput {
        let Self {
            context,
            document,
            viewport,
        } = self;
        let input = RawInput { events };
        context.run(input, |context| {
            document.show(context, Rect::from_min_size(Pos2::ZERO, *viewport));
        })
    }

    pub(crate) fn click(&mut self, pos: Pos2) {
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }]);
        self.frame(vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]);
    }

    pub(crate) fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![Event::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }]);
        self.frame(vec![Event::PointerMoved(to)]);
        self.frame(vec![Event::PointerButton {
            pos: to,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]);
    }

    pub(crate) fn touch(&mut self, phase: TouchPhase, pos: Pos2) {
        self.finger(1, phase, pos);
    }

    pub(crate) fn finger(&mut self, finger: u64, phase: TouchPhase, pos: Pos2) {
        self.frame(vec![Event::Touch {
            id: TouchId { device: 1, finger },
            phase,
            pos,
            force: None,
        }]);
    }

    pub(crate) fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame(vec![key_event(key, true, modifiers)]);
        self.frame(vec![key_event(key, false, modifiers)]);
    }

    pub(crate) fn type_text(&mut self, text: &str) {
        for letter in text.chars() {
            self.frame(vec![Event::Text(letter.to_string())]);
        }
    }

    pub(crate) fn toggle_inspector(&mut self) {
        self.chord(Key::I);
    }

    pub(crate) fn toggle_picking(&mut self) {
        self.chord(Key::C);
    }

    pub(crate) fn toggle_inspector_focus(&mut self) {
        self.chord(Key::F);
    }

    fn chord(&mut self, key: Key) {
        self.key(
            key,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
        );
    }

    pub(crate) fn document(&self) -> &Document {
        &self.document
    }

    pub(crate) fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    pub(crate) fn rect(&self, id: NodeId) -> Rect {
        self.document
            .node_rect(id)
            .expect("the node was not laid out")
    }

    pub(crate) fn find(&self, test_id: &str) -> NodeId {
        self.document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("no node with test id {test_id:?}"))
    }

    pub(crate) fn center(&self, id: NodeId) -> Pos2 {
        self.rect(id).center()
    }

    pub(crate) fn inspector(&self) -> &Inspector {
        self.document
            .inspector
            .as_ref()
            .expect("the inspector is closed")
    }

    pub(crate) fn tree(&self) -> Vec<String> {
        self.inspector()
            .entries
            .iter()
            .map(|entry| format!("{}{}", "  ".repeat(entry.depth), entry.kind))
            .collect()
    }

    pub(crate) fn row_center(&self, index: usize) -> Pos2 {
        self.node_center(self.inspector().row_node(index))
    }

    pub(crate) fn focused_row_index(&self) -> Option<usize> {
        let focused = self.inspector().focused_row()?;
        self.inspector()
            .entries
            .iter()
            .position(|entry| entry.key == focused)
    }

    pub(crate) fn touch_toggle_center(&self) -> Pos2 {
        self.node_center(self.inspector().touch_toggle_node())
    }

    pub(crate) fn mouse_toggle_center(&self) -> Pos2 {
        self.node_center(self.inspector().mouse_toggle_node())
    }

    pub(crate) fn mouse_simulation(&self) -> bool {
        self.context.mouse_simulation()
    }

    pub(crate) fn simulated_cursor(&self) -> Pos2 {
        self.context.simulated_cursor()
    }

    pub(crate) fn simulated_button(&self, index: usize) -> Pos2 {
        self.context.simulated_button(index)
    }

    pub(crate) fn simulated_trackpad(&self) -> Pos2 {
        self.context.simulated_trackpad()
    }

    pub(crate) fn simulated_key(&self, label: &str) -> Pos2 {
        self.context
            .simulated_key(label)
            .unwrap_or_else(|| panic!("the simulated keyboard has no {label:?} key"))
    }

    pub(crate) fn enable_mouse_simulation(&mut self) {
        self.toggle_inspector();
        let tab = self.simulation_tab_center();
        self.click(tab);
        self.frame(Vec::new());
        let toggle = self.mouse_toggle_center();
        self.click(toggle);
        self.key(Key::Escape, Modifiers::NONE);
        self.frame(Vec::new());
    }

    pub(crate) fn point_at(&mut self, target: Pos2) {
        let origin = self.simulated_trackpad();
        let jog = Vec2::new(0.0, -40.0);
        let delta = target - self.simulated_cursor();
        self.finger(9, TouchPhase::Start, origin);
        self.finger(9, TouchPhase::Move, origin + jog);
        self.finger(9, TouchPhase::Move, origin + delta);
        self.finger(9, TouchPhase::End, origin + delta);
    }

    pub(crate) fn tap_trackpad(&mut self) {
        let at = self.simulated_trackpad();
        self.finger(1, TouchPhase::Start, at);
        self.finger(1, TouchPhase::End, at);
        self.frame(Vec::new());
    }

    pub(crate) fn change_flash_toggle_center(&self) -> Pos2 {
        self.node_center(self.inspector().change_flash_toggle_node())
    }

    pub(crate) fn damage_flash_toggle_center(&self) -> Pos2 {
        self.node_center(self.inspector().damage_flash_toggle_node())
    }

    pub(crate) fn accesskit_tab_center(&self) -> Pos2 {
        self.node_center(self.inspector().accesskit_tab_node())
    }

    pub(crate) fn performance_tab_center(&self) -> Pos2 {
        self.node_center(self.inspector().performance_tab_node())
    }

    pub(crate) fn simulation_tab_center(&self) -> Pos2 {
        self.node_center(self.inspector().simulation_tab_node())
    }

    pub(crate) fn pixel_ratio_option_center(&self, index: usize) -> Pos2 {
        self.node_center(self.inspector().pixel_ratio_option_node(index))
    }

    pub(crate) fn theme_option_center(&self, index: usize) -> Pos2 {
        self.node_center(self.inspector().theme_option_node(index))
    }

    pub(crate) fn performance_panel_visible(&self) -> bool {
        self.inspector()
            .document
            .node_rect(self.inspector().performance_panel_node())
            .is_some()
    }

    pub(crate) fn touch_emulation(&self) -> bool {
        self.context.touch_emulation()
    }

    fn node_center(&self, id: NodeId) -> Pos2 {
        let center = self
            .inspector()
            .document
            .node_rect(id)
            .expect("the row was not laid out")
            .center();
        let scale = crate::inspector::scale(&self.context);
        pos2(center.x * scale, center.y * scale)
    }
}

fn key_event(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        pressed,
        repeat: false,
        modifiers,
    }
}

pub(crate) fn with_installed<R>(document: &mut Document, f: impl FnOnce(&mut Document) -> R) -> R {
    crate::reactive::with_reactive_scope(document, || crate::reactive::with_document(f))
}

#[component]
pub(crate) fn MenuRegion() -> NodeId {
    view! {
        <Frame width=120.0 height=60.0 color=Color32::from_gray(80) radius=4 />
    }
}

#[component]
pub(crate) fn ButtonFace(label: String) -> NodeId {
    view! {
        <Frame color=Color32::from_gray(60) radius=4 padding_horizontal=20.0 padding_vertical=12.0>
            <Text string={label} font_size=14.0 color=Color32::WHITE />
        </Frame>
    }
}

#[component]
pub(crate) fn LabelledButton(label: String, on_click: ClickCallback) -> NodeId {
    view! {
        <unstyled::Button on_click={move || on_click.call()}>
            <ButtonFace label />
        </unstyled::Button>
    }
}

pub(crate) fn virtual_list(built: &Rc<RefCell<Vec<usize>>>) -> (Document, NodeId) {
    let scroll = NodeRef::new();
    let sink = built.clone();
    let document = build({
        let scroll = scroll.clone();
        move || {
            view! {
                <Column spacing=0.0>
                    <VirtualList
                        @sizing=ItemSize::Percent(100.0)
                        @node_ref=&scroll
                        count=VIRTUAL_ITEM_COUNT
                        item_height=VIRTUAL_ITEM_HEIGHT
                    >
                        {move |index: usize| {
                            sink.borrow_mut().push(index);
                            view! {
                                <Frame
                                    padding_horizontal=0.0
                                    padding_vertical={VIRTUAL_ITEM_HEIGHT / 2.0}
                                >
                                    <Spacer />
                                </Frame>
                            }
                        }}
                    </VirtualList>
                </Column>
            }
        }
    });
    (document, scroll.get())
}

pub(crate) struct HelloColumn {
    pub(crate) document: Document,
    pub(crate) padding: NodeId,
    pub(crate) text: NodeId,
}

pub(crate) fn hello_column() -> HelloColumn {
    let (padding, text) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (padding, text) = (padding.clone(), text.clone());
        move || {
            view! {
                <Column spacing=0.0>
                    <Frame @node_ref=&padding padding_horizontal=4.0 padding_vertical=4.0>
                        <Text @node_ref=&text string="Hello" font_size=14.0 color=Color32::WHITE />
                    </Frame>
                </Column>
            }
        }
    });
    HelloColumn {
        document,
        padding: padding.get(),
        text: text.get(),
    }
}

pub(crate) struct StackedPanels {
    pub(crate) document: Document,
    pub(crate) upper: NodeId,
    pub(crate) lower: NodeId,
}

pub(crate) fn stacked_panels() -> StackedPanels {
    let (upper, lower) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (upper, lower) = (upper.clone(), lower.clone());
        move || {
            view! {
                <Column spacing=0.0>
                    <Frame @node_ref=&upper height=100.0 color=Color32::WHITE radius=0 />
                    <Frame @node_ref=&lower height=100.0 color={Color32::from_gray(40)} radius=0 />
                </Column>
            }
        }
    });
    StackedPanels {
        document,
        upper: upper.get(),
        lower: lower.get(),
    }
}

pub(crate) fn flashed(output: &crate::FrameOutput, bounds: Rect, color: Color32) -> bool {
    output.shapes().iter().any(|shape| {
        matches!(
            shape,
            crate::painter::Shape::Rect {
                rect,
                stroke_width,
                color: painted,
                ..
            } if *rect == bounds
                && *stroke_width > 0.0
                && painted.to_array()[..3] == color.to_array()[..3]
        )
    })
}

pub(crate) fn text_of(document: &Document, id: NodeId) -> &str {
    document.text(id)
}

pub(crate) fn toolbar_of<const N: usize>(
    controls: impl FnOnce() -> [NodeId; N],
) -> (Document, [NodeId; N]) {
    let built = Rc::new(Cell::new(None));
    let sink = built.clone();
    let document = build(move || {
        let nodes = controls();
        sink.set(Some(nodes));
        view! {
            <Column spacing=8.0 children={nodes.map(intrinsic)} />
        }
    });
    let nodes = built.get().expect("the toolbar was built");
    (document, nodes)
}

use crate::node::{Element, InteractInput};
use crate::painter::Painter;
use std::any::Any;
use std::time::{Duration, Instant};

struct Counted {
    inner: Box<dyn Element>,
    layouts: Rc<Cell<usize>>,
    paints: Rc<Cell<usize>>,
}

impl Element for Counted {
    fn measure(&self, doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        self.inner.measure(doc, painter, available)
    }

    fn layout(
        &self,
        doc: &Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        self.layouts.set(self.layouts.get() + 1);
        self.inner.layout(doc, painter, rect, out);
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, rect: Rect) {
        self.paints.set(self.paints.get() + 1);
        self.inner.paint(doc, painter, rects, rect);
    }

    fn captures(&mut self, doc: &mut Document, pos: Pos2, rect: Rect) -> bool {
        self.inner.captures(doc, pos, rect)
    }

    fn interact(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        input: &InteractInput,
        id: NodeId,
        rect: Rect,
        focus_target: &mut Option<NodeId>,
    ) -> Vec<NodeId> {
        self.inner
            .interact(doc, painter, input, id, rect, focus_target)
    }

    fn children(&self) -> Vec<NodeId> {
        self.inner.children()
    }
    fn kind(&self) -> &'static str {
        self.inner.kind()
    }
    fn as_any(&self) -> &dyn Any {
        self.inner.as_any()
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self.inner.as_any_mut()
    }
}

fn counted(document: &mut Document, node: NodeId) -> (Rc<Cell<usize>>, Rc<Cell<usize>>) {
    let layouts = Rc::new(Cell::new(0));
    let paints = Rc::new(Cell::new(0));
    let inner = document.arena.take(node);
    document.arena.put_back(
        node,
        Box::new(Counted {
            inner,
            layouts: layouts.clone(),
            paints: paints.clone(),
        }),
    );
    (layouts, paints)
}
mod a_blinking_caret_only_damages_the_text_it_belongs_to;
mod a_click_handler_can_mutate_the_tree_in_the_current_frame;
mod accordion_headers_are_keyboard_operable_and_skip_collapsed_content;
mod activation_requires_a_matching_release_and_escape_cancels_it;
mod caret_repaints_on_a_deadline_without_repeating_layout;
mod clicking_a_choice_keeps_keyboard_focus_on_the_selected_option;
mod copy_and_cut_export_only_selected_text_and_cut_can_be_undone;
mod empty_choices_and_invalid_selection_do_not_break_tab_navigation;
mod every_styled_interactive_control_paints_a_keyboard_focus_ring;
mod focus_loss_and_hidden_content_cancel_keyboard_activation;
mod hover_only_repaints_when_its_handler_changes_a_node;
mod key_handlers_can_move_focus_and_change_their_tab_stop;
mod key_repeats_and_shortcut_modifiers_do_not_accidentally_activate_controls;
mod keyboard_scrolling_reaches_virtual_items_and_endpoints;
mod list_rows_and_pressables_activate_from_the_keyboard;
mod listbox_navigation_reveals_options_inside_a_tall_scroll_item;
mod listbox_typeahead_matches_prefixes_and_cycles_repeated_letters;
mod losing_window_focus_cancels_a_held_activation_key;
mod radio_groups_select_with_space_and_arrows_without_leaving_the_group;
mod resizing_scaling_and_replacing_the_root_invalidate_the_cache;
mod slider_home_end_and_page_keys_clamp_at_the_bounds;
mod space_toggles_checkboxes_switches_and_toggle_buttons;
mod tabbing_to_an_offscreen_control_reveals_it;
mod tabs_have_one_tab_stop_and_wrap_with_arrow_keys;
mod unchanged_input_reuses_layout_and_paint;

mod unused_navigation_keys_scroll_the_nearest_ancestor;
