use iced::alignment::Vertical;
use iced::widget::{column, container, row, scrollable, text};
use iced::{Element, Length, Padding};
use iced_m3::theme::{Accent, ColorScheme, Theme};
use iced_m3::widget::text_input::TextInput;
use iced_m3::widget::{self as m3, Action, button, card};
use launcher_core::{InstallPlan, Instance, InstanceId, Kind, Package, PackageSource};
use launcher_providers::has_catalog;

use crate::strings;
use crate::style::{self, Tone};

const TAB_LIST_WIDTH: f32 = 200.0;
const ROW_HEIGHT: f32 = 74.0;
const ROW_GAP: f32 = 6.0;
const ROW_PITCH: f32 = ROW_HEIGHT + ROW_GAP;
const OVERSCAN: usize = 3;
const DEFAULT_VIEWPORT: f32 = 600.0;
const SUMMARY_CHARS: usize = 90;
const ENTRY_PADDING: [f32; 2] = [10.0, 16.0];
const INPUT_PADDING: f32 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Version,
    Plugins,
    Browse,
    Settings,
}

const TABS: [(Tab, &str, &str); 4] = [
    (
        Tab::Version,
        strings::EDIT_TAB_VERSION,
        style::icons::HISTORY,
    ),
    (
        Tab::Plugins,
        strings::EDIT_TAB_PLUGINS,
        style::icons::EXTENSION,
    ),
    (Tab::Browse, strings::EDIT_TAB_BROWSE, style::icons::SEARCH),
    (
        Tab::Settings,
        strings::EDIT_TAB_SETTINGS,
        style::icons::TUNE,
    ),
];

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    TabSelected(Tab),
    SearchChanged(String),
    CategorySelected(Option<String>),
    Scrolled(f32, f32),
    ReloadCatalog,
    /// Package id.
    Install(String),
    /// Package id.
    Open(String),
    ConfirmPending,
    CancelPending,
    /// Plugin id.
    Remove(String),
    UpdateAll,
    NameChanged(String),
    GroupChanged(String),
    SaveSettings,
}

pub enum Catalog {
    Idle,
    Loading,
    Ready(Vec<Package>),
    Failed(String),
}

/// Installation that waits for the confirmation of the user.
pub struct Pending {
    pub package: Package,
    pub plan: InstallPlan,
}

pub struct State {
    pub id: InstanceId,
    pub kind: Kind,
    pub tab: Tab,
    pub search: String,
    pub category: Option<String>,
    pub scroll_y: f32,
    pub viewport_height: f32,
    pub catalog: Catalog,
    pub pending: Option<Pending>,
    pub name: String,
    pub group: String,
}

impl State {
    pub fn new(instance: &Instance) -> Self {
        Self {
            id: instance.config.id,
            kind: instance.config.kind,
            tab: Tab::Plugins,
            search: String::new(),
            category: None,
            scroll_y: 0.0,
            viewport_height: DEFAULT_VIEWPORT,
            catalog: Catalog::Idle,
            pending: None,
            name: instance.config.name.clone(),
            group: instance.config.group.clone().unwrap_or_default(),
        }
    }

    pub fn package(&self, id: &str) -> Option<&Package> {
        match &self.catalog {
            Catalog::Ready(packages) => packages.iter().find(|package| package.id == id),
            _ => None,
        }
    }

    pub fn can_save(&self) -> bool {
        !self.name.trim().is_empty()
    }

    pub const fn needs_catalog(&self) -> bool {
        has_catalog(self.kind) && matches!(self.catalog, Catalog::Idle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    NotInstalled,
    Installed,
    UpdateAvailable,
}

fn status_of(instance: &Instance, package: &Package) -> Status {
    let Some(record) = instance
        .config
        .plugins
        .iter()
        .find(|record| record.id == package.id)
    else {
        return Status::NotInstalled;
    };
    match package.latest_version() {
        Some(latest) if latest != record.version => Status::UpdateAvailable,
        _ => Status::Installed,
    }
}

fn matches_query(package: &Package, needle: &str) -> bool {
    needle.is_empty()
        || [&package.name, &package.summary, &package.author]
            .iter()
            .any(|field| field.to_lowercase().contains(needle))
}

pub fn view<'a>(
    state: &'a State,
    instance: &'a Instance,
    theme: &'a Theme,
) -> Element<'a, Message> {
    let header = row![
        m3::button(
            button::Style::tonal(theme, Accent::Primary),
            style::btn_content(style::icons::BACK, strings::BACK),
        )
        .on_press(Message::Back),
        row![
            style::icon(style::kind_icon(instance.config.kind))
                .size(26)
                .color(theme.primary()),
            style::label(theme, instance.config.name.clone()).size(24),
            style::icon_chip::<Message>(
                theme,
                style::kind_icon(instance.config.kind),
                strings::kind_label(instance.config.kind),
                style::kind_tone(instance.config.kind),
            ),
        ]
        .spacing(8)
        .align_y(Vertical::Center)
        .width(Length::Fill),
    ]
    .spacing(12)
    .align_y(Vertical::Center);

    let tabs: Vec<Element<'a, Message>> = TABS
        .iter()
        .map(|(tab, label, glyph)| tab_entry(theme, *tab, label, glyph, state.tab == *tab))
        .collect();
    let tab_card = m3::card(card::Style::filled(theme), column(tabs).spacing(8))
        .width(Length::Fixed(TAB_LIST_WIDTH));

    let content: Element<'a, Message> = match state.tab {
        Tab::Version => version_tab(theme, instance),
        Tab::Plugins => plugins_tab(theme, instance),
        Tab::Browse => browse_tab(theme, state, instance),
        Tab::Settings => settings_tab(theme, state),
    };
    let content_card = m3::card(card::Style::filled(theme), content)
        .width(Length::Fill)
        .height(Length::Fill);

    column![
        header,
        row![tab_card, content_card]
            .spacing(style::GAP)
            .height(Length::Fill)
    ]
    .spacing(style::GAP)
    .into()
}

fn tab_entry<'a>(
    theme: &Theme,
    tab: Tab,
    label: &'static str,
    glyph: &'static str,
    selected: bool,
) -> Element<'a, Message> {
    let color = style::tile_content(theme, selected);
    let icon_color = if selected {
        theme.on_primary_container()
    } else {
        theme.primary()
    };
    m3::card(
        style::tile_card(theme, selected),
        row![
            style::icon(glyph).size(18).color(icon_color),
            text(label).size(14).color(color),
        ]
        .spacing(10)
        .align_y(Vertical::Center),
    )
    .interaction(card::Interaction::Press(Action::Direct(
        Message::TabSelected(tab),
    )))
    .padding(Padding::from(ENTRY_PADDING))
    .width(Length::Fill)
    .into()
}

fn field_with_icon<'a>(
    theme: &Theme,
    glyph: &'static str,
    label: &str,
    value: &str,
) -> Element<'a, Message> {
    row![
        style::icon(glyph).size(16).color(theme.primary()),
        style::label(theme, format!("{label}: {value}")).size(14),
    ]
    .spacing(6)
    .align_y(Vertical::Center)
    .into()
}

fn version_tab<'a>(theme: &Theme, instance: &Instance) -> Element<'a, Message> {
    let config = &instance.config;
    let mut lines = column![style::icon_chip::<Message>(
        theme,
        style::kind_icon(config.kind),
        strings::kind_label(config.kind),
        style::kind_tone(config.kind)
    ),]
    .spacing(10);
    if config.kind == Kind::Ymm4 {
        lines = lines.push(field_with_icon(
            theme,
            style::icons::LAYERS,
            strings::EDITION,
            strings::edition_label(config.edition),
        ));
    }
    lines = lines.push(field_with_icon(
        theme,
        style::icons::HISTORY,
        strings::VERSION,
        &config.app_version,
    ));
    if let Some(addon) = &config.addon_version {
        lines = lines.push(field_with_icon(
            theme,
            style::icons::EXTENSION,
            strings::ADDON_VERSION,
            addon,
        ));
    }
    lines = lines
        .push(field_with_icon(
            theme,
            style::icons::GROUP,
            strings::GROUP,
            config.group.as_deref().unwrap_or(strings::UNGROUPED),
        ))
        .push(field_with_icon(
            theme,
            style::icons::EXTENSION,
            strings::PLUGINS,
            &config.plugins.len().to_string(),
        ));
    lines
        .push(
            m3::button(
                button::Style::filled(theme, Accent::Primary),
                style::btn_content(style::icons::UPDATE, strings::UPDATE_ALL),
            )
            .on_press(Message::UpdateAll),
        )
        .into()
}

fn plugins_tab<'a>(theme: &Theme, instance: &Instance) -> Element<'a, Message> {
    if instance.config.plugins.is_empty() {
        return row![
            style::icon(style::icons::EXTENSION)
                .size(20)
                .color(theme.on_surface_variant()),
            style::label(theme, strings::NO_PLUGINS).size(14),
        ]
        .spacing(8)
        .align_y(Vertical::Center)
        .into();
    }
    let rows: Vec<Element<'a, Message>> = instance
        .config
        .plugins
        .iter()
        .map(|record| {
            let info = column![
                row![
                    style::icon(style::icons::EXTENSION)
                        .size(16)
                        .color(theme.primary()),
                    style::label(theme, record.id.clone()).size(16),
                ]
                .spacing(6)
                .align_y(Vertical::Center),
                row![
                    style::icon(style::icons::HISTORY)
                        .size(12)
                        .color(theme.on_surface_variant()),
                    style::label(
                        theme,
                        format!(
                            "{} | {}: {}",
                            record.version,
                            strings::FILES,
                            record.files.len()
                        )
                    )
                    .size(12),
                ]
                .spacing(4)
                .align_y(Vertical::Center),
            ]
            .spacing(3)
            .width(Length::Fill);

            let remove = m3::button(
                style::danger_button(theme),
                style::btn_content(style::icons::DELETE, strings::DELETE),
            )
            .on_press(Message::Remove(record.id.clone()));

            m3::card(
                style::tile_card(theme, false),
                row![info, remove]
                    .spacing(style::GAP)
                    .align_y(Vertical::Center),
            )
            .padding(Padding::from([10.0, 16.0]))
            .width(Length::Fill)
            .into()
        })
        .collect();
    scrollable(column(rows).spacing(ROW_GAP))
        .height(Length::Fill)
        .style(style::scroll(theme))
        .into()
}

fn browse_tab<'a>(
    theme: &'a Theme,
    state: &'a State,
    instance: &'a Instance,
) -> Element<'a, Message> {
    if !has_catalog(state.kind) {
        return row![
            style::icon(style::icons::INFO)
                .size(20)
                .color(theme.on_surface_variant()),
            style::label(theme, strings::NO_CATALOG).size(14),
        ]
        .spacing(8)
        .align_y(Vertical::Center)
        .into();
    }
    let packages = match &state.catalog {
        Catalog::Idle | Catalog::Loading => {
            return row![
                style::icon(style::icons::SYNC)
                    .size(20)
                    .color(theme.primary()),
                style::label(theme, strings::LOADING).size(14),
            ]
            .spacing(8)
            .align_y(Vertical::Center)
            .into();
        }
        Catalog::Failed(reason) => {
            return column![
                row![
                    style::icon(style::icons::ERROR)
                        .size(16)
                        .color(theme.error()),
                    style::label(theme, format!("{}: {reason}", strings::LOAD_FAILED)).size(12),
                ]
                .spacing(6)
                .align_y(Vertical::Center),
                m3::button(
                    button::Style::tonal(theme, Accent::Primary),
                    style::btn_content(style::icons::REFRESH, strings::RELOAD),
                )
                .on_press(Message::ReloadCatalog),
            ]
            .spacing(6)
            .into();
        }
        Catalog::Ready(packages) => packages,
    };

    let search = TextInput::new(strings::SEARCH_HINT, &state.search, theme)
        .with_label_text(strings::SEARCH, card::Style::filled(theme).container_color)
        .on_input(Message::SearchChanged)
        .padding(INPUT_PADDING);
    let refresh = m3::button(
        button::Style::tonal(theme, Accent::Primary),
        style::btn_content(style::icons::REFRESH, strings::REFRESH),
    )
    .on_press(Message::ReloadCatalog);
    let mut page =
        column![row![search, refresh].spacing(8).align_y(Vertical::Center)].spacing(style::GAP);

    let mut categories: Vec<&str> = packages
        .iter()
        .flat_map(|package| package.categories.iter().map(String::as_str))
        .collect();
    categories.sort_unstable();
    categories.dedup();
    if !categories.is_empty() {
        page = page.push(category_bar(theme, &categories, state.category.as_deref()));
    }
    if let Some(pending) = &state.pending {
        page = page.push(pending_card(theme, pending));
    }

    let needle = state.search.trim().to_lowercase();
    let shown: Vec<&Package> = packages
        .iter()
        .filter(|package| {
            state
                .category
                .as_ref()
                .is_none_or(|category| package.categories.contains(category))
        })
        .filter(|package| matches_query(package, &needle))
        .collect();
    if shown.is_empty() {
        return page
            .push(
                row![
                    style::icon(style::icons::SEARCH)
                        .size(16)
                        .color(theme.on_surface_variant()),
                    style::label(theme, strings::NO_MATCH).size(14),
                ]
                .spacing(8)
                .align_y(Vertical::Center),
            )
            .into();
    }
    page.push(package_list(theme, &shown, instance, state))
        .into()
}

fn category_bar<'a>(
    theme: &Theme,
    categories: &[&str],
    current: Option<&str>,
) -> Element<'a, Message> {
    let entry = |label: String, value: Option<String>, selected: bool| -> Element<'a, Message> {
        let glyph = if value.is_none() {
            style::icons::CATEGORY
        } else {
            style::icons::EXTENSION
        };
        let color = style::tile_content(theme, selected);
        let icon_color = if selected {
            theme.on_primary_container()
        } else {
            theme.primary()
        };
        m3::card(
            style::tile_card(theme, selected),
            row![
                style::icon(glyph).size(14).color(icon_color),
                text(label).size(13).color(color),
            ]
            .spacing(6)
            .align_y(Vertical::Center),
        )
        .interaction(card::Interaction::Press(Action::Direct(
            Message::CategorySelected(value),
        )))
        .padding(Padding::from([4.0, 12.0]))
        .into()
    };
    let mut entries = vec![entry(
        strings::ALL_CATEGORIES.to_owned(),
        None,
        current.is_none(),
    )];
    entries.extend(categories.iter().map(|category| {
        entry(
            (*category).to_owned(),
            Some((*category).to_owned()),
            current == Some(*category),
        )
    }));
    scrollable(row(entries).spacing(8))
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .style(style::scroll(theme))
        .into()
}

fn pending_card<'a>(theme: &Theme, pending: &Pending) -> Element<'a, Message> {
    m3::card(
        card::Style::filled(theme),
        row![
            style::icon(style::icons::WARNING)
                .size(20)
                .color(theme.error()),
            style::label(
                theme,
                format!("{}: {}", pending.package.name, strings::UNVERIFIED_WARNING)
            )
            .size(13)
            .width(Length::Fill),
            m3::button(
                button::Style::filled(theme, Accent::Primary),
                style::btn_content(style::icons::CHECK, strings::CONTINUE),
            )
            .on_press(Message::ConfirmPending),
            m3::button(
                button::Style::tonal(theme, Accent::Primary),
                style::btn_content(style::icons::CLOSE, strings::CANCEL),
            )
            .on_press(Message::CancelPending),
        ]
        .spacing(8)
        .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .into()
}

/// Builds only the rows near the viewport. Spacers keep the height of the omitted rows.
fn package_list<'a>(
    theme: &Theme,
    shown: &[&Package],
    instance: &Instance,
    state: &State,
) -> Element<'a, Message> {
    let total = shown.len();
    let content_height = extent(total);
    let offset = state
        .scroll_y
        .clamp(0.0, (content_height - state.viewport_height).max(0.0));
    let first = row_at(offset, total);
    let visible = row_at(state.viewport_height, total) + 2;
    let start = first.saturating_sub(OVERSCAN).min(total);
    let end = (first + visible + OVERSCAN).min(total);

    let mut rows: Vec<Element<'a, Message>> = Vec::with_capacity(end - start + 2);
    rows.push(spacer(extent(start)));
    rows.extend(
        shown[start..end]
            .iter()
            .map(|package| package_row(theme, package, status_of(instance, package))),
    );
    rows.push(spacer(extent(total - end)));
    scrollable(column(rows).spacing(0))
        .on_scroll(|viewport| {
            Message::Scrolled(viewport.absolute_offset().y, viewport.bounds().height)
        })
        .height(Length::Fill)
        .style(style::scroll(theme))
        .into()
}

/// Height of `rows` rows. Counts above `u16::MAX` saturate.
fn extent(rows: usize) -> f32 {
    f32::from(u16::try_from(rows).unwrap_or(u16::MAX)) * ROW_PITCH
}

/// Index of the row that contains `offset`, or `total` when `offset` lies below every row.
fn row_at(offset: f32, total: usize) -> usize {
    let (mut low, mut high) = (0, total);
    while low < high {
        let middle = low + (high - low) / 2;
        if extent(middle + 1) <= offset {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low
}

fn spacer<'a>(height: f32) -> Element<'a, Message> {
    container(column![]).height(Length::Fixed(height)).into()
}

fn truncated(summary: &str) -> String {
    let line = summary.lines().next().unwrap_or_default();
    if line.chars().count() > SUMMARY_CHARS {
        let head: String = line.chars().take(SUMMARY_CHARS).collect();
        format!("{head}...")
    } else {
        line.to_owned()
    }
}

fn package_row<'a>(theme: &Theme, package: &Package, status: Status) -> Element<'a, Message> {
    let info = column![
        row![
            style::icon(style::icons::EXTENSION)
                .size(16)
                .color(theme.primary()),
            style::label(theme, package.name.clone()).size(16),
            style::label(theme, format!("by {}", package.author)).size(12),
        ]
        .spacing(6)
        .align_y(Vertical::Center),
        style::label(theme, truncated(&package.summary)).size(12),
    ]
    .spacing(2)
    .width(Length::Fill);

    let (status_text, status_icon, status_tone) = match status {
        Status::NotInstalled => (
            strings::NOT_INSTALLED,
            style::icons::DOWNLOAD,
            Tone::Secondary,
        ),
        Status::Installed => (
            strings::INSTALLED,
            style::icons::CHECK_CIRCLE,
            Tone::Tertiary,
        ),
        Status::UpdateAvailable => (
            strings::UPDATE_AVAILABLE,
            style::icons::UPDATE,
            Tone::Primary,
        ),
    };
    let action = match (&package.source, status) {
        (PackageSource::Web(_), _) => Some((
            style::icons::OPEN_IN_NEW,
            strings::OPEN_IN_BROWSER,
            Message::Open(package.id.clone()),
        )),
        (_, Status::NotInstalled) => Some((
            style::icons::DOWNLOAD,
            strings::INSTALL,
            Message::Install(package.id.clone()),
        )),
        (_, Status::UpdateAvailable) => Some((
            style::icons::UPDATE,
            strings::UPDATE,
            Message::Install(package.id.clone()),
        )),
        (_, Status::Installed) => None,
    };
    let act = match action {
        Some((glyph, label, message)) => m3::button(
            button::Style::filled(theme, Accent::Primary),
            style::btn_content(glyph, label),
        )
        .on_press(message),
        None => m3::button(
            button::Style::tonal(theme, Accent::Primary),
            style::btn_content(style::icons::CHECK, strings::INSTALLED),
        ),
    };

    let body = row![
        info,
        style::icon_chip::<Message>(theme, status_icon, status_text, status_tone),
        act
    ]
    .spacing(style::GAP)
    .align_y(Vertical::Center);
    container(
        m3::card(style::tile_card(theme, false), body)
            .padding(Padding::from([8.0, 14.0]))
            .width(Length::Fill)
            .height(Length::Fixed(ROW_HEIGHT)),
    )
    .padding(Padding::default().bottom(ROW_GAP))
    .into()
}

fn settings_tab<'a>(theme: &'a Theme, state: &'a State) -> Element<'a, Message> {
    let background = card::Style::filled(theme).container_color;
    let name = TextInput::new("", &state.name, theme)
        .with_label_text(strings::NAME, background)
        .on_input(Message::NameChanged)
        .padding(INPUT_PADDING);
    let group = TextInput::new(strings::GROUP_HINT, &state.group, theme)
        .with_label_text(strings::GROUP, background)
        .on_input(Message::GroupChanged)
        .padding(INPUT_PADDING);
    let save = m3::button(
        button::Style::filled(theme, Accent::Primary),
        style::btn_content(style::icons::SAVE, strings::SAVE),
    )
    .on_press_maybe(state.can_save().then_some(Message::SaveSettings));

    let fields = column![
        row![
            style::icon(style::icons::EDIT)
                .size(16)
                .color(theme.primary()),
            style::label(theme, strings::NAME).size(14),
        ]
        .spacing(6)
        .align_y(Vertical::Center),
        name,
        row![
            style::icon(style::icons::GROUP)
                .size(16)
                .color(theme.primary()),
            style::label(theme, strings::GROUP).size(14),
        ]
        .spacing(6)
        .align_y(Vertical::Center),
        group,
        save
    ]
    .spacing(style::GAP);

    fields.into()
}
