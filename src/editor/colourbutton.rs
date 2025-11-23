use gtk4::glib;

glib::wrapper! {
    pub struct ColourButton(ObjectSubclass<underlying::ColourButton>)
        @extends gtk4::Widget, gtk4::Button,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Actionable;
}

mod underlying {
    use std::cell::Cell;

    use gtk4::{
        gdk::RGBA,
        glib::{self, Properties},
        graphene::Rect,
        prelude::*,
        subclass::prelude::*,
    };
    use kcshot_data::colour::Colour;

    use crate::ext::DisposeExt;

    #[derive(Debug, Properties)]
    #[properties(wrapper_type = super::ColourButton)]
    pub struct ColourButton {
        #[property(get, set)]
        colour: Cell<Colour>,
    }

    impl Default for ColourButton {
        fn default() -> Self {
            Self {
                colour: Cell::new(Colour {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 0,
                }),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ColourButton {
        const NAME: &'static str = "KCShotColourButton";
        type Type = super::ColourButton;
        type ParentType = gtk4::Button;
    }

    #[glib::derived_properties]
    impl ObjectImpl for ColourButton {
        fn constructed(&self) {
            self.obj().set_halign(gtk4::Align::Center);
            self.obj().set_valign(gtk4::Align::Center);
        }

        fn dispose(&self) {
            self.obj().dispose_children();
        }
    }

    impl WidgetImpl for ColourButton {
        fn snapshot(&self, snapshot: &gtk4::Snapshot) {
            let colour = self.colour.get();

            let w = self.obj().width() as f32;
            let h = self.obj().height() as f32;

            if colour.alpha != 0 {
                snapshot.append_color(&colour.into(), &Rect::new(0.0, 0.0, w, h));
            } else {
                let black = RGBA::new(0.0, 0.0, 0.0, 1.0);
                let magenta = RGBA::new(1.0, 0.0, 0.8, 1.0);
                snapshot.append_color(&black, &Rect::new(0.0, 0.0, w / 2.0, h / 2.0));
                snapshot.append_color(&magenta, &Rect::new(w / 2.0, 0.0, w / 2.0, h / 2.0));
                snapshot.append_color(&magenta, &Rect::new(0.0, h / 2.0, w / 2.0, h / 2.0));
                snapshot.append_color(&black, &Rect::new(w / 2.0, h / 2.0, w / 2.0, h / 2.0));
            }
        }
    }

    impl ButtonImpl for ColourButton {}
}
