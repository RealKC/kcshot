use std::{cell::RefCell, rc::Rc};

use gtk4::{glib, glib::clone, prelude::*, DialogFlags, Orientation, ResponseType};

use super::data::{Colour, Text};

#[derive(Debug)]
pub enum DialogResponse {
    Cancel,
    Text(Text),
}

pub fn pop_text_dialog_and_get_text(parent: &gtk4::Window) -> DialogResponse {
    let dialog = gtk4::Dialog::with_buttons(
        Some("Add text"),
        Some(parent),
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

    let nested_loop = glib::MainLoop::new(None, false);

    let response = Rc::new(RefCell::new(ResponseType::Cancel));
    dialog.connect_response(
        clone!(@strong nested_loop, @strong response => move |_this, dialog_response| {
            response.replace(dialog_response);
            shutdown_loop(&nested_loop);
        }),
    );

    dialog.connect_unmap(clone!(@strong nested_loop => move |_this| shutdown_loop(&nested_loop)));
    dialog.show();

    // FIXME/NOTE/LOOK INTO IT: gtk4 removed Dialog::run() as it was deemed an inappropriate method
    //      in the context of gtk's event based model. And we just replicate its behaviour using
    //      nested main loops here.
    //      We should figure out if there is a better way to do this without blocking, or if the
    //      gtk team's concern is purely ideological.
    nested_loop.run();

    let response = response.borrow();
    let response = match *response {
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

fn shutdown_loop(nested_loop: &glib::MainLoop) {
    if nested_loop.is_running() {
        nested_loop.quit();
    }
}
