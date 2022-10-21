use gtk4::glib::{ParamSpec, ParamSpecBuilderExt, ParamSpecObject, StaticType};

pub fn construct_only_wo_object_property<T: StaticType>(name: &str) -> ParamSpec {
    ParamSpecObject::builder::<T>(name)
        .write_only()
        .construct_only()
        .build()
}

pub fn construct_only_rw_object_property<T: StaticType>(name: &str) -> ParamSpec {
    ParamSpecObject::builder::<T>(name)
        .readwrite()
        .construct_only()
        .build()
}
