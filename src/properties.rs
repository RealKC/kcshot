use gtk4::glib::{self, IsA, ParamSpec, ParamSpecBuilderExt, ParamSpecObject};

pub fn construct_only_wo_object_property<T: IsA<glib::Object>>(name: &str) -> ParamSpec {
    ParamSpecObject::builder::<T>(name)
        .write_only()
        .construct_only()
        .build()
}
