use gtk4::{
    glib, glib::clone, pango, prelude::*, subclass::prelude::ObjectSubclassIsExt, DialogFlags,
    ResponseType,
};
use kcshot_data::{colour::Colour, Text};

mod parse;

glib::wrapper! {
    pub struct TextInput(ObjectSubclass<underlying::TextInput>)
        @extends gtk4::Widget, gtk4::Box;
}

impl TextInput {
    pub fn new(editor: &super::EditorWindow) -> Self {
        glib::Object::builder().property("editor", editor).build()
    }

    fn text(&self) -> String {
        let input = self.imp().input.get().unwrap();
        let buffer = input.buffer();

        let markdown = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string();

        parse::markdown2pango(&markdown)
    }

    fn colour(&self) -> Colour {
        self.imp().editor.get().unwrap().secondary_colour()
    }

    fn font_description(&self) -> pango::FontDescription {
        self.imp()
            .font_button
            .get()
            .unwrap()
            .font_desc()
            .expect("There should be a font description")
    }
}

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

mod underlying {
    use gtk4::{glib, prelude::*, subclass::prelude::*};
    use once_cell::{sync::Lazy, unsync::OnceCell};

    use super::parse;
    use crate::{
        editor::{colourchooser, utils::CairoExt, Colour, EditorWindow},
        log_if_err,
    };

    #[derive(Debug, Default)]
    pub struct TextInput {
        content: OnceCell<gtk4::Box>,

        pub(super) editor: OnceCell<EditorWindow>,
        pub(super) font_button: OnceCell<gtk4::FontButton>,
        pub(super) input: OnceCell<gtk4::TextView>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TextInput {
        const NAME: &'static str = "KCShotTextInput";
        type Type = super::TextInput;
        type ParentType = gtk4::Box;
    }

    impl ObjectImpl for TextInput {
        fn constructed(&self) {
            self.parent_constructed();

            let content = self
                .content
                .get_or_init(|| gtk4::Box::new(gtk4::Orientation::Vertical, 2));

            let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
            let font_button = place_format_buttons(content, &self.obj());
            self.font_button.set(font_button).unwrap();
            self.input.set(make_text_view(&hbox)).unwrap();
            content.append(&hbox);

            let info_label = make_info_label();
            content.append(&info_label);

            self.obj().append(content);
        }

        fn dispose(&self) {
            if let Some(content) = self.content.get() {
                content.unparent();
            }
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                use crate::properties::*;
                vec![construct_only_wo_object_property::<EditorWindow>("editor")]
            });

            PROPERTIES.as_ref()
        }

        #[tracing::instrument]
        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "editor" => self.editor.set(value.get().unwrap()).unwrap(),
                property => tracing::error!("Unknown property: {property}"),
            }
        }
    }

    fn place_format_buttons(vbox: &gtk4::Box, text_input: &super::TextInput) -> gtk4::FontButton {
        let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        let font_button = gtk4::FontButton::new();
        font_button.set_margin_bottom(5);
        let colour_button = make_colour_chooser_button(text_input);
        colour_button.set_margin_bottom(5);

        hbox.append(&colour_button);
        hbox.append(&font_button);
        hbox.set_margin_start(10);
        hbox.set_margin_top(10);

        vbox.append(&hbox);

        font_button
    }

    fn make_info_label() -> gtk4::Label {
        let label = gtk4::Label::new(None);
        label.set_markup(r#"You can use CommonMark Markdown or <a href="https://docs.gtk.org/Pango/pango_markup.html" title="Pango markup">Pango markup</a> to format your text."#);
        label.set_margin_bottom(10);
        label
    }

    fn make_text_view(hbox: &gtk4::Box) -> gtk4::TextView {
        let input_view_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        input_view_box.append(&gtk4::Label::new(Some("Input")));

        let input_view = gtk4::TextView::new();
        input_view.set_margin_top(7);
        input_view.set_margin_start(10);
        input_view.set_margin_bottom(5);
        input_view.set_size_request(250, 250);
        input_view.set_wrap_mode(gtk4::WrapMode::Word);
        input_view_box.append(&input_view);
        hbox.append(&input_view_box);

        let preview_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        preview_box.append(&gtk4::Label::new(Some("Preview")));

        let preview = gtk4::TextView::new();
        preview.set_editable(false);
        preview.set_margin_top(7);
        preview.set_margin_end(10);
        preview.set_margin_bottom(5);
        preview.set_size_request(250, 250);
        preview.set_wrap_mode(gtk4::WrapMode::Word);
        preview_box.append(&preview);
        hbox.append(&preview_box);

        input_view
            .buffer()
            .connect_changed(glib::clone!(@weak preview => move |this| {
                let text = this.text(&this.start_iter(), &this.end_iter(), true);

                let markup = parse::markdown2pango(&text);

                preview.buffer().set_text("");
                preview.buffer().insert_markup(&mut preview.buffer().start_iter(), &markup);
            }));

        input_view
    }

    impl BoxImpl for TextInput {}
    impl WidgetImpl for TextInput {}

    fn make_colour_chooser_button(text_input: &super::TextInput) -> gtk4::Button {
        const SIZEI: i32 = 25;
        const SIZEF: f64 = 25.0;

        let drawing_area = gtk4::DrawingArea::new();
        drawing_area.set_accessible_role(gtk4::AccessibleRole::Img);
        drawing_area.set_size_request(SIZEI, SIZEI);
        drawing_area.set_draw_func(
            glib::clone!(@weak text_input => move |_this, cairo, _w, _h| {
                cairo.set_operator(cairo::Operator::Over);

                let editor = text_input.imp().editor.get().unwrap();

                let secondary_colour = editor.secondary_colour();
                if secondary_colour.alpha != 0 {
                    cairo.rectangle(0.0, 0.0, SIZEF, SIZEF);
                    cairo.set_source_colour(secondary_colour);
                    log_if_err!(cairo.fill());
                } else {
                    // Instead of drawing nothing (what a fully transparent colour is) we draw a
                    // checkerboard pattern instead
                    cairo.set_source_colour(Colour {
                        red: 0xff,
                        green: 0x00,
                        blue: 0xdc,
                        alpha: 0xff,
                    });
                    cairo.rectangle(0.0, 0.0, SIZEF / 2.0, SIZEF / 2.0);
                    log_if_err!(cairo.fill());
                    cairo.rectangle(SIZEF / 2.0,SIZEF / 2.0,SIZEF / 2.0, SIZEF / 2.0);
                    log_if_err!(cairo.fill());

                    cairo.set_source_colour(Colour::BLACK);
                    cairo.rectangle(0.0, SIZEF / 2.0,SIZEF / 2.0,SIZEF / 2.0);
                    log_if_err!(cairo.fill());
                    cairo.rectangle(SIZEF / 2.0, 0.0, SIZEF / 2.0,SIZEF / 2.0);
                    log_if_err!(cairo.fill());
                }

                cairo.set_source_colour(Colour::BLACK);
                cairo.rectangle(0.0, 0.0, SIZEF, SIZEF);
                cairo.set_line_width(1.0);
                log_if_err!(cairo.stroke());
            }),
        );

        let button = gtk4::Button::new();
        button.set_child(Some(&drawing_area));
        button.set_size_request(SIZEI, SIZEI);

        button.connect_clicked(
            glib::clone!(@weak text_input => @default-panic, move |_this| {
                let editor = text_input.imp().editor.get().unwrap();
                let dialog = colourchooser::dialog(editor);

                dialog.connect_response(glib::clone!(@weak drawing_area => move |editor, colour| {
                    editor.set_secondary_colour(colour);
                    drawing_area.queue_draw();
                }));

                dialog.show();
            }),
        );

        button
    }
}
