use std::backtrace::Backtrace;

use gtk4::{prelude::*, subclass::prelude::*};
use kcshot_data::settings::Settings;
use kcshot_editor::image::Image;

use crate::editor::{Colour, imp::EditorWindow, utils::ContextLogger};

impl EditorWindow {
    pub fn get_primary_color(&self) -> Colour {
        self.with_image("get primary_colour", |image| {
            image.operation_stack.primary_colour
        })
        .unwrap()
    }

    pub fn set_primary_color(&self, colour: Colour) {
        tracing::info!("primary");
        self.with_image_mut("set_primary_colour", |image| {
            image.operation_stack.primary_colour = colour;
        });

        let settings = Settings::open();
        if let Err(why) = settings.try_set_last_used_primary_colour(colour) {
            tracing::warn!("Failed to update `last-used-primary-colour` setting value: {why}");
        }
        self.obj().notify("primary-colour");
    }

    pub fn get_secondary_color(&self) -> Colour {
        self.with_image("get secondary_colour", |image| {
            image.operation_stack.secondary_colour
        })
        .unwrap()
    }

    pub fn set_secondary_color(&self, colour: Colour) {
        self.with_image_mut("set_secondary_colour", |image| {
            image.operation_stack.secondary_colour = colour;
        });

        let settings = Settings::open();
        if let Err(why) = settings.try_set_last_used_secondary_colour(colour) {
            tracing::warn!("Failed to update `last-used-secondary-colour` setting value: {why}");
        }
        self.obj().notify("secondary-colour");
    }

    pub fn is_moving_toolbar(&self) -> bool {
        self.toolbar_offset.borrow().is_some()
    }

    pub fn set_toolbar_position(&self, x: i32, y: i32) {
        *self.toolbar_position.borrow_mut() = Some((x, y));
    }

    pub fn toolbar_position(&self) -> (i32, i32) {
        self.toolbar_position.borrow().unwrap()
    }

    pub fn set_toolbar_offset(&self, off_x: i32, off_y: i32) {
        *self.toolbar_offset.borrow_mut() = Some((off_x, off_y));
    }

    pub fn finish_toolbar_move(&self, x: i32, y: i32) {
        self.set_toolbar_position(x, y);
        *self.toolbar_offset.borrow_mut() = None;
    }

    pub fn with_image<F, T>(&self, ctx: &str, func: F) -> Option<T>
    where
        F: FnOnce(&Image) -> T,
    {
        let _ctx = ContextLogger::new(ctx, "with_image");

        if self.is_in_with_image_mut.get() {
            tracing::error!(
                "with_image called inside with_image_mut, likely a bug...\n{}",
                Backtrace::capture()
            );
            return None;
        }

        match self.image.try_borrow() {
            Ok(image) => {
                if let Some(image) = image.as_ref() {
                    return Some(func(image));
                }
            }
            Err(why) => {
                if ctx.is_empty() {
                    tracing::info!(
                        "Failed to immutably borrow self.image:\n\t- error: '{why}' ({why:?})"
                    );
                } else {
                    tracing::info!(
                        "Failed to immutably borrow self.image:\n\t- error: '{why}' ({why:?})\n\t- context: {ctx}"
                    );
                }
            }
        }

        None
    }

    pub fn with_image_mut<F, T>(&self, ctx: &str, func: F) -> Option<T>
    where
        F: FnOnce(&mut Image) -> T,
    {
        let _ctx = ContextLogger::new(ctx, "with_image_mut");

        if self.is_in_with_image_mut.get() {
            tracing::error!(
                "with_image_mut called inside with_image_mut, likely a bug...\n{}",
                Backtrace::capture()
            );
            return None;
        }

        self.is_in_with_image_mut.set(true);

        match self.image.try_borrow_mut() {
            Ok(mut image) => {
                if let Some(image) = image.as_mut() {
                    self.is_in_with_image_mut.set(false);
                    return Some(func(image));
                }
            }
            Err(why) => {
                if ctx.is_empty() {
                    tracing::info!(
                        "Failed to mutably borrow self.image:\n\t- error: '{why}' ({why:?})"
                    );
                } else {
                    tracing::info!(
                        "Failed to borrow mutably self.image:\n\t- error: '{why}' ({why:?})\n\t- context: {ctx}"
                    );
                }
            }
        }
        self.is_in_with_image_mut.set(false);

        None
    }
}
