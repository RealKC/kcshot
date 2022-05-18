use gtk4::{
    glib, glib::clone, prelude::*, subclass::prelude::ObjectSubclassIsExt, DialogFlags,
    Orientation, ResponseType,
};

use super::data::{Colour, Text};

pub fn pop_text_dialog_and_get_text(editor: &super::EditorWindow) {
    let dialog = gtk4::Dialog::with_buttons(
        Some("Add text"),
        Some(editor),
        DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
        &[("Cancel", ResponseType::Cancel), ("Ok", ResponseType::Ok)],
    );

    let content = gtk4::Box::new(Orientation::Vertical, 5);
    let entry_followed_by_buttons = gtk4::Box::new(Orientation::Horizontal, 0);

    let text_buffer = gtk4::EntryBuffer::new(Some("Add your text here"));
    let text_editor = gtk4::Entry::with_buffer(&text_buffer);
    entry_followed_by_buttons.prepend(&text_editor);

    let font_button = gtk4::FontButton::new();
    entry_followed_by_buttons.prepend(&font_button);
    let colour_chooser = gtk4::ColorButton::new();
    entry_followed_by_buttons.prepend(&colour_chooser);

    content.prepend(&entry_followed_by_buttons);

    // See https://docs.gtk.org/Pango/pango_markup.html#convenience-tags
    let info_text = gtk4::Label::new(Some(
        r#"You can use HTML-like formatting in the above textbox, the following tags are usable:
- <i>text</i> for italic text
- <b>bold</i> for bold text
- <u>underline</u> for underlined text
- <s>strikethough</s> to strikethrough text
- <sub>subscript</sub> for subscript text
- <sup>superscript</sup> for superscript text
- <tt>monospace</tt> to make your text monospace
    "#,
    ));
    content.prepend(&info_text);

    let dialog_content_area = dialog.content_area();
    dialog_content_area.append(&content);

    dialog.connect_response(clone!(
        @weak editor,
        @weak text_buffer,
        @weak font_button,
        @weak colour_chooser
    => move |this, response| {
        this.close();

        if response != ResponseType::Ok {
            tracing::trace!("Text dialog response wasn't 'Ok', returning...");
            return;
        }

        editor.imp().with_image_mut("text dialog response", |image| {
            let text = Text {
                string: text_buffer.text(),
                font_description: font_button
                    .font_desc()
                    .expect("There should be a font description"),
                colour: Colour::from_gdk_rgba(colour_chooser.rgba()),
            };
            image.operation_stack.set_text(text);
        });
    }));

    dialog.show();
}
