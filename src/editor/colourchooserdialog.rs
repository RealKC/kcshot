use gtk4::{glib, subclass::prelude::*};
use kcshot_data::colour::Colour;

use super::EditorWindow;

glib::wrapper! {
    pub struct ColourChooserDialog(ObjectSubclass<underlying::ColourChooserDialog>)
        @extends gtk4::Widget, gtk4::Window;
}

impl ColourChooserDialog {
    pub fn new(editor: &EditorWindow) -> Self {
        glib::Object::builder().property("editor", editor).build()
    }

    pub async fn colour(&self) -> Colour {
        let colour_rx = self.imp().colour_rx.borrow_mut().take().unwrap();
        colour_rx.await.unwrap()
    }
}

mod underlying {
    use std::cell::RefCell;

    use gtk4::{
        glib::{self, Properties, WeakRef},
        prelude::*,
        subclass::prelude::*,
        CompositeTemplate,
    };
    use kcshot_data::colour::Colour;
    use tokio::sync::oneshot;

    use crate::{
        editor::{colourchooser::ColourChooserWidget, EditorWindow},
        ext::DisposeExt,
    };

    #[derive(Debug, CompositeTemplate, Properties)]
    #[template(file = "src/editor/colourchooserdialog.blp")]
    #[properties(wrapper_type = super::ColourChooserDialog)]
    pub struct ColourChooserDialog {
        #[property(get, set)]
        editor: WeakRef<EditorWindow>,

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

    #[gtk4::template_callbacks]
    impl ColourChooserDialog {
        #[template_callback]
        fn on_cancel_clicked(&self, _: &gtk4::Button) {
            self.obj().close();
        }

        #[template_callback]
        fn on_colour_picker_clicked(&self, _: &gtk4::Button) {
            if let Some(editor) = self.editor.upgrade() {
                self.obj().hide();

                // This branch is part of the mechanism that handles picking a colour from the image.
                // The actual retrieving a colour part is implemented directly in the editor's click
                // event handler, which checks the `is_picking_a_colour` field on the impl struct of
                // EditorWindow.
                // Once the colour is picked, the receive end of the channel will receive the colour
                // of the pixel the user clicked on, set the colour_chooser's colour to that, and show
                // the dialog again, as such eventually one of the other two branches of this `if` will
                // be reached.

                let (colour_tx, colour_rx) = glib::MainContext::channel(glib::Priority::DEFAULT);

                editor.start_picking_a_colour(colour_tx);

                let this = self.obj();
                colour_rx.attach(
                    None,
                    glib::clone!(
                        @weak this
                    => @default-return glib::ControlFlow::Break, move |colour| {
                        this.imp().colour_chooser.set_colour(colour);
                        this.show();
                        glib::ControlFlow::Break
                    }),
                );
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
