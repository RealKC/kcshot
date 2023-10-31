use gtk4::{
    glib, glib::clone, prelude::*, subclass::prelude::ObjectSubclassIsExt, DialogFlags,
    ResponseType,
};
use kcshot_data::Text;

use text_input::*;

mod parse;
mod text_input;

pub fn pop_text_dialog_and_get_text(editor: &super::EditorWindow) {
    let dialog = gtk4::Dialog::with_buttons(
        Some("Add text"),
        Some(editor),
        DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
        &[],
    );

    let cancel_button = dialog.add_button("Cancel", ResponseType::Cancel);
    cancel_button.add_css_class("destructive-action");
    cancel_button.set_margin_bottom(10);

    let ok_button = dialog.add_button("OK", ResponseType::Ok);
    ok_button.add_css_class("suggested-action");
    ok_button.set_margin_bottom(10);
    ok_button.set_margin_start(5);
    ok_button.set_margin_end(10);

    let text_input = TextInput::new(editor);
    dialog.content_area().append(&text_input);

    dialog.connect_response(clone!(
        @weak text_input,
        @weak editor
    => move |this, response| {
        this.close();

        if response != ResponseType::Ok {
            tracing::trace!("Text dialog response wasn't 'Ok', returning...");
            return;
        }

        // NOTE: We create the `Text` outside the `with_image_mut` closure because `TextInput::colour`
        //       calls `with_image`, which will fail inside `with_image_mut`
        let text = Text {
            string: text_input.text(),
            font_description: text_input.font_description(),
            colour: text_input.colour(),
        };
        editor.imp().with_image_mut("text dialog response", |image| {
            image.operation_stack.set_text(text);
            image.operation_stack.finish_current_operation();
        });
    }));

    dialog.show();
}
