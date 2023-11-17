use gtk4::{glib, pango, prelude::*, subclass::prelude::ObjectSubclassIsExt};
use kcshot_data::colour::Colour;

use super::parse;
use crate::editor::EditorWindow;

glib::wrapper! {
    pub struct TextInput(ObjectSubclass<underlying::TextInput>)
        @extends gtk4::Widget;
}

impl TextInput {
    pub fn new(editor: &EditorWindow) -> Self {
        glib::Object::builder().property("editor", editor).build()
    }

    pub(super) fn text(&self) -> String {
        let input = self.imp().input_view.get();
        let buffer = input.buffer();

        let markdown = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string();

        parse::markdown2pango(&markdown)
    }

    #[track_caller]
    pub(super) fn colour(&self) -> Colour {
        self.editor().unwrap().secondary_colour()
    }

    #[track_caller]
    pub(super) fn font_description(&self) -> pango::FontDescription {
        self.imp()
            .font_button
            .get()
            .font_desc()
            .expect("There should be a font description")
    }
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
        editor::{
            colourchooserdialog::ColourChooserDialog, textdialog::TextDialog, utils::CairoExt,
            Colour, EditorWindow,
        },
        ext::DisposeExt,
        log_if_err,
    };

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::TextInput)]
    #[template(file = "src/editor/textdialog/text_input.blp")]
    pub struct TextInput {
        #[property(get, set)]
        editor: WeakRef<EditorWindow>,
        #[property(get, set)]
        parent_dialog: WeakRef<TextDialog>,

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
        type ParentType = gtk4::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
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
            self.obj().dispose_children();
        }
    }
    impl WidgetImpl for TextInput {}

    #[gtk4::template_callbacks]
    impl TextInput {
        #[template_callback]
        async fn on_colour_button_clicked(&self, _colour_button: &gtk4::Button) {
            let editor = self.obj().editor().unwrap();

            let dialog = ColourChooserDialog::new(&editor);
            dialog.set_transient_for(self.parent_dialog.upgrade().as_ref());
            dialog.show();
            editor.set_secondary_colour(dialog.colour().await);

            self.colour_button_drawing_area.queue_draw();
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
