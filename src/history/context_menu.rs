use gtk4::{gdk, gio, prelude::*};

use super::RowData;

/// Creates a context menu with the usual operations you'd expect from a context menu on a history entry
pub fn context_menu(data: RowData, parent: &gtk4::Widget) -> gtk4::PopoverMenu {
    let builder = gtk4::Builder::from_resource("/kc/kcshot/ui/history/context_menu.ui");
    let model = builder
        .object::<gio::MenuModel>("history_context_menu")
        .unwrap();

    let menu = gtk4::PopoverMenu::builder()
        .menu_model(&model)
        .autohide(true)
        .has_arrow(false)
        .halign(gtk4::Align::Start)
        .build();
    menu.set_parent(parent);

    let actions = gio::SimpleActionGroup::new();
    menu.insert_action_group("ctx_menu", Some(&actions));

    let copy_path = gio::SimpleAction::new("copy_path", None);
    let path = data.path();
    copy_path.connect_activate({
        let menu = menu.clone();
        move |_, _| {
            menu.popdown();
            if let Some(path) = &path {
                clipboard().set_text(path);
            }
        }
    });
    actions.add_action(&copy_path);

    let copy_image = gio::SimpleAction::new("copy_image", None);
    let path = data.path();
    copy_image.connect_activate({
        let menu = menu.clone();
        move |_, _| {
            menu.popdown();
            if let Some(path) = &path {
                clipboard()
                    .set_texture(&gdk::Texture::from_file(&gio::File::for_path(path)).unwrap());
            }
        }
    });
    actions.add_action(&copy_image);

    menu
}

#[track_caller]
fn clipboard() -> gdk::Clipboard {
    gdk::Display::default().unwrap().clipboard()
}
