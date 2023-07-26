use gtk4::{glib, traits::WidgetExt};

glib::wrapper! {
    pub struct ToolbarWidget(ObjectSubclass<underlying::ToolbarWidget>)
        @extends gtk4::Widget;
}

impl ToolbarWidget {
    pub fn new(parent_editor: &super::EditorWindow, editing_started_with_cropping: bool) -> Self {
        let obj = glib::Object::builder::<Self>()
            .property("parent-editor", parent_editor)
            .property(
                "editing-started-with-cropping",
                editing_started_with_cropping,
            )
            .build();

        // We want to start as hidden if editing started with cropping
        obj.set_visible(!editing_started_with_cropping);

        obj
    }
}

mod underlying {
    use std::cell::Cell;

    use gtk4::{
        glib::{self, clone, ParamSpec, Properties, WeakRef},
        prelude::*,
        subclass::prelude::*,
        Inhibit,
    };
    use kcshot_data::colour::Colour;

    use crate::{
        editor::{
            self, colourchooser, operations::Tool, underlying::EditorWindow as EditorWindowImp,
            utils::CairoExt,
        },
        ext::DisposeExt,
        kcshot::KCShot,
        log_if_err,
    };

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::ToolbarWidget)]
    pub struct ToolbarWidget {
        #[property(get, set, construct_only)]
        parent_editor: WeakRef<editor::EditorWindow>,
        #[property(set, construct_only)]
        editing_started_with_cropping: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ToolbarWidget {
        const NAME: &'static str = "KCShotToolbarWidget";
        type Type = super::ToolbarWidget;
        type ParentType = gtk4::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk4::BoxLayout>();
        }
    }

    impl ObjectImpl for ToolbarWidget {
        fn constructed(&self) {
            let editor = self
                .parent_editor
                .upgrade()
                .expect("self.parent_editor should be set");

            let adjustment = gtk4::Adjustment::new(4.0, 1.0, 1000.0, 0.4, 1.0, 1.0);
            let line_width_spinner = gtk4::SpinButton::new(Some(&adjustment), 0.5, 1);
            line_width_spinner.set_numeric(true);
            line_width_spinner.connect_value_changed(clone!(@weak editor => move |this| {
                editor.set_line_width(this.value());
            }));
            line_width_spinner.set_visible(false);

            let obj: gtk4::Widget = self.obj().to_owned().upcast();
            let group_source_tool = if self.editing_started_with_cropping.get() {
                Tool::Save
            } else {
                Tool::CropAndSave
            };
            let (group_source, _) =
                make_tool_button(group_source_tool, &obj, &editor, None, None, None, None);
            group_source.set_active(!should_start_saving_immediately(group_source_tool));

            let primary_colour_button = Self::make_primary_colour_chooser_button(editor.clone());
            primary_colour_button.set_tooltip_text(Some("Set primary colour"));
            let secondary_colour_button = Self::make_secondary_colour_button(editor.clone());
            secondary_colour_button.set_tooltip_text(Some("Set secondary colour"));

            #[rustfmt::skip]
            let mut buttons = vec![
                make_tool_button(Tool::Pencil, &obj, &editor, Some(&group_source), Some(&line_width_spinner), None, Some(&secondary_colour_button)),
                make_tool_button(Tool::Line, &obj, &editor, Some(&group_source), Some(&line_width_spinner), None, Some(&secondary_colour_button)),
                make_tool_button(Tool::Arrow, &obj, &editor, Some(&group_source), Some(&line_width_spinner), None, Some(&secondary_colour_button)),
                make_tool_button(Tool::Rectangle, &obj, &editor, Some(&group_source), Some(&line_width_spinner), Some(&primary_colour_button), Some(&secondary_colour_button)),
                make_tool_button(Tool::Highlight, &obj, &editor, Some(&group_source), None, None, None),
                make_tool_button(Tool::Ellipse, &obj, &editor, Some(&group_source), Some(&line_width_spinner), Some(&primary_colour_button), Some(&secondary_colour_button)),
                make_tool_button(Tool::Pixelate, &obj, &editor, Some(&group_source), None, None, None),
                make_tool_button(Tool::Blur, &obj, &editor, Some(&group_source), None, None, None),
                make_tool_button(Tool::AutoincrementBubble, &obj, &editor, Some(&group_source), None, Some(&primary_colour_button), Some(&secondary_colour_button)),
                make_tool_button(Tool::Text, &obj, &editor, Some(&group_source), None, None, None),
            ];

            if self.editing_started_with_cropping.get() {
                buttons[0].0.set_active(true);
            }

            primary_colour_button.set_parent(&obj);
            secondary_colour_button.set_parent(&obj);
            line_width_spinner.set_parent(&obj);

            buttons.insert(0, (group_source, group_source_tool));

            let key_event_handler = gtk4::EventControllerKey::new();
            key_event_handler.connect_key_pressed(
                clone!(@weak editor => @default-return Inhibit(false), move |_, key, _, _| {
                    if let Some(tool) = key.to_unicode().and_then(Tool::from_unicode) {
                        editor.set_current_tool(tool);
                        for (button, button_tool) in buttons.iter() {
                            if *button_tool == tool {
                                button.set_active(true);
                                break;
                            }
                        }
                    }
                    Inhibit(false)
                }),
            );

            editor.add_controller(key_event_handler);
        }

        fn dispose(&self) {
            self.obj().dispose_children();
        }

        fn properties() -> &'static [ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &ParamSpec) {
            Self::derived_set_property(self, id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
        }
    }
    impl WidgetImpl for ToolbarWidget {}
    impl BoxImpl for ToolbarWidget {}

    impl ToolbarWidget {
        fn make_primary_colour_chooser_button(editor: editor::EditorWindow) -> gtk4::Button {
            let drawing_area = gtk4::DrawingArea::new();
            drawing_area.set_accessible_role(gtk4::AccessibleRole::Img);
            drawing_area.set_size_request(20, 20);
            drawing_area.set_draw_func(clone!(@weak editor =>  move |_this, cairo, _w, _h| {
                cairo.set_operator(cairo::Operator::Over);

                let primary_colour = editor.primary_colour();
                if primary_colour.alpha != 0 {
                    cairo.rectangle(0.0, 0.0, 20.0, 20.0);
                    cairo.set_source_colour(primary_colour);
                    log_if_err!(cairo.fill());
                } else {
                    // Instead of drawing nothing (what a fully transparent colour is) we draw a
                    // checkerboard pattern instead
                    cairo.set_source_colour(Colour {
                        red: 0xff,
                        green: 0x00,
                        blue: 0xdc,
                        alpha: 0xff
                    });
                    cairo.rectangle(0.0, 0.0, 10.0, 10.0);
                    log_if_err!(cairo.fill());
                    cairo.rectangle(10.0, 10.0, 10.0, 10.0);
                    log_if_err!(cairo.fill());

                    cairo.set_source_colour(Colour::BLACK);
                    cairo.rectangle(0.0, 10.0, 10.0, 10.0);
                    log_if_err!(cairo.fill());
                    cairo.rectangle(10.0, 0.0, 10.0, 10.0);
                    log_if_err!(cairo.fill());
                }

                cairo.set_source_colour(Colour::BLACK);
                cairo.rectangle(0.0, 0.0, 20.0, 20.0);
                cairo.set_line_width(1.0);
                log_if_err!(cairo.stroke());

            }));

            Self::make_button::<true>(drawing_area, editor)
        }

        fn make_secondary_colour_button(editor: editor::EditorWindow) -> gtk4::Button {
            let drawing_area = gtk4::DrawingArea::new();
            drawing_area.set_accessible_role(gtk4::AccessibleRole::Img);
            drawing_area.set_size_request(20, 20);
            drawing_area.set_draw_func(clone!(@weak editor =>  move |_this, cairo, _w, _h| {
                cairo.set_operator(cairo::Operator::Over);

                // The interior contour of the square
                cairo.set_source_colour(Colour::BLACK);
                cairo.rectangle(5.0, 5.0, 10.0, 10.0);
                cairo.set_line_width(1.0);
                log_if_err!(cairo.stroke());

                // The empty square representing the border
                cairo.set_source_colour(editor.secondary_colour());
                cairo.rectangle(3.0, 3.0, 14.0, 14.0);
                cairo.set_line_width(4.0);
                log_if_err!(cairo.stroke());

                // The exterior contour of the square
                cairo.set_source_colour(Colour::BLACK);
                cairo.rectangle(1.0, 1.0, 18.0, 18.0);
                cairo.set_line_width(1.0);
                log_if_err!(cairo.stroke());

            }));

            Self::make_button::<false>(drawing_area, editor)
        }

        fn make_button<const IS_PRIMARY: bool>(
            button_drawing_area: gtk4::DrawingArea,
            editor: editor::EditorWindow,
        ) -> gtk4::Button {
            let button = gtk4::Button::new();
            button.set_child(Some(&button_drawing_area));
            button.set_visible(false);

            button.connect_clicked(move |_this| {
                let dialog = colourchooser::dialog(&editor);

                dialog.connect_response(
                    clone!(@weak button_drawing_area => move |editor, colour| {
                        if IS_PRIMARY {
                            editor.set_primary_colour(colour);
                        } else {
                            editor.set_secondary_colour(colour);
                        }
                        button_drawing_area.queue_draw();
                    }),
                );

                dialog.show();
            });

            button
        }
    }

    fn make_tool_button(
        tool: Tool,
        toolbar: &gtk4::Widget,
        editor: &editor::EditorWindow,
        group_source: Option<&gtk4::ToggleButton>,
        // Should only be passed for buttons that use the line-width-spinner
        spinner: Option<&gtk4::SpinButton>,
        // Should only be passed for buttons that care about primary-colour (i.e. they want to fill a shape)
        primary: Option<&gtk4::Button>,
        // Should only be passed for buttons that care about secondary-colour (i.e. they want to do lines of some form)
        secondary: Option<&gtk4::Button>,
    ) -> (gtk4::ToggleButton, Tool) {
        let button = match group_source {
            Some(group_source) => {
                let button = gtk4::ToggleButton::new();
                button.set_group(Some(group_source));
                button
            }
            None => gtk4::ToggleButton::new(),
        };
        button.set_child(Some(&gtk4::Image::from_resource(tool.path())));
        button.set_tooltip_markup(Some(tool.tooltip()));

        let spinner = spinner.cloned();
        let primary = primary.cloned();
        let secondary = secondary.cloned();
        button.connect_toggled(move |this| {
            if let Some(spinner) = &spinner {
                spinner.set_visible(this.is_active());
            }

            if let Some(primary) = &primary {
                primary.set_visible(this.is_active());
            }

            if let Some(secondary) = &secondary {
                secondary.set_visible(this.is_active());
            }
        });

        button.connect_clicked(clone!(@weak editor => move |_| {
            tracing::info!("Entered on-click handler of {tool:?}");
            editor.set_current_tool(tool);

            if should_start_saving_immediately(tool) {
                editor.imp().with_image_mut(&format!("on_click of {tool:?} - immediate save"), |image| {
                    KCShot::the().with_conn(|conn| EditorWindowImp::do_save_surface(
                        &KCShot::the().model_notifier(),
                        conn,
                        editor.upcast_ref(),
                        image,
                        None
                    ));
                });
            }
        }));
        button.set_active(false);
        button.set_parent(toolbar);
        (button, tool)
    }

    /// This functions returns whether the button calling this needs to immediately start the saving
    /// process on click
    ///
    /// This is applicable only for the "crop-first" mode, as there the "Save" action is logically
    /// the final thing you do, and needing to click somewhere on screen would be weird
    fn should_start_saving_immediately(tool: Tool) -> bool {
        matches!(tool, Tool::Save)
    }
}
