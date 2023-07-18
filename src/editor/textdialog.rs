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
        let input = self.imp().input_view.get();
        let buffer = input.buffer();

        let markdown = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string();

        parse::markdown2pango(&markdown)
    }

    #[track_caller]
    fn colour(&self) -> Colour {
        self.editor().unwrap().secondary_colour()
    }

    #[track_caller]
    fn font_description(&self) -> pango::FontDescription {
        self.imp()
            .font_button
            .get()
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
    use gtk4::{
        glib::{self, Properties, WeakRef},
        prelude::*,
        subclass::prelude::*,
        CompositeTemplate,
    };

    use super::parse;
    use crate::{
        editor::{colourchooser, utils::CairoExt, Colour, EditorWindow},
        log_if_err,
    };

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::TextInput)]
    #[template(file = "src/editor/textdialog.blp")]
    pub struct TextInput {
        #[property(get, set, construct_only)]
        editor: WeakRef<EditorWindow>,

        #[template_child]
        colour_button_drawing_area: TemplateChild<gtk4::DrawingArea>,
        #[template_child]
        pub(super) font_button: TemplateChild<gtk4::FontButton>,
        #[template_child]
        pub(super) input_view: TemplateChild<gtk4::TextView>,
        #[template_child]
        preview: TemplateChild<gtk4::TextView>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TextInput {
        const NAME: &'static str = "KCShotTextInput";
        type Type = super::TextInput;
        type ParentType = gtk4::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for TextInput {
        fn constructed(&self) {
            /// Size of the DrawingArea, must be kept in sync with the blueprint file
            const SIZE: f64 = 25.0;

            self.parent_constructed();

            let text_input = self.obj();

            self.colour_button_drawing_area.set_draw_func(
                glib::clone!(@weak text_input => move |_this, cairo, _w, _h| {
                    cairo.set_operator(cairo::Operator::Over);

                    let editor = text_input.editor().unwrap();

                    let secondary_colour = editor.secondary_colour();
                    if secondary_colour.alpha != 0 {
                        cairo.rectangle(0.0, 0.0, SIZE, SIZE);
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
                        cairo.rectangle(0.0, 0.0, SIZE / 2.0, SIZE / 2.0);
                        log_if_err!(cairo.fill());
                        cairo.rectangle(SIZE / 2.0,SIZE / 2.0,SIZE / 2.0, SIZE / 2.0);
                        log_if_err!(cairo.fill());

                        cairo.set_source_colour(Colour::BLACK);
                        cairo.rectangle(0.0, SIZE / 2.0,SIZE / 2.0,SIZE / 2.0);
                        log_if_err!(cairo.fill());
                        cairo.rectangle(SIZE / 2.0, 0.0, SIZE / 2.0,SIZE / 2.0);
                        log_if_err!(cairo.fill());
                    }

                    cairo.set_source_colour(Colour::BLACK);
                    cairo.rectangle(0.0, 0.0, SIZE, SIZE);
                    cairo.set_line_width(1.0);
                    log_if_err!(cairo.stroke());
                }),
            );
        }

        fn dispose(&self) {
            self.obj().first_child().unwrap().unparent();
        }

        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            Self::derived_set_property(self, id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
        }
    }
    impl BoxImpl for TextInput {}
    impl WidgetImpl for TextInput {}

    #[gtk4::template_callbacks]
    impl TextInput {
        #[template_callback]
        fn on_colour_button_clicked(&self, _colour_button: &gtk4::Button) {
            let editor = self.obj().editor().unwrap();
            let dialog = colourchooser::dialog(&editor);
            let drawing_area = self.colour_button_drawing_area.get();

            dialog.connect_response(glib::clone!(@weak drawing_area => move |editor, colour| {
                editor.set_secondary_colour(colour);
                drawing_area.queue_draw();
            }));

            dialog.show();
        }

        #[template_callback]
        fn on_input_textbuffer_changed(&self, text_buffer: &gtk4::TextBuffer) {
            let text = text_buffer.text(&text_buffer.start_iter(), &text_buffer.end_iter(), true);
            let preview = self.preview.get();

            let markup = parse::markdown2pango(&text);

            preview.buffer().set_text("");
            preview
                .buffer()
                .insert_markup(&mut preview.buffer().start_iter(), &markup);
        }
    }
}
