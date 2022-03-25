use gtk4::glib;

glib::wrapper! {
    pub struct ToolbarWidget(ObjectSubclass<underlying::ToolbarWidget>)
        @extends gtk4::Widget, gtk4::Box;
}

impl ToolbarWidget {
    pub fn new(parent_editor: &super::EditorWindow) -> Self {
        glib::Object::new(&[("parent-editor", parent_editor)])
            .expect("Failed to make a ToolbarWidget")
    }
}

mod underlying {
    use gtk4::{
        gdk::Key,
        glib::{self, clone, ParamSpec, ParamSpecObject},
        prelude::*,
        subclass::prelude::*,
        Inhibit, ResponseType,
    };
    use once_cell::sync::{Lazy, OnceCell};

    use crate::{
        editor::{
            self,
            data::Colour,
            display_server,
            operations::{SelectionMode, Tool},
            utils::CairoExt,
        },
        log_if_err,
    };

    #[derive(Debug, Default)]
    pub struct ToolbarWidget {
        parent_editor: OnceCell<editor::EditorWindow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ToolbarWidget {
        const NAME: &'static str = "kcshotToolbarWidget";
        type Type = super::ToolbarWidget;
        type ParentType = gtk4::Box;
    }

    impl ObjectImpl for ToolbarWidget {
        fn constructed(&self, obj: &Self::Type) {
            let editor = self
                .parent_editor
                .get()
                .expect("self.parent_editor should be set");

            let adjustment = gtk4::Adjustment::new(4.0, 1.0, 1000.0, 0.4, 1.0, 1.0);
            let line_width_spinner = gtk4::SpinButton::new(Some(&adjustment), 0.5, 1);
            line_width_spinner.set_numeric(true);
            line_width_spinner.connect_value_changed(clone!(@strong editor => move |this| {
                editor.set_line_width(this.value());
            }));

            let box_ = obj.upcast_ref();
            let (group_source, _) =
                make_tool_button(Tool::CropAndSave, box_, editor, None, None, None, None);
            group_source.set_active(true);

            let primary_colour_button =
                Self::make_primary_colour_chooser_button(editor, editor.upcast_ref());
            primary_colour_button.set_tooltip_text(Some("Set primary colour"));
            let secondary_colour_button =
                Self::make_secondary_colour_button(editor, editor.upcast_ref());
            secondary_colour_button.set_tooltip_text(Some("Set secondary colour"));

            #[rustfmt::skip]
            let mut buttons = vec![
                make_tool_button(Tool::Pencil, box_, editor, Some(&group_source), Some(&line_width_spinner), Some(&primary_colour_button), Some(&secondary_colour_button)),
                make_tool_button(Tool::Line, box_, editor, Some(&group_source), Some(&line_width_spinner), Some(&primary_colour_button), Some(&secondary_colour_button)),
                make_tool_button(Tool::Arrow, box_, editor, Some(&group_source), Some(&line_width_spinner), Some(&primary_colour_button), Some(&secondary_colour_button)),
                make_tool_button(Tool::Rectangle, box_, editor, Some(&group_source), Some(&line_width_spinner), Some(&primary_colour_button), Some(&secondary_colour_button)),
                make_tool_button(Tool::Highlight, box_, editor, Some(&group_source), None, None, None),
                make_tool_button(Tool::Ellipse, box_, editor, Some(&group_source), Some(&line_width_spinner), Some(&primary_colour_button), Some(&secondary_colour_button)),
                make_tool_button(Tool::Pixelate, box_, editor, Some(&group_source), None, None, None),
                make_tool_button(Tool::Blur, box_, editor, Some(&group_source), None, None, None),
                make_tool_button(Tool::AutoincrementBubble, box_, editor, Some(&group_source), None, Some(&primary_colour_button), Some(&secondary_colour_button)),
                make_tool_button(Tool::Text, box_, editor, Some(&group_source), None, None, Some(&secondary_colour_button)),
            ];

            obj.append(&primary_colour_button);
            obj.append(&secondary_colour_button);
            obj.append(&line_width_spinner);

            // Don't bother with the dropdown if the displa
            if display_server::can_retrieve_windows() {
                let drop_down = if display_server::can_retrieve_window_decorations() {
                    gtk4::DropDown::from_strings(SelectionMode::DECORATIONS)
                } else {
                    gtk4::DropDown::from_strings(SelectionMode::NO_DECORATIONS)
                };
                drop_down.set_tooltip_text(Some("Selection mode"));
                drop_down.connect_selected_item_notify(clone!(@weak editor => move |this| {
                    if let Some(selection_mode) = SelectionMode::from_integer(
                        this.selected(),
                        display_server::can_retrieve_window_decorations(),
                    ) {
                        editor.set_selection_mode(selection_mode);
                    }
                }));

                obj.append(&drop_down);
            }

            buttons.insert(0, (group_source, Tool::CropAndSave));

            let key_event_handler = gtk4::EventControllerKey::new();
            key_event_handler.connect_key_pressed(
                clone!(@weak editor => @default-return Inhibit(false), move |_this, key, _, _| {
                    if key == Key::Escape {
                        editor.close();
                    } else if let Some(tool) = key.to_unicode().and_then(Tool::from_unicode) {
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

            editor.add_controller(&key_event_handler);
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![ParamSpecObject::new(
                    "parent-editor",
                    "ParentEditor",
                    "Parent Editor",
                    editor::EditorWindow::static_type(),
                    glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                )]
            });

            PROPERTIES.as_ref()
        }

        #[tracing::instrument]
        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &ParamSpec,
        ) {
            match pspec.name() {
                "parent-editor" => {
                    let parent_editor = value.get::<editor::EditorWindow>().unwrap();
                    self.parent_editor
                        .set(parent_editor)
                        .expect("parent-editor should only be set once")
                }
                name => tracing::warn!("Unknown property: {name}"),
            }
        }
    }
    impl WidgetImpl for ToolbarWidget {}
    impl BoxImpl for ToolbarWidget {}

    impl ToolbarWidget {
        fn make_primary_colour_chooser_button(
            editor: &editor::EditorWindow,
            parent_window: &gtk4::Window,
        ) -> gtk4::Button {
            let drawing_area = gtk4::DrawingArea::new();
            drawing_area.set_accessible_role(gtk4::AccessibleRole::Img);
            drawing_area.set_size_request(20, 20);
            drawing_area.set_draw_func(clone!(@strong editor =>  move |_this, cairo, _w, _h| {
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

            Self::make_button::<true>(&drawing_area, parent_window, editor)
        }

        fn make_secondary_colour_button(
            editor: &editor::EditorWindow,
            parent_window: &gtk4::Window,
        ) -> gtk4::Button {
            let drawing_area = gtk4::DrawingArea::new();
            drawing_area.set_accessible_role(gtk4::AccessibleRole::Img);
            drawing_area.set_size_request(20, 20);
            drawing_area.set_draw_func(clone!(@strong editor =>  move |_this, cairo, _w, _h| {
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

                // The exterior countour of the square
                cairo.set_source_colour(Colour::BLACK);
                cairo.rectangle(1.0, 1.0, 18.0, 18.0);
                cairo.set_line_width(1.0);
                log_if_err!(cairo.stroke());

            }));

            Self::make_button::<false>(&drawing_area, parent_window, editor)
        }

        fn make_button<const IS_PRIMARY: bool>(
            drawing_area: &gtk4::DrawingArea,
            parent_window: &gtk4::Window,
            editor: &editor::EditorWindow,
        ) -> gtk4::Button {
            let button = gtk4::Button::new();
            button.set_child(Some(drawing_area));

            button.connect_clicked(clone!(@strong parent_window, @strong editor, @strong drawing_area => move |_this| {
                let colour_chooser = gtk4::ColorChooserDialog::new(Some("Pick a colour"), Some(&parent_window));

                colour_chooser.connect_response(clone!(@strong editor, @strong drawing_area => move |this, response| {
                    if response == ResponseType::Ok {
                        if IS_PRIMARY {
                            editor.set_primary_colour(Colour::from_gdk_rgba(this.rgba()));
                        } else {
                            editor.set_secondary_colour(Colour::from_gdk_rgba(this.rgba()));
                        }
                        drawing_area.queue_draw();
                    }

                    this.close();
                }));

                colour_chooser.show();
            }));

            button
        }
    }

    fn make_tool_button(
        tool: Tool,
        toolbar: &gtk4::Box,
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

        button.connect_clicked(clone!(@strong editor => move |_| {
            tracing::info!("Entered on-click handler of {tool:?}");
            editor.set_current_tool(tool);
        }));
        button.set_active(false);
        toolbar.append(&button);
        (button, tool)
    }
}
