use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};

use iced::alignment::Vertical;
use iced::widget::{Id, checkbox, column, container, image, mouse_area, row, scrollable, text};
use iced::{ContentFit, Element, Length, Padding, Task};
use iced_m3::theme::{Accent, ColorScheme, Theme};
use iced_m3::widget::text_input::TextInput;
use iced_m3::widget::{self as m3, Action, button, card};
use launcher_core::{
    InstallPlan, Instance, InstanceId, Kind, Package, PackageSource, SortOrder, sort_packages,
    today,
};
use launcher_providers::{has_catalog, parse_aviutl2_manifest};
use rust_i18n::t;

use crate::strings;
use crate::style::{self, Tone};

const TAB_LIST_WIDTH: f32 = 200.0;
const ROW_HEIGHT: f32 = 78.0;
const ROW_GAP: f32 = 6.0;
const ROW_PITCH: f32 = ROW_HEIGHT + ROW_GAP;
const OVERSCAN: usize = 3;
const DEFAULT_VIEWPORT: f32 = 600.0;
const SUMMARY_CHARS: usize = 90;
const ENTRY_PADDING: [f32; 2] = [10.0, 16.0];
const INPUT_PADDING: f32 = 12.0;
const THUMBNAIL_HEIGHT: f32 = 280.0;
const PREVIEW_HEIGHT: f32 = 640.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Version,
    Plugins,
    Browse,
    Commons,
    JsonRegister,
    Settings,
}

fn tabs() -> [(Tab, Cow<'static, str>, &'static str); 6] {
    [
        (Tab::Version, t!("edit_tab_version"), style::icons::HISTORY),
        (
            Tab::Plugins,
            t!("edit_tab_plugins"),
            style::icons::EXTENSION,
        ),
        (Tab::Browse, t!("edit_tab_browse"), style::icons::SEARCH),
        (Tab::Commons, t!("edit_tab_commons"), style::icons::SHARE),
        (
            Tab::JsonRegister,
            t!("json_direct_register"),
            style::icons::CODE,
        ),
        (Tab::Settings, t!("edit_tab_settings"), style::icons::TUNE),
    ]
}

/// Identifier of the scrollable that lists the catalog packages of the browse tab.
fn browse_scroll_id() -> Id {
    Id::new("browse-packages")
}

/// Scrolls the package list of the browse tab to `offset`.
/// The list is rebuilt at offset 0 whenever the tab or the detail view is left and shown again.
pub fn restore_scroll<T: 'static>(offset: f32) -> Task<T> {
    iced::widget::operation::scroll_to(
        browse_scroll_id(),
        scrollable::AbsoluteOffset {
            x: None,
            y: Some(offset),
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusFilter {
    #[default]
    All,
    Installed,
    NotInstalled,
}

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    TabSelected(Tab),
    SearchChanged(String),
    CategorySelected(Option<String>),
    TagToggled(String),
    TagsCleared,
    SortOrderChanged(SortOrder),
    StatusFilterChanged(StatusFilter),
    ToggleHideDeprecated,
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
    RequestUpdateAll,
    ConfirmUpdateAll,
    CancelUpdateAll,
    TogglePausePlugin(String, bool),
    ScanManualPlugins,
    // Commons tab messages
    ToggleCommonsSelect(String),
    SelectAllCommons,
    DeselectAllCommons,
    CopySelectedCommons,
    CopySingleCommons(String),
    OpenCommonsUrl(String),
    OpenCommonsGuide,
    // JSON register tab messages
    JsonInputChanged(String),
    LoadSampleJson,
    RegisterJsonPackage,
    // Details & carousel
    ViewPackageDetails(Option<String>),
    NextScreenshot(usize),
    PrevScreenshot(usize),
    ShowLicense(Option<String>),
    TogglePreview,
    // Config export & import
    ExportConfig,
    ImportConfig,
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

/// A screenshot of the package detail view.
#[derive(Debug, Clone)]
pub enum ImageState {
    Loading,
    Ready(image::Handle),
    Failed(String),
}

/// Installation that waits for the confirmation of the user.
pub struct Pending {
    pub package: Package,
    pub plan: InstallPlan,
}

pub type UpdatePreviewItem = (String, String, String, Option<String>);

pub struct State {
    pub id: InstanceId,
    pub kind: Kind,
    pub tab: Tab,
    pub search: String,
    pub category: Option<String>,
    pub tags: BTreeSet<String>,
    pub sort_order: SortOrder,
    pub status_filter: StatusFilter,
    pub hide_deprecated: bool,
    pub scroll_y: f32,
    /// Offset that `restore_scroll` requested. `Scrolled` events of the rebuilt list at
    /// offset 0 do not overwrite `scroll_y` while this is set.
    pub pending_scroll: Option<f32>,
    pub viewport_height: f32,
    pub catalog: Catalog,
    pub pending: Option<Pending>,
    pub selected_commons: HashSet<String>,
    pub json_input: String,
    pub json_status: Option<Result<String, String>>,
    /// Operations that installing the registered manifest performs, one per line.
    pub json_log: Vec<String>,
    pub viewing_package_id: Option<String>,
    pub active_screenshot_idx: usize,
    /// Screenshots by url.
    pub images: HashMap<String, ImageState>,
    /// The screenshot is shown at `PREVIEW_HEIGHT` instead of `THUMBNAIL_HEIGHT`.
    pub preview_open: bool,
    pub license_dialog_content: Option<String>,
    pub update_preview: Option<Vec<UpdatePreviewItem>>,
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
            tags: BTreeSet::new(),
            sort_order: SortOrder::Popularity,
            status_filter: StatusFilter::All,
            hide_deprecated: false,
            scroll_y: 0.0,
            pending_scroll: None,
            viewport_height: DEFAULT_VIEWPORT,
            catalog: Catalog::Idle,
            pending: None,
            selected_commons: HashSet::new(),
            json_input: String::new(),
            json_status: None,
            json_log: Vec::new(),
            viewing_package_id: None,
            active_screenshot_idx: 0,
            images: HashMap::new(),
            preview_open: false,
            license_dialog_content: None,
            update_preview: None,
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

    /// Tries to parse the json_input as a Package or RawEntry and append to catalog.
    /// Registers the manifest in `json_input` into the open catalog and fills `json_log`
    /// with the operations that installing it performs.
    /// A manifest with an `installer` has the shape of an `index.json` entry.
    /// A manifest without one registers a package that opens in the browser.
    pub fn register_json_package(&mut self) -> Result<String, String> {
        let text = self.json_input.trim();
        if text.is_empty() {
            return Err(t!("json_empty").into_owned());
        }
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(text);
        let value = parsed.map_err(|e| t!("json_invalid", error = e).into_owned())?;

        let pkg = if value.get("installer").is_some() {
            let package = parse_aviutl2_manifest(text)
                .map_err(|e| t!("json_invalid", error = e).into_owned())?;
            self.json_log = match &package.source {
                PackageSource::Recipe(recipe) => recipe.outline(),
                _ => vec![t!("json_log_browser").into_owned()],
            };
            package
        } else {
            self.json_log = vec![t!("json_log_browser").into_owned()];
            let id = value
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| t!("json_id_required").into_owned())?;
            let name = value
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(id)
                .to_owned();
            let summary = value
                .get("overview")
                .or_else(|| value.get("summary"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned();
            let author = value
                .get("author")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    value
                        .get("developer")
                        .and_then(|d| d.get("name"))
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("Unknown")
                .to_owned();
            let niconi_commons_id = value
                .get("niconiCommonsId")
                .or_else(|| value.get("niconi_commons_id"))
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            let page_url = url::Url::parse("https://github.com").unwrap();
            let mut pkg = Package::simple(id, name, page_url.clone(), PackageSource::Web(page_url));
            pkg.summary = summary;
            pkg.author = author;
            pkg.niconi_commons_id = niconi_commons_id;
            pkg
        };

        let id = pkg.id.clone();
        match &mut self.catalog {
            Catalog::Ready(packages) => {
                if let Some(pos) = packages.iter().position(|p| p.id == id) {
                    packages[pos] = pkg;
                } else {
                    packages.insert(0, pkg);
                }
            }
            _ => {
                self.catalog = Catalog::Ready(vec![pkg]);
            }
        }
        Ok(t!("json_registered", id = id).into_owned())
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
            style::btn_content(style::icons::BACK, t!("back")),
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

    let tabs: Vec<Element<'a, Message>> = tabs()
        .into_iter()
        .map(|(tab, label, glyph)| tab_entry(theme, tab, label, glyph, state.tab == tab))
        .collect();
    let tab_card = m3::card(card::Style::filled(theme), column(tabs).spacing(8))
        .width(Length::Fixed(TAB_LIST_WIDTH));

    let content: Element<'a, Message> = if let Some(pkg_id) = &state.viewing_package_id {
        if let Some(package) = state.package(pkg_id) {
            package_detail_view(theme, state, instance, package)
        } else {
            match state.tab {
                Tab::Version => version_tab(theme, instance),
                Tab::Plugins => plugins_tab(theme, instance),
                Tab::Browse => browse_tab(theme, state, instance),
                Tab::Commons => commons_tab(theme, state, instance),
                Tab::JsonRegister => json_register_tab(theme, state),
                Tab::Settings => settings_tab(theme, state),
            }
        }
    } else {
        match state.tab {
            Tab::Version => version_tab(theme, instance),
            Tab::Plugins => plugins_tab(theme, instance),
            Tab::Browse => browse_tab(theme, state, instance),
            Tab::Commons => commons_tab(theme, state, instance),
            Tab::JsonRegister => json_register_tab(theme, state),
            Tab::Settings => settings_tab(theme, state),
        }
    };
    let content_card = m3::card(card::Style::filled(theme), content)
        .width(Length::Fill)
        .height(Length::Fill);

    let mut body = column![
        header,
        row![tab_card, content_card]
            .spacing(style::GAP)
            .height(Length::Fill)
    ]
    .spacing(style::GAP);

    if let Some(items) = &state.update_preview {
        body = body.push(update_preview_modal(theme, items));
    }

    body.into()
}

fn update_preview_modal<'a>(
    theme: &'a Theme,
    items: &'a [UpdatePreviewItem],
) -> Element<'a, Message> {
    let rows: Vec<Element<'a, Message>> = items
        .iter()
        .map(|(name, cur, next, changelog)| {
            let mut col = column![
                row![
                    style::icon(style::icons::EXTENSION)
                        .size(16)
                        .color(theme.primary()),
                    style::label(theme, name.clone()).size(15),
                    text(format!("{cur} → {next}"))
                        .size(13)
                        .color(theme.tertiary()),
                ]
                .spacing(8)
                .align_y(Vertical::Center),
            ]
            .spacing(4);

            if let Some(log) = changelog {
                col = col.push(text(log).size(12).color(theme.on_surface_variant()));
            }

            m3::card(style::tile_card(theme, false), col)
                .padding(Padding::from([8.0, 12.0]))
                .width(Length::Fill)
                .into()
        })
        .collect();

    m3::card(
        card::Style::filled(theme),
        column![
            row![
                style::icon(style::icons::UPDATE)
                    .size(20)
                    .color(theme.primary()),
                style::label(theme, t!("update_summary")).size(16),
                style::label(theme, t!("items_count", count = items.len()).into_owned()).size(14),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
            scrollable(column(rows).spacing(6)).height(Length::Fixed(200.0)),
            row![
                m3::button(
                    button::Style::filled(theme, Accent::Primary),
                    style::btn_content(style::icons::CHECK, t!("update_now")),
                )
                .on_press(Message::ConfirmUpdateAll),
                m3::button(
                    button::Style::tonal(theme, Accent::Primary),
                    style::btn_content(style::icons::CLOSE, t!("cancel")),
                )
                .on_press(Message::CancelUpdateAll),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
        ]
        .spacing(12)
        .padding(Padding::from([14.0, 18.0])),
    )
    .width(Length::Fill)
    .into()
}

fn tab_entry<'a>(
    theme: &Theme,
    tab: Tab,
    label: Cow<'a, str>,
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
            &t!("edition"),
            &strings::edition_label(config.edition),
        ));
    }
    lines = lines.push(field_with_icon(
        theme,
        style::icons::HISTORY,
        &t!("version"),
        &config.app_version,
    ));
    if let Some(addon) = &config.addon_version {
        lines = lines.push(field_with_icon(
            theme,
            style::icons::EXTENSION,
            &t!("addon_version"),
            addon,
        ));
    }
    lines = lines
        .push(field_with_icon(
            theme,
            style::icons::GROUP,
            &t!("group"),
            config.group.as_deref().unwrap_or(&t!("ungrouped")),
        ))
        .push(field_with_icon(
            theme,
            style::icons::EXTENSION,
            &t!("plugins"),
            &config.plugins.len().to_string(),
        ));
    lines
        .push(
            m3::button(
                button::Style::filled(theme, Accent::Primary),
                style::btn_content(style::icons::UPDATE, t!("update_all")),
            )
            .on_press(Message::RequestUpdateAll),
        )
        .into()
}

fn plugins_tab<'a>(theme: &Theme, instance: &Instance) -> Element<'a, Message> {
    let toolbar = row![
        m3::button(
            button::Style::tonal(theme, Accent::Tertiary),
            style::btn_content(style::icons::SYNC, t!("scan_manual")),
        )
        .on_press(Message::ScanManualPlugins),
        m3::button(
            button::Style::tonal(theme, Accent::Primary),
            style::btn_content(style::icons::EXPORT, t!("export_config")),
        )
        .on_press(Message::ExportConfig),
        m3::button(
            button::Style::tonal(theme, Accent::Secondary),
            style::btn_content(style::icons::IMPORT, t!("import_config")),
        )
        .on_press(Message::ImportConfig),
    ]
    .spacing(8)
    .align_y(Vertical::Center);

    if instance.config.plugins.is_empty() {
        return column![
            toolbar,
            row![
                style::icon(style::icons::EXTENSION)
                    .size(20)
                    .color(theme.on_surface_variant()),
                style::label(theme, t!("no_plugins")).size(14),
            ]
            .spacing(8)
            .align_y(Vertical::Center)
        ]
        .spacing(style::GAP)
        .into();
    }
    let rows: Vec<Element<'a, Message>> = instance
        .config
        .plugins
        .iter()
        .map(|record| {
            let mut badges = row![].spacing(6).align_y(Vertical::Center);
            if record.paused {
                badges = badges.push(style::icon_chip::<Message>(
                    theme,
                    style::icons::PAUSE,
                    t!("paused"),
                    Tone::Secondary,
                ));
            }

            let info = column![
                row![
                    style::icon(style::icons::EXTENSION)
                        .size(16)
                        .color(theme.primary()),
                    style::label(theme, record.id.clone()).size(16),
                    badges,
                ]
                .spacing(8)
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
                            t!("files"),
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

            let pause_btn = if record.paused {
                m3::button(
                    button::Style::tonal(theme, Accent::Secondary),
                    style::btn_content(style::icons::PLAY, t!("resume_update")),
                )
                .on_press(Message::TogglePausePlugin(record.id.clone(), false))
            } else {
                m3::button(
                    button::Style::tonal(theme, Accent::Tertiary),
                    style::btn_content(style::icons::PAUSE, t!("pause_update")),
                )
                .on_press(Message::TogglePausePlugin(record.id.clone(), true))
            };

            let remove = m3::button(
                style::danger_button(theme),
                style::btn_content(style::icons::DELETE, t!("delete")),
            )
            .on_press(Message::Remove(record.id.clone()));

            m3::card(
                style::tile_card(theme, false),
                row![info, pause_btn, remove]
                    .spacing(8)
                    .align_y(Vertical::Center),
            )
            .padding(Padding::from([10.0, 16.0]))
            .width(Length::Fill)
            .into()
        })
        .collect();

    column![
        toolbar,
        scrollable(column(rows).spacing(ROW_GAP))
            .height(Length::Fill)
            .style(style::scroll(theme))
    ]
    .spacing(style::GAP)
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
            style::label(theme, t!("no_catalog")).size(14),
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
                style::label(theme, t!("loading")).size(14),
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
                    style::label(theme, format!("{}: {reason}", t!("load_failed"))).size(12),
                ]
                .spacing(6)
                .align_y(Vertical::Center),
                m3::button(
                    button::Style::tonal(theme, Accent::Primary),
                    style::btn_content(style::icons::REFRESH, t!("reload")),
                )
                .on_press(Message::ReloadCatalog),
            ]
            .spacing(6)
            .into();
        }
        Catalog::Ready(packages) => packages,
    };

    let search = TextInput::new(&t!("search_hint"), &state.search, theme)
        .with_label_text(
            strings::label("search"),
            card::Style::filled(theme).container_color,
        )
        .on_input(Message::SearchChanged)
        .padding(INPUT_PADDING);
    let refresh = m3::button(
        button::Style::tonal(theme, Accent::Primary),
        style::btn_content(style::icons::REFRESH, t!("refresh")),
    )
    .on_press(Message::ReloadCatalog);
    let mut page =
        column![row![search, refresh].spacing(8).align_y(Vertical::Center)].spacing(style::GAP);

    // Filter and sort controls
    page = page.push(sort_and_filter_bar(theme, state));

    let mut categories: Vec<&str> = packages
        .iter()
        .flat_map(|package| package.categories.iter().map(String::as_str))
        .collect();
    categories.sort_unstable();
    categories.dedup();
    if !categories.is_empty() {
        page = page.push(category_bar(theme, &categories, state.category.as_deref()));
    }

    let mut tags: Vec<&str> = packages
        .iter()
        .flat_map(|package| package.tags.iter().map(String::as_str))
        .collect();
    tags.sort_unstable();
    tags.dedup();
    if !tags.is_empty() {
        page = page.push(tag_bar(theme, &tags, &state.tags));
    }

    if let Some(pending) = &state.pending {
        page = page.push(pending_card(theme, pending));
    }

    let needle = state.search.trim().to_lowercase();
    let mut shown: Vec<&Package> = packages
        .iter()
        .filter(|package| !state.hide_deprecated || !package.deprecated)
        .filter(|package| match state.status_filter {
            StatusFilter::All => true,
            StatusFilter::Installed => status_of(instance, package) != Status::NotInstalled,
            StatusFilter::NotInstalled => status_of(instance, package) == Status::NotInstalled,
        })
        .filter(|package| {
            state
                .category
                .as_ref()
                .is_none_or(|category| package.categories.contains(category))
        })
        .filter(|package| state.tags.iter().all(|tag| package.tags.contains(tag)))
        .filter(|package| matches_query(package, &needle))
        .collect();

    sort_packages(&mut shown, state.sort_order, packages, today());

    if shown.is_empty() {
        return page
            .push(
                row![
                    style::icon(style::icons::SEARCH)
                        .size(16)
                        .color(theme.on_surface_variant()),
                    style::label(theme, t!("no_match").into_owned()).size(14),
                ]
                .spacing(8)
                .align_y(Vertical::Center),
            )
            .into();
    }
    page.push(package_list(theme, &shown, instance, state))
        .into()
}

fn sort_and_filter_bar<'a>(theme: &Theme, state: &State) -> Element<'a, Message> {
    let status_btn = |filter: StatusFilter, label: Cow<'static, str>| -> Element<'a, Message> {
        let active = state.status_filter == filter;
        let color = style::tile_content(theme, active);
        m3::card(
            style::tile_card(theme, active),
            text(label).size(12).color(color),
        )
        .interaction(card::Interaction::Press(Action::Direct(
            Message::StatusFilterChanged(filter),
        )))
        .padding(Padding::from([4.0, 10.0]))
        .into()
    };

    let sort_btn = |order: SortOrder, label: Cow<'static, str>| -> Element<'a, Message> {
        let active = state.sort_order == order;
        let color = style::tile_content(theme, active);
        m3::card(
            style::tile_card(theme, active),
            text(label).size(12).color(color),
        )
        .interaction(card::Interaction::Press(Action::Direct(
            Message::SortOrderChanged(order),
        )))
        .padding(Padding::from([4.0, 10.0]))
        .into()
    };

    let status_row = row![
        style::icon(style::icons::FILTER)
            .size(14)
            .color(theme.primary()),
        status_btn(StatusFilter::All, t!("status_all")),
        status_btn(StatusFilter::Installed, t!("status_installed")),
        status_btn(StatusFilter::NotInstalled, t!("status_not_installed")),
    ]
    .spacing(6)
    .align_y(Vertical::Center);

    let sort_row = row![
        style::icon(style::icons::SORT)
            .size(14)
            .color(theme.primary()),
        sort_btn(SortOrder::Popularity, t!("sort_popularity")),
        sort_btn(SortOrder::Trend, t!("sort_trend")),
        sort_btn(SortOrder::Newest, t!("sort_newest")),
        sort_btn(SortOrder::Updated, t!("sort_updated")),
    ]
    .spacing(6)
    .align_y(Vertical::Center);

    let deprecated_toggle = checkbox(state.hide_deprecated)
        .label(t!("hide_deprecated").into_owned())
        .size(14)
        .on_toggle(|_| Message::ToggleHideDeprecated);

    row![status_row, sort_row, deprecated_toggle]
        .spacing(16)
        .align_y(Vertical::Center)
        .into()
}

fn tag_bar<'a>(theme: &Theme, tags: &[&str], selected: &BTreeSet<String>) -> Element<'a, Message> {
    let entry = |label: String, message: Message, selected: bool| -> Element<'a, Message> {
        let color = style::tile_content(theme, selected);
        m3::card(
            style::tile_card(theme, selected),
            row![
                style::icon(style::icons::TAG).size(12).color(if selected {
                    theme.on_primary_container()
                } else {
                    theme.primary()
                }),
                text(label).size(12).color(color),
            ]
            .spacing(4)
            .align_y(Vertical::Center),
        )
        .interaction(card::Interaction::Press(Action::Direct(message)))
        .padding(Padding::from([3.0, 8.0]))
        .into()
    };

    let mut entries = vec![entry(
        format!("#{}", t!("all_categories")),
        Message::TagsCleared,
        selected.is_empty(),
    )];
    entries.extend(tags.iter().map(|tag| {
        entry(
            format!("#{tag}"),
            Message::TagToggled((*tag).to_owned()),
            selected.contains(*tag),
        )
    }));

    scrollable(row(entries).spacing(6))
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .style(style::scroll(theme))
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
        t!("all_categories").into_owned(),
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

/// Why the installation waits for the confirmation of the user.
fn pending_warning(pending: &Pending) -> String {
    if !pending.package.deprecated {
        return t!("unverified_warning").into_owned();
    }
    match &pending.package.deprecation_message {
        Some(message) => t!("deprecated_install_warning_message", message = message).into_owned(),
        None => t!("deprecated_install_warning").into_owned(),
    }
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
                format!("{}: {}", pending.package.name, pending_warning(pending))
            )
            .size(13)
            .width(Length::Fill),
            m3::button(
                button::Style::filled(theme, Accent::Primary),
                style::btn_content(style::icons::CHECK, t!("continue")),
            )
            .on_press(Message::ConfirmPending),
            m3::button(
                button::Style::tonal(theme, Accent::Primary),
                style::btn_content(style::icons::CLOSE, t!("cancel")),
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
        .id(browse_scroll_id())
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
    let mut header_row = row![
        style::icon(style::icons::EXTENSION)
            .size(16)
            .color(theme.primary()),
        style::label(theme, package.name.clone()).size(16),
        style::label(theme, format!("by {}", package.author)).size(12),
    ]
    .spacing(6)
    .align_y(Vertical::Center);

    if package.deprecated {
        header_row = header_row.push(style::icon_chip::<Message>(
            theme,
            style::icons::WARNING,
            t!("deprecated"),
            Tone::Error,
        ));
    }

    if let Some(commons_id) = &package.niconi_commons_id {
        header_row = header_row.push(style::icon_chip::<Message>(
            theme,
            style::icons::SHARE,
            commons_id,
            Tone::Tertiary,
        ));
    }

    let mut info = column![
        header_row,
        style::label(theme, truncated(&package.summary)).size(12),
    ]
    .spacing(2)
    .width(Length::Fill);

    if !package.tags.is_empty() {
        let tag_chips: Vec<Element<'a, Message>> = package
            .tags
            .iter()
            .take(3)
            .map(|t| text(format!("#{t}")).size(11).color(theme.outline()).into())
            .collect();
        info = info.push(row(tag_chips).spacing(6));
    }

    let (status_text, status_icon, status_tone) = match status {
        Status::NotInstalled => (t!("not_installed"), style::icons::DOWNLOAD, Tone::Secondary),
        Status::Installed => (t!("installed"), style::icons::CHECK_CIRCLE, Tone::Tertiary),
        Status::UpdateAvailable => (t!("update_available"), style::icons::UPDATE, Tone::Primary),
    };
    let action = match (&package.source, status) {
        (PackageSource::Web(_), _) => Some((
            style::icons::OPEN_IN_NEW,
            t!("open_in_browser"),
            Message::Open(package.id.clone()),
        )),
        (_, Status::NotInstalled) => Some((
            style::icons::DOWNLOAD,
            t!("install"),
            Message::Install(package.id.clone()),
        )),
        (_, Status::UpdateAvailable) => Some((
            style::icons::UPDATE,
            t!("update"),
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
            style::btn_content(style::icons::CHECK, t!("installed")),
        ),
    };

    let details_btn = m3::button(
        button::Style::tonal(theme, Accent::Secondary),
        style::btn_content(style::icons::DESCRIPTION, t!("details")),
    )
    .on_press(Message::ViewPackageDetails(Some(package.id.clone())));

    let body = row![
        info,
        details_btn,
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

fn commons_tab<'a>(
    theme: &'a Theme,
    state: &'a State,
    instance: &'a Instance,
) -> Element<'a, Message> {
    // Collect all installed plugins that have a niconi_commons_id
    let mut items = Vec::new();
    for record in &instance.config.plugins {
        let Some(pkg) = state.package(&record.id) else {
            continue;
        };
        if let Some(commons_id) = &pkg.niconi_commons_id {
            items.push((record.id.clone(), pkg.name.clone(), commons_id.clone()));
        }
    }

    let guide_btn = m3::button(
        button::Style::tonal(theme, Accent::Primary),
        style::btn_content(style::icons::LINK, t!("commons_guide")),
    )
    .on_press(Message::OpenCommonsGuide);

    if items.is_empty() {
        return column![
            row![guide_btn].align_y(Vertical::Center),
            row![
                style::icon(style::icons::SHARE)
                    .size(20)
                    .color(theme.on_surface_variant()),
                style::label(theme, t!("commons_no_ids")).size(14),
            ]
            .spacing(8)
            .align_y(Vertical::Center)
        ]
        .spacing(style::GAP)
        .into();
    }

    let select_all_btn = m3::button(
        button::Style::tonal(theme, Accent::Secondary),
        style::btn_content(style::icons::CHECK, t!("select_all")),
    )
    .on_press(Message::SelectAllCommons);

    let deselect_all_btn = m3::button(
        button::Style::tonal(theme, Accent::Secondary),
        style::btn_content(style::icons::CLOSE, t!("deselect_all")),
    )
    .on_press(Message::DeselectAllCommons);

    let copy_all_btn = m3::button(
        button::Style::filled(theme, Accent::Primary),
        style::btn_content(style::icons::COPY, t!("commons_copy_all")),
    )
    .on_press(Message::CopySelectedCommons);

    let toolbar = row![copy_all_btn, select_all_btn, deselect_all_btn, guide_btn]
        .spacing(8)
        .align_y(Vertical::Center);

    let rows: Vec<Element<'a, Message>> = items
        .into_iter()
        .map(|(plugin_id, name, commons_id)| {
            let is_selected = state.selected_commons.contains(&commons_id);
            let id_for_chk = commons_id.clone();
            let chk = checkbox(is_selected)
                .on_toggle(move |_| Message::ToggleCommonsSelect(id_for_chk.clone()));

            let info = column![
                row![
                    style::icon(style::icons::EXTENSION)
                        .size(16)
                        .color(theme.primary()),
                    style::label(theme, name).size(15),
                    style::icon_chip::<Message>(
                        theme,
                        style::icons::SHARE,
                        &commons_id,
                        Tone::Tertiary
                    ),
                ]
                .spacing(8)
                .align_y(Vertical::Center),
                style::label(theme, plugin_id).size(12),
            ]
            .spacing(2)
            .width(Length::Fill);

            let copy_btn = m3::button(
                button::Style::tonal(theme, Accent::Primary),
                style::btn_content(style::icons::COPY, t!("copy")),
            )
            .on_press(Message::CopySingleCommons(commons_id.clone()));

            let open_btn = m3::button(
                button::Style::tonal(theme, Accent::Secondary),
                style::btn_content(style::icons::OPEN_IN_NEW, t!("open")),
            )
            .on_press(Message::OpenCommonsUrl(commons_id));

            m3::card(
                style::tile_card(theme, is_selected),
                row![chk, info, copy_btn, open_btn]
                    .spacing(8)
                    .align_y(Vertical::Center),
            )
            .padding(Padding::from([10.0, 16.0]))
            .width(Length::Fill)
            .into()
        })
        .collect();

    column![
        toolbar,
        scrollable(column(rows).spacing(ROW_GAP))
            .height(Length::Fill)
            .style(style::scroll(theme))
    ]
    .spacing(style::GAP)
    .into()
}

fn json_register_tab<'a>(theme: &'a Theme, state: &'a State) -> Element<'a, Message> {
    let background = card::Style::filled(theme).container_color;
    let input = TextInput::new(&t!("json_placeholder"), &state.json_input, theme)
        .with_label_text(strings::label("json_direct_register"), background)
        .on_input(Message::JsonInputChanged)
        .padding(INPUT_PADDING);

    let load_sample = m3::button(
        button::Style::tonal(theme, Accent::Secondary),
        style::btn_content(style::icons::CODE, t!("insert_sample_json")),
    )
    .on_press(Message::LoadSampleJson);

    let submit = m3::button(
        button::Style::filled(theme, Accent::Primary),
        style::btn_content(style::icons::CHECK, t!("register_manifest")),
    )
    .on_press(Message::RegisterJsonPackage);

    let mut body = column![
        row![
            style::icon(style::icons::CODE)
                .size(20)
                .color(theme.primary()),
            style::label(theme, t!("json_direct_register").into_owned()).size(18),
        ]
        .spacing(8)
        .align_y(Vertical::Center),
        style::label(theme, t!("json_register_help").into_owned()).size(13),
        row![load_sample, submit].spacing(8),
        input,
    ]
    .spacing(style::GAP);

    if let Some(status) = &state.json_status {
        match status {
            Ok(msg) => {
                body = body.push(
                    row![
                        style::icon(style::icons::CHECK_CIRCLE)
                            .size(16)
                            .color(theme.primary()),
                        style::label(theme, msg.clone()).size(13),
                    ]
                    .spacing(6),
                );
            }
            Err(err) => {
                body = body.push(
                    row![
                        style::icon(style::icons::ERROR)
                            .size(16)
                            .color(theme.error()),
                        style::label(theme, err.clone()).size(13),
                    ]
                    .spacing(6),
                );
            }
        }
    }

    if !state.json_log.is_empty() {
        body = body.push(
            style::label(theme, t!("json_log_title").into_owned())
                .size(13)
                .color(theme.primary()),
        );
        for line in &state.json_log {
            body = body.push(
                text(line.clone())
                    .size(12)
                    .font(iced::Font::MONOSPACE)
                    .color(theme.on_surface_variant()),
            );
        }
    }

    body.into()
}

fn settings_tab<'a>(theme: &'a Theme, state: &'a State) -> Element<'a, Message> {
    let background = card::Style::filled(theme).container_color;
    let name = TextInput::new("", &state.name, theme)
        .with_label_text(strings::label("name"), background)
        .on_input(Message::NameChanged)
        .padding(INPUT_PADDING);
    let group = TextInput::new(&t!("group_hint"), &state.group, theme)
        .with_label_text(strings::label("group"), background)
        .on_input(Message::GroupChanged)
        .padding(INPUT_PADDING);
    let save = m3::button(
        button::Style::filled(theme, Accent::Primary),
        style::btn_content(style::icons::SAVE, t!("save")),
    )
    .on_press_maybe(state.can_save().then_some(Message::SaveSettings));

    let fields = column![
        row![
            style::icon(style::icons::EDIT)
                .size(16)
                .color(theme.primary()),
            style::label(theme, t!("name").into_owned()).size(14),
        ]
        .spacing(6)
        .align_y(Vertical::Center),
        name,
        row![
            style::icon(style::icons::GROUP)
                .size(16)
                .color(theme.primary()),
            style::label(theme, t!("group").into_owned()).size(14),
        ]
        .spacing(6)
        .align_y(Vertical::Center),
        group,
        save
    ]
    .spacing(style::GAP);

    fields.into()
}

fn package_detail_view<'a>(
    theme: &'a Theme,
    state: &'a State,
    instance: &'a Instance,
    package: &'a Package,
) -> Element<'a, Message> {
    let back_btn = m3::button(
        button::Style::tonal(theme, Accent::Primary),
        style::btn_content(style::icons::BACK, t!("back_to_list")),
    )
    .on_press(Message::ViewPackageDetails(None));

    let status = status_of(instance, package);
    let (status_text, status_icon, status_tone) = match status {
        Status::NotInstalled => (t!("not_installed"), style::icons::DOWNLOAD, Tone::Secondary),
        Status::Installed => (t!("installed"), style::icons::CHECK_CIRCLE, Tone::Tertiary),
        Status::UpdateAvailable => (t!("update_available"), style::icons::UPDATE, Tone::Primary),
    };

    let action = match (&package.source, status) {
        (PackageSource::Web(_), _) => Some((
            style::icons::OPEN_IN_NEW,
            t!("open_in_browser"),
            Message::Open(package.id.clone()),
        )),
        (_, Status::NotInstalled) => Some((
            style::icons::DOWNLOAD,
            t!("install"),
            Message::Install(package.id.clone()),
        )),
        (_, Status::UpdateAvailable) => Some((
            style::icons::UPDATE,
            t!("update"),
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
            style::btn_content(style::icons::CHECK, t!("installed")),
        ),
    };

    let mut header = row![
        back_btn,
        style::label(theme, package.name.clone()).size(22),
        style::icon_chip::<Message>(theme, status_icon, status_text, status_tone),
    ]
    .spacing(12)
    .align_y(Vertical::Center);

    if package.deprecated {
        header = header.push(style::icon_chip::<Message>(
            theme,
            style::icons::WARNING,
            t!("deprecated_warning"),
            Tone::Error,
        ));
    }

    if let Some(commons_id) = &package.niconi_commons_id {
        header = header.push(style::icon_chip::<Message>(
            theme,
            style::icons::SHARE,
            commons_id,
            Tone::Tertiary,
        ));
    }

    header = header.push(act);

    let author_btn = m3::button(
        button::Style::tonal(theme, Accent::Secondary),
        style::btn_content(style::icons::PERSON, &package.author),
    )
    .on_press(Message::SearchChanged(package.author.clone()));

    let meta_bar = row![
        style::icon(style::icons::PERSON)
            .size(16)
            .color(theme.primary()),
        text(format!("{}:", t!("author")))
            .size(13)
            .color(theme.outline()),
        author_btn,
        style::icon(style::icons::LINK)
            .size(16)
            .color(theme.primary()),
        text(format!("ID: {}", package.id))
            .size(13)
            .color(theme.outline()),
    ]
    .spacing(8)
    .align_y(Vertical::Center);

    let mut content = column![header, meta_bar].spacing(12);

    if package.deprecated {
        let message = package
            .deprecation_message
            .clone()
            .unwrap_or_else(|| t!("deprecated_warning").into_owned());
        content = content.push(
            m3::card(
                card::Style {
                    container_color: theme.error_container(),
                    ..card::Style::filled(theme)
                },
                row![
                    style::icon(style::icons::WARNING)
                        .size(20)
                        .color(theme.error()),
                    text(message)
                        .size(13)
                        .color(theme.on_surface())
                        .width(Length::Fill),
                ]
                .spacing(10)
                .padding(Padding::from([10.0, 14.0]))
                .align_y(Vertical::Center),
            )
            .width(Length::Fill),
        );
    }

    // 6-19: Screenshot carousel
    if !package.screenshots.is_empty() {
        let len = package.screenshots.len();
        let idx = state.active_screenshot_idx.min(len - 1);
        let current_url = &package.screenshots[idx];

        let prev_btn = m3::button(
            button::Style::tonal(theme, Accent::Primary),
            style::btn_content(style::icons::BACK, t!("prev")),
        )
        .on_press(Message::PrevScreenshot(len));

        let next_btn = m3::button(
            button::Style::tonal(theme, Accent::Primary),
            style::btn_content(style::icons::LAUNCH, t!("next")),
        )
        .on_press(Message::NextScreenshot(len));

        let picture: Element<'a, Message> = match state.images.get(current_url) {
            Some(ImageState::Ready(handle)) => {
                let height = if state.preview_open {
                    PREVIEW_HEIGHT
                } else {
                    THUMBNAIL_HEIGHT
                };
                mouse_area(
                    image(handle.clone())
                        .width(Length::Fill)
                        .height(Length::Fixed(height))
                        .content_fit(ContentFit::Contain),
                )
                .on_press(Message::TogglePreview)
                .into()
            }
            Some(ImageState::Failed(reason)) => text(format!("{current_url}\n{reason}"))
                .size(13)
                .color(theme.error())
                .into(),
            _ => text(t!("image_loading").into_owned())
                .size(13)
                .color(theme.on_surface_variant())
                .into(),
        };

        let carousel_card = m3::card(
            card::Style::filled(theme),
            column![
                row![
                    style::icon(style::icons::IMAGE)
                        .size(20)
                        .color(theme.primary()),
                    text(t!("screenshot_position", index = idx + 1, total = len).into_owned())
                        .size(14),
                    prev_btn,
                    next_btn,
                ]
                .spacing(12)
                .align_y(Vertical::Center),
                m3::card(
                    style::tile_card(theme, false),
                    container(picture)
                        .padding(Padding::from([8.0, 12.0]))
                        .width(Length::Fill),
                )
                .width(Length::Fill),
            ]
            .spacing(8)
            .padding(Padding::from([10.0, 14.0])),
        )
        .width(Length::Fill);

        content = content.push(carousel_card);
    }

    // 6-20: Markdown overview
    let summary_card = m3::card(
        card::Style::filled(theme),
        column![
            row![
                style::icon(style::icons::DESCRIPTION)
                    .size(18)
                    .color(theme.primary()),
                style::label(theme, t!("package_overview").into_owned()).size(16),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
            crate::screen::markdown::view(theme, &package.summary),
        ]
        .spacing(8)
        .padding(Padding::from([12.0, 16.0])),
    )
    .width(Length::Fill);
    content = content.push(summary_card);

    // 6-21: Dependencies
    if !package.dependencies.is_empty() {
        let dep_chips: Vec<Element<'a, Message>> = package
            .dependencies
            .iter()
            .map(|dep| {
                m3::button(
                    button::Style::tonal(theme, Accent::Tertiary),
                    style::btn_content(style::icons::EXTENSION, dep.as_str()),
                )
                .on_press(Message::ViewPackageDetails(Some(dep.clone())))
                .into()
            })
            .collect();

        let dep_card = m3::card(
            card::Style::filled(theme),
            column![
                row![
                    style::icon(style::icons::LAYERS)
                        .size(18)
                        .color(theme.primary()),
                    style::label(theme, t!("dependencies").into_owned()).size(15),
                ]
                .spacing(8)
                .align_y(Vertical::Center),
                row(dep_chips).spacing(8),
            ]
            .spacing(8)
            .padding(Padding::from([10.0, 14.0])),
        )
        .width(Length::Fill);
        content = content.push(dep_card);
    }

    // 6-23: Licenses
    if !package.licenses.is_empty() {
        let lic_chips: Vec<Element<'a, Message>> = package
            .licenses
            .iter()
            .map(|lic| {
                m3::button(
                    button::Style::tonal(theme, Accent::Secondary),
                    style::btn_content(style::icons::POLICY, lic.as_str()),
                )
                .on_press(Message::ShowLicense(Some(lic.clone())))
                .into()
            })
            .collect();

        let lic_card = m3::card(
            card::Style::filled(theme),
            column![
                row![
                    style::icon(style::icons::POLICY)
                        .size(18)
                        .color(theme.primary()),
                    style::label(theme, t!("license").into_owned()).size(15),
                ]
                .spacing(8)
                .align_y(Vertical::Center),
                row(lic_chips).spacing(8),
            ]
            .spacing(8)
            .padding(Padding::from([10.0, 14.0])),
        )
        .width(Length::Fill);
        content = content.push(lic_card);
    }

    // 6-24: Changelog
    if let Some(changelog) = &package.changelog {
        let changelog_card = m3::card(
            card::Style::filled(theme),
            column![
                row![
                    style::icon(style::icons::HISTORY)
                        .size(18)
                        .color(theme.primary()),
                    style::label(theme, t!("changelog").into_owned()).size(15),
                ]
                .spacing(8)
                .align_y(Vertical::Center),
                scrollable(text(changelog).size(13).color(theme.on_surface_variant()))
                    .height(Length::Fixed(150.0)),
            ]
            .spacing(8)
            .padding(Padding::from([10.0, 14.0])),
        )
        .width(Length::Fill);
        content = content.push(changelog_card);
    }

    // License modal popup if active
    if let Some(lic_text) = &state.license_dialog_content {
        let modal = m3::card(
            card::Style::filled(theme),
            column![
                row![
                    style::icon(style::icons::POLICY)
                        .size(20)
                        .color(theme.primary()),
                    style::label(theme, t!("license_terms").into_owned()).size(16),
                    m3::button(
                        button::Style::tonal(theme, Accent::Primary),
                        style::btn_content(style::icons::CLOSE, t!("close")),
                    )
                    .on_press(Message::ShowLicense(None)),
                ]
                .spacing(12)
                .align_y(Vertical::Center),
                scrollable(text(lic_text).size(13)).height(Length::Fixed(200.0)),
            ]
            .spacing(10)
            .padding(Padding::from([14.0, 18.0])),
        )
        .width(Length::Fill);
        content = content.push(modal);
    }

    scrollable(content)
        .height(Length::Fill)
        .style(style::scroll(theme))
        .into()
}
