use gtk4::{glib::BorrowedObject, prelude::*};

pub trait DisposeExt {
    /// Disposes all children of a widget
    fn dispose_children(&self);
}

impl<T> DisposeExt for BorrowedObject<'_, T>
where
    T: WidgetExt,
{
    fn dispose_children(&self) {
        while let Some(child) = self.first_child() {
            child.unparent();
        }
    }
}
