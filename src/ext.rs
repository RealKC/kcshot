use gtk4::{glib::BorrowedObject, traits::WidgetExt};

pub trait DisposeExt {
    /// Disposes all children of a widget
    fn dispose_children(&self);
}

impl<'a, T> DisposeExt for BorrowedObject<'a, T>
where
    T: WidgetExt,
{
    fn dispose_children(&self) {
        while let Some(child) = self.first_child() {
            child.unparent();
        }
    }
}
