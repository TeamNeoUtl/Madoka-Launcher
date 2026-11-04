use rust_i18n::t;
use iced::widget::{button, container, text};
use iced::{Element, Length, Theme};
use iced_aw::menu::{self, Item, Menu, MenuBar};

use crate::strings;
use crate::style;

/// The message of this menu is the selected theme.
type Ui = Element<'static, Theme>;

const MENU_WIDTH: f32 = 200.0;
const LABEL_PADDING: [u16; 2] = [6, 12];

pub fn view(current: &Theme) -> Ui {
    let current_name = current.to_string();
    let families: Vec<_> = style::theme_groups()
        .into_iter()
        .map(|(family, themes)| {
            let entries: Vec<_> = themes
                .into_iter()
                .map(|theme| {
                    let selected = theme.to_string() == current_name;
                    Item::new(
                        button(text(theme.to_string()))
                            .width(Length::Fill)
                            .padding(LABEL_PADDING)
                            .style(style::menu_item(selected))
                            .on_press(theme),
                    )
                })
                .collect();
            Item::with_menu(
                container(text(format!("{family}  ›")))
                    .width(Length::Fill)
                    .padding(LABEL_PADDING),
                Menu::new(entries).width(Length::Fixed(MENU_WIDTH)),
            )
        })
        .collect();
    let theme_menu = Item::with_menu(
        container(text(format!("{}  ›", t!("menu_theme"))))
            .width(Length::Fill)
            .padding(LABEL_PADDING),
        Menu::new(families).width(Length::Fixed(MENU_WIDTH)),
    );
    let settings = Item::with_menu(
        container(text(t!("menu_settings"))).padding(LABEL_PADDING),
        Menu::new(vec![theme_menu]).width(Length::Fixed(MENU_WIDTH)),
    );
    MenuBar::new(vec![settings])
        .draw_path(menu::DrawPath::Backdrop)
        .into()
}
