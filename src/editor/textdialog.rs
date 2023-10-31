use gtk4::glib;

mod parse;
mod text_input;

glib::wrapper! {
    pub struct TextDialog(ObjectSubclass<underlying::TextDialog>)
        @extends gtk4::Widget, gtk4::Window;
}

impl TextDialog {
    pub fn new(editor: &super::EditorWindow) -> Self {
        glib::Object::builder().property("editor", editor).build()
    }
}

mod underlying {
    use gtk4::{
        glib::{self, Properties, WeakRef},
        prelude::*,
        subclass::prelude::*,
        CompositeTemplate,
    };
    use kcshot_data::Text;

    use crate::{editor::EditorWindow, ext::DisposeExt};

    use super::text_input::TextInput;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::TextDialog)]
    #[template(file = "src/editor/textdialog.blp")]
    pub struct TextDialog {
        #[property(get, set, construct_only)]
        editor: WeakRef<EditorWindow>,

        #[template_child]
        text_input: TemplateChild<TextInput>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TextDialog {
        const NAME: &'static str = "KCShotTextDialog";
        type Type = super::TextDialog;
        type ParentType = gtk4::Window;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for TextDialog {
        fn dispose(&self) {
            self.obj().dispose_children();
        }
    }

    impl WidgetImpl for TextDialog {}
    impl WindowImpl for TextDialog {}

    #[gtk4::template_callbacks]
    impl TextDialog {
        #[template_callback]
        fn on_cancel_clicked(&self, _: &gtk4::Button) {
            self.obj().close();
        }

        #[template_callback]
        fn on_ok_clicked(&self, _: &gtk4::Button) {
            self.obj().close();

            // NOTE: We create the `Text` outside the `with_image_mut` closure because `TextInput::colour`
            //       calls `with_image`, which will fail inside `with_image_mut`
            let text = Text {
                string: self.text_input.text(),
                font_description: self.text_input.font_description(),
                colour: self.text_input.colour(),
            };

            if let Some(editor) = self.editor.upgrade() {
                editor
                    .imp()
                    .with_image_mut("text dialog response", |image| {
                        image.operation_stack.set_text(text);
                        image.operation_stack.finish_current_operation();
                    });
            } else {
                tracing::warn!("Failed to upgrade editor weak ref to strong ref. Did this TextDialog get OK'ed after its parent died?");
            }
        }
    }
}
