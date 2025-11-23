use gtk4::{glib, subclass::prelude::*};
use kcshot_data::colour::Colour;

use super::EditorWindow;

glib::wrapper! {
    pub struct ColourChooserDialog(ObjectSubclass<underlying::ColourChooserDialog>)
        @extends gtk4::Widget, gtk4::Window,
        @implements gtk4::ConstraintTarget, gtk4::Buildable, gtk4::Accessible,
                    gtk4::ShortcutManager, gtk4::Root, gtk4::Native;
}

impl ColourChooserDialog {
    pub fn new(editor: &EditorWindow, initial_colour: Colour) -> Self {
        glib::Object::builder()
            .property("editor", editor)
            .property("initial-colour", initial_colour)
            .build()
    }

    pub async fn colour(&self) -> Colour {
        let colour_rx = self.imp().colour_rx.borrow_mut().take().unwrap();
        colour_rx.await.unwrap()
    }
}

mod underlying {
    use std::{cell::RefCell, marker::PhantomData};

    use gtk4::{
        CompositeTemplate,
        glib::{self, Properties, WeakRef},
        prelude::*,
        subclass::prelude::*,
    };
    use kcshot_data::colour::Colour;
    use tokio::sync::oneshot;

    use crate::{
        editor::{EditorWindow, colourchooser::ColourChooserWidget},
        ext::DisposeExt,
    };

    #[derive(Debug, CompositeTemplate, Properties)]
    #[template(file = "src/editor/colourchooserdialog.blp")]
    #[properties(wrapper_type = super::ColourChooserDialog)]
    pub struct ColourChooserDialog {
        #[property(get, set)]
        editor: WeakRef<EditorWindow>,
        #[property(name = "initial-colour", set = Self::set_initial_colour, construct_only)]
        colour: PhantomData<Colour>,

        #[template_child]
        colour_chooser: TemplateChild<ColourChooserWidget>,

        pub(super) colour_rx: RefCell<Option<oneshot::Receiver<Colour>>>,
        colour_tx: RefCell<Option<oneshot::Sender<Colour>>>,
    }

    impl Default for ColourChooserDialog {
        fn default() -> Self {
            let (colour_tx, colour_rx) = oneshot::channel();

            Self {
                editor: Default::default(),
                colour: PhantomData,
                colour_chooser: Default::default(),
                colour_rx: RefCell::new(Some(colour_rx)),
                colour_tx: RefCell::new(Some(colour_tx)),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ColourChooserDialog {
        const NAME: &'static str = "KCShotColourChooserDialog";
        type Type = super::ColourChooserDialog;
        type ParentType = gtk4::Window;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("kcshot-colour-chooser-dialog");

            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ColourChooserDialog {
        fn dispose(&self) {
            self.obj().dispose_children();
        }
    }

    impl WidgetImpl for ColourChooserDialog {}
    impl WindowImpl for ColourChooserDialog {}

    impl ColourChooserDialog {
        fn set_initial_colour(&self, colour: Colour) {
            self.colour_chooser.set_colour(colour);
        }
    }

    #[gtk4::template_callbacks]
    impl ColourChooserDialog {
        #[template_callback]
        fn on_cancel_clicked(&self, _: &gtk4::Button) {
            self.obj().close();
        }

        #[template_callback]
        async fn on_colour_picker_clicked(&self, _: &gtk4::Button) {
            if let Some(editor) = self.editor.upgrade() {
                self.obj().hide();
                let colour = editor.pick_colour().await;
                self.colour_chooser.set_colour(colour);
                self.obj().show();
            }
        }

        #[template_callback]
        fn on_ok_clicked(&self, _: &gtk4::Button) {
            self.obj().close();

            let colour_tx = self.colour_tx.borrow_mut().take().unwrap();
            colour_tx.send(self.colour_chooser.colour()).unwrap();
        }
    }
}
