use super::data::{Colour, Text};

use gtk::{prelude::*, DialogFlags, Orientation, ResponseType};

#[derive(Debug)]
pub enum DialogResponse {
    Cancel,
    Text(Text),
}

pub fn pop_text_dialog_and_get_text(parent: &gtk::Window) -> DialogResponse {
    let dialog = gtk::Dialog::with_buttons(
        Some("Add text"),
        Some(parent),
        DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
        &[("Cancel", ResponseType::Cancel), ("Ok", ResponseType::Ok)],
    );

    let content = gtk::Box::new(Orientation::Vertical, 5);
    let entry_followed_by_buttons = gtk::Box::new(Orientation::Horizontal, 0);

    let text_buffer = gtk::EntryBuffer::new(Some("Add your text here"));
    let text_editor = gtk::Entry::with_buffer(&text_buffer);
    entry_followed_by_buttons.pack_start(&text_editor, false, true, 0);

    let font_button = gtk::FontButton::new();
    entry_followed_by_buttons.pack_start(&font_button, false, true, 0);
    let colour_chooser = gtk::ColorButton::new();
    entry_followed_by_buttons.pack_start(&colour_chooser, false, true, 0);

    content.pack_start(&entry_followed_by_buttons, false, true, 0);

    // See https://docs.gtk.org/Pango/pango_markup.html#convenience-tags
    let info_text = gtk::Label::new(Some(
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
    content.pack_start(&info_text, false, true, 5);

    let dialog_content_area = dialog.content_area();
    dialog_content_area.add(&content);

    dialog.show_all();
    let response = dialog.run();
    let response = match response {
        ResponseType::Ok => DialogResponse::Text(Text {
            string: text_buffer.text(),
            font_description: font_button
                .font_desc()
                .expect("There should be a font description"),
            colour: Colour::from_gdk_rgba(colour_chooser.rgba()),
        }),
        _ => DialogResponse::Cancel,
    };
    dialog.hide();

    response
}
