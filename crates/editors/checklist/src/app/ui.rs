use std::rc::Rc;

use block_client::blocks::checklist::Checklist;
use block_editor_plugin::beui::reactive::{
    build, clone, create_memo, create_signal, view, with_reactive_scope, CenteredRow, Column, Fill,
    ForEach, ItemSize, Padding, ReadSignal, Scroll, Show, WriteSignal,
};
use block_editor_plugin::beui::styled::theme::{ACCENT, BACKGROUND, SURFACE_RAISED};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Card, Checkbox, Heading, Progress, TextInput,
    ToggleButton,
};
use block_editor_plugin::beui::{Color32, Context, Document, NodeId, Rect, TextAlign};

const PAGE_PADDING: f32 = 24.0;
const SECTION_SPACING: f32 = 18.0;

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub(super) struct ChecklistEntry {
    index: u32,
    text: String,
    done: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ChecklistSnapshot {
    entries: Vec<ChecklistEntry>,
}

impl From<&Checklist> for ChecklistSnapshot {
    fn from(checklist: &Checklist) -> Self {
        Self {
            entries: checklist
                .items()
                .iter()
                .enumerate()
                .map(|(index, item)| ChecklistEntry {
                    index: index as u32,
                    text: item.text.clone(),
                    done: item.done,
                })
                .collect(),
        }
    }
}

pub(super) trait ChecklistModel {
    fn snapshot(&self) -> ChecklistSnapshot;
    fn add(&self, text: String);
    fn set_done(&self, index: u32, done: bool);
    fn remove(&self, index: u32);
    fn clear_done(&self);
}

pub struct ChecklistUi {
    document: Document,
    set_snapshot: WriteSignal<ChecklistSnapshot>,
}

impl ChecklistUi {
    pub(super) fn new(checklist: Rc<dyn ChecklistModel>) -> Self {
        let (snapshot, set_snapshot) = create_signal(checklist.snapshot());
        let view_set_snapshot = set_snapshot.clone();
        let document = build(move || {
            view! { <ChecklistView checklist snapshot set_snapshot=view_set_snapshot /> }
        });
        Self {
            document,
            set_snapshot,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn background(&self) -> Color32 {
        BACKGROUND
    }

    pub(super) fn set_snapshot(&mut self, snapshot: ChecklistSnapshot) {
        let set_snapshot = self.set_snapshot.clone();
        with_reactive_scope(&mut self.document, move || set_snapshot.set(snapshot));
    }

    pub(super) fn show(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Filter {
    All,
    Open,
    Done,
}

impl Filter {
    fn keeps(self, entry: &ChecklistEntry) -> bool {
        match self {
            Self::All => true,
            Self::Open => !entry.done,
            Self::Done => entry.done,
        }
    }
}

#[block_editor_plugin::beui::reactive::component]
fn ChecklistView(
    checklist: Rc<dyn ChecklistModel>,
    snapshot: ReadSignal<ChecklistSnapshot>,
    set_snapshot: WriteSignal<ChecklistSnapshot>,
) -> NodeId {
    let (draft, set_draft) = create_signal(String::new());
    let (filter, set_filter) = create_signal(Filter::All);
    let visible_entries = create_memo(clone!(snapshot filter -> move || {
        let filter = filter.get();
        snapshot
            .get()
            .entries
            .into_iter()
            .filter(|entry| filter.keeps(entry))
            .collect::<Vec<_>>()
    }));
    let done_count = create_memo(clone!(snapshot -> move || {
        snapshot.get().entries.iter().filter(|entry| entry.done).count()
    }));
    let progress = create_memo(clone!(snapshot done_count -> move || {
        let total = snapshot.get().entries.len();
        if total == 0 {
            0.0
        } else {
            done_count.get() as f32 / total as f32
        }
    }));
    let summary = create_memo(clone!(snapshot done_count -> move || {
        let total = snapshot.get().entries.len();
        match total {
            0 => "Add your first task below".to_owned(),
            _ => format!("{} of {total} complete", done_count.get()),
        }
    }));
    let empty = create_memo(clone!(visible_entries -> move || visible_entries.get().is_empty()));
    let clear_disabled = create_memo(clone!(done_count -> move || done_count.get() == 0));
    let all_selected = create_memo(clone!(filter -> move || filter.get() == Filter::All));
    let open_selected = create_memo(clone!(filter -> move || filter.get() == Filter::Open));
    let done_selected = create_memo(clone!(filter -> move || filter.get() == Filter::Done));

    let submit_model = checklist.clone();
    let submit_snapshot = set_snapshot.clone();
    let submit_draft = set_draft.clone();
    let add_model = checklist.clone();
    let add_draft = draft.clone();
    let add_snapshot = set_snapshot.clone();
    let add_set_draft = set_draft.clone();
    let clear_model = checklist.clone();
    let clear_snapshot = set_snapshot.clone();
    let set_open_filter = set_filter.clone();
    let set_done_filter = set_filter.clone();
    let rows = clone!(checklist set_snapshot -> move |entry: ChecklistEntry| {
        view! {
            <ChecklistRow
                checklist={checklist.clone()}
                set_snapshot={set_snapshot.clone()}
                entry
            />
        }
    });

    view! {
        <Fill color=BACKGROUND radius=0>
            <Padding horizontal=PAGE_PADDING vertical=PAGE_PADDING>
                <Column spacing=SECTION_SPACING>
                    <Column spacing=6.0>
                        <Heading content="Checklist" />
                        <Caption content={summary} />
                        <Progress value={progress} label="Checklist completion" />
                    </Column>
                    <Card>
                        <Column spacing=12.0>
                            <CenteredRow spacing=10.0>
                                <TextInput
                                    @sizing=ItemSize::Percent(100.0)
                                    value={draft}
                                    placeholder="What needs doing?"
                                    label="New checklist item"
                                    @test_id={"checklist.draft"}
                                    on_change={move |value| set_draft.set(value)}
                                    on_submit={move |value| {
                                        add_item(&submit_model, &submit_snapshot, &submit_draft, value);
                                    }}
                                />
                                <Button
                                    label="Add task"
                                    variant=ButtonVariant::Primary
                                    @test_id={"checklist.add"}
                                    on_click={move || {
                                        add_item(&add_model, &add_snapshot, &add_set_draft, add_draft.get());
                                    }}
                                />
                            </CenteredRow>
                            <CenteredRow spacing=10.0>
                                <CenteredRow
                                    @sizing=ItemSize::Percent(100.0)
                                    spacing=6.0
                                >
                                    <ToggleButton
                                        label="All"
                                        pressed={all_selected}
                                        @test_id={"checklist.filter.all"}
                                        on_change={move |_| set_filter.set(Filter::All)}
                                    />
                                    <ToggleButton
                                        label="Open"
                                        pressed={open_selected}
                                        @test_id={"checklist.filter.open"}
                                        on_change={move |_| set_open_filter.set(Filter::Open)}
                                    />
                                    <ToggleButton
                                        label="Done"
                                        pressed={done_selected}
                                        @test_id={"checklist.filter.done"}
                                        on_change={move |_| set_done_filter.set(Filter::Done)}
                                    />
                                </CenteredRow>
                                <Button
                                    label="Clear completed"
                                    variant=ButtonVariant::Secondary
                                    disabled={clear_disabled}
                                    @test_id={"checklist.clear-done"}
                                    on_click={move || {
                                        clear_model.clear_done();
                                        set_from_model(&clear_snapshot, clear_model.as_ref());
                                    }}
                                />
                            </CenteredRow>
                        </Column>
                    </Card>
                    <Card @sizing=ItemSize::Percent(100.0)>
                        <Column spacing=10.0>
                            <Show condition={empty}>
                                <Body content="No tasks match this view." align=TextAlign::Center />
                            </Show>
                            <Scroll @sizing=ItemSize::Percent(100.0) focus_color=ACCENT>
                                <ForEach
                                    spacing=8.0
                                    items={visible_entries}
                                    key={|entry: ChecklistEntry| entry.clone()}
                                    view={rows}
                                />
                            </Scroll>
                        </Column>
                    </Card>
                </Column>
            </Padding>
        </Fill>
    }
}

fn add_item(
    checklist: &Rc<dyn ChecklistModel>,
    set_snapshot: &WriteSignal<ChecklistSnapshot>,
    set_draft: &WriteSignal<String>,
    value: String,
) {
    let text = value.trim().to_owned();
    if text.is_empty() {
        return;
    }
    checklist.add(text);
    set_from_model(set_snapshot, checklist.as_ref());
    set_draft.set(String::new());
}

fn set_from_model(set_snapshot: &WriteSignal<ChecklistSnapshot>, checklist: &dyn ChecklistModel) {
    set_snapshot.set(checklist.snapshot());
}

#[block_editor_plugin::beui::reactive::component]
fn ChecklistRow(
    checklist: Rc<dyn ChecklistModel>,
    set_snapshot: WriteSignal<ChecklistSnapshot>,
    entry: ChecklistEntry,
) -> NodeId {
    let done_index = entry.index;
    let remove_index = entry.index;
    let done_model = checklist.clone();
    let done_snapshot = set_snapshot.clone();
    view! {
        <Fill color=SURFACE_RAISED radius=6>
            <Padding horizontal=12.0 vertical=10.0>
                <CenteredRow spacing=10.0>
                    <Checkbox
                        @sizing=ItemSize::Percent(100.0)
                        label={entry.text}
                        checked={entry.done}
                        @test_id={format!("checklist.item.{}.done", entry.index)}
                        on_change={move |done| {
                            done_model.set_done(done_index, done);
                            set_from_model(&done_snapshot, done_model.as_ref());
                        }}
                    />
                    <Button
                        label="Remove"
                        variant=ButtonVariant::Secondary
                        @test_id={format!("checklist.item.{}.remove", entry.index)}
                        on_click={move || {
                            checklist.remove(remove_index);
                            set_from_model(&set_snapshot, checklist.as_ref());
                        }}
                    />
                </CenteredRow>
            </Padding>
        </Fill>
    }
}
