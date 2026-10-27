use std::{
    cell::{Cell, OnceCell, RefCell},
    marker::PhantomData,
};

use cairo::Context;
use diesel::SqliteConnection;
use gtk4::{
    CompositeTemplate,
    glib::{self, Properties, clone},
    prelude::*,
    subclass::prelude::*,
};
use kcshot_data::{
    colour::Colour,
    geometry::{Point, Rectangle},
    settings::Settings,
};
use kcshot_editor::image::Image;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::error;

use crate::{
    editor::toolbar, ext::DisposeExt, history::ModelNotifier, kcshot::KCShot,
    postcapture::run_postcapture_actions,
};

mod accessors;
mod actions;
mod events;

#[derive(Debug, Properties, CompositeTemplate)]
#[properties(wrapper_type = super::EditorWindow)]
#[template(file = "src/editor/editor.blp")]
pub struct EditorWindow {
    #[property(name = "editing-starts-with-cropping", set, construct_only)]
    editing_started_with_cropping: Cell<bool>,
    #[property(name = "primary-colour", get = Self::get_primary_color, set = Self::set_primary_color)]
    /// Colour used for filling in shapes
    primary_colour: PhantomData<Colour>,
    #[property(name = "secondary-colour", get = Self::get_secondary_color, set = Self::set_secondary_color)]
    /// Colour used for lines, bubble text, normal text and pencil
    secondary_colour: PhantomData<Colour>,

    pub(super) image: RefCell<Option<Image>>,

    #[template_child]
    overlay: TemplateChild<gtk4::Overlay>,
    #[template_child]
    pub(super) drawing_area: TemplateChild<gtk4::DrawingArea>,

    toolbar: OnceCell<toolbar::ToolbarWidget>,

    colour_tx: Sender<Colour>,
    colour_rx: RefCell<Receiver<Colour>>,
    colour_requested: Cell<bool>,

    toolbar_position: RefCell<Option<(i32, i32)>>,
    toolbar_offset: RefCell<Option<(i32, i32)>>,

    is_in_with_image_mut: Cell<bool>,
}

impl Default for EditorWindow {
    fn default() -> Self {
        let (colour_tx, colour_rx) = mpsc::channel(8);

        Self {
            editing_started_with_cropping: Default::default(),
            image: Default::default(),
            overlay: Default::default(),
            drawing_area: Default::default(),
            toolbar: Default::default(),
            colour_tx,
            colour_rx: RefCell::new(colour_rx),
            colour_requested: Default::default(),
            is_in_with_image_mut: Default::default(),
            primary_colour: PhantomData,
            secondary_colour: PhantomData,
            toolbar_position: Default::default(),
            toolbar_offset: Default::default(),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for EditorWindow {
    const NAME: &'static str = "KCShotEditorWindow";
    type Type = super::EditorWindow;
    type ParentType = gtk4::ApplicationWindow;

    fn class_init(klass: &mut Self::Class) {
        klass.set_css_name("kcshot-editor-window");

        klass.bind_template();
        klass.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

#[glib::derived_properties]
impl ObjectImpl for EditorWindow {
    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();

        let image = kcshot_screenshot::take_screenshot(KCShot::the().tokio_rt())
            .expect("Couldn't take a screenshot");
        let windows = kcshot_screenshot::get_windows().unwrap_or_else(|why| {
            tracing::info!("Got while trying to retrieve windows: {why}");
            vec![]
        });
        let screen_dimensions = Rectangle {
            x: 0.0,
            y: 0.0,
            w: image.width() as f64,
            h: image.height() as f64,
        };

        let settings = Settings::open();
        self.image.replace(Some(Image::new(
            image,
            windows,
            screen_dimensions,
            self.editing_started_with_cropping.get(),
            settings.last_used_primary_colour(),
            settings.last_used_secondary_colour(),
        )));

        self.overlay.connect_get_child_position({
            let obj = obj.clone();
            move |_, widget| obj.imp().on_get_child_position(widget)
        });

        let toolbar = self.toolbar.get_or_init(|| {
            toolbar::ToolbarWidget::new(
                &obj,
                self.editing_started_with_cropping.get(),
                self.get_primary_color(),
                self.get_secondary_color(),
            )
        });
        self.overlay.add_overlay(toolbar);

        self.drawing_area.set_draw_func(clone!(
            #[weak]
            obj,
            move |_, cairo, _, _| {
                obj.imp()
                    .with_image("draw event", |image| image.draw_in(cairo, false));
            }
        ));

        self.setup_actions();
    }

    fn dispose(&self) {
        self.obj().dispose_children();
        self.with_image_mut("dispose", |image| image.surface.finish());
    }
}

impl EditorWindow {
    #[allow(clippy::await_holding_refcell_ref)]
    pub(super) async fn pick_colour(&self) -> Colour {
        self.colour_requested.set(true);
        self.colour_rx.borrow_mut().recv().await.unwrap()
    }

    pub(super) fn do_save_surface(
        model_notifier: &ModelNotifier,
        conn: &mut SqliteConnection,
        window: &gtk4::Window,
        image: &Image,
        point: Option<Point>,
    ) {
        let cairo = match Context::new(&image.surface) {
            Ok(cairo) => cairo,
            Err(err) => {
                error!("Got error constructing Cairo context inside do_save_surface: {err}");
                return;
            }
        };

        image.draw_in(&cairo, false);

        window.close();

        match image.to_texture(point) {
            // Process all post capture actions
            Ok(texture) => run_postcapture_actions(model_notifier, conn, &texture),
            Err(why) => {
                error!("Failed to get texture from image: {why:?}");
            }
        };
    }
}

impl WidgetImpl for EditorWindow {}
impl WindowImpl for EditorWindow {}
impl ApplicationWindowImpl for EditorWindow {}
