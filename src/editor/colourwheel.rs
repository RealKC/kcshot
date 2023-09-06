use std::f32::consts::PI;

use graphene::{Point3D, Rect, Triangle};
use gtk4::{gdk, glib, graphene, gsk, prelude::*, subclass::prelude::*};
use kcshot_data::colour::Hsv;

// Code adapted from Trinket https://gitlab.gnome.org/msandova/trinket/-/blob/8af25267188adde3f7a79cd481ea69d865dd0767/src/color_wheel.rs
// The code is licensed under LGPL-3.0-or-later to Maximiliano Sandoval, a full copy of the license
// can be found at $PROJECT_ROOT/3rdparty-licenses/Trinket.AGPL3.0
// A number of changes have been done to update the widget to newer gtk4-rs and better integrate with
// kcshot code.

#[derive(Debug, Copy, Clone)]
enum Drag {
    Ring(f32, f32),
    Triangle(f32, f32),
    None,
}

glib::wrapper! {
    pub struct ColourWheel(ObjectSubclass<underlying::ColourWheel>)
        @extends gtk4::Widget;
}

impl Default for ColourWheel {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl ColourWheel {
    pub fn hsv(&self) -> Hsv {
        self.imp().hsv.get()
    }
    pub fn rgba(&self) -> gdk::RGBA {
        self.imp().hsv.get().into()
    }

    fn snapshot_ring(&self, snapshot: &gtk4::Snapshot) {
        let imp = self.imp();
        let width = self.size();
        let height = self.size();

        if let Some(texture) = imp.ring_texture.get() {
            texture.snapshot(snapshot, width as f64, height as f64);
        } else {
            let mut bytes: Vec<u8> = vec![];
            let center_x = width / 2;
            let center_y = height / 2;

            let outer = width / 2;
            let inner = outer - self.ring_diameter();

            for y in 0..height {
                let dy = -(y - center_y);
                for x in 0..width {
                    let dx = x - center_x;

                    let dist = dx * dx + dy * dy;
                    if dist < inner * inner || dist > outer * outer {
                        bytes.extend_from_slice(&[0, 0, 0, 0]);
                        continue;
                    }

                    let mut angle = (dy as f32).atan2(dx as f32);
                    if angle < 0.0 {
                        angle += 2.0 * PI;
                    }
                    let h = angle / (2.0 * PI);
                    let rgb = gdk::RGBA::from(Hsv { h, s: 1.0, v: 1.0 });

                    let pixel = [
                        (rgb.red() * 255.0) as u8,
                        (rgb.green() * 255.0) as u8,
                        (rgb.blue() * 255.0) as u8,
                        255_u8,
                    ];
                    bytes.extend_from_slice(&pixel);
                }
            }

            let gbytes = glib::Bytes::from_owned(bytes);
            let format = gdk::MemoryFormat::R8g8b8a8;
            let stride = (width * 4) as usize;
            let texture = gdk::MemoryTexture::new(width, height, format, &gbytes, stride);

            texture.snapshot(snapshot, width as f64, height as f64);

            imp.ring_texture.set(texture).unwrap();
        }
    }

    // FIXME: Borked.
    fn snapshot_triangle_indicator(&self, snapshot: &gtk4::Snapshot) {
        let imp = self.imp();

        let triangle = self.triangle();
        let hsv = imp.hsv.get();
        let (h_vert, s_vert, v_vert) = triangle.points();

        let (h, s, v) = (hsv.h, hsv.s, hsv.v);

        let inverse_h = match h + 0.5 <= 1.0 {
            true => h + 0.5,
            false => h - 0.5,
        };

        let u = 1.0 - s;
        let v = 1.0 - v;

        let a = h_vert.to_vec3().scale(1.0 - u - v);
        let b = s_vert.to_vec3().scale(u);
        let c = v_vert.to_vec3().scale(v);

        let p = a.add(&b).add(&c);

        let radius = 5.0;
        let rect = Rect::new(p.x() - radius, p.y() - radius, 2.0 * radius, 2.0 * radius);
        let round = gsk::RoundedRect::from_rect(rect, radius);

        let inverse_color = gdk::RGBA::from(Hsv {
            h: inverse_h,
            v: s,
            s: v,
        });

        snapshot.push_rounded_clip(&round);
        snapshot.append_color(&inverse_color, &rect);
        snapshot.pop();
    }

    fn snapshot_ring_indicator(&self, snapshot: &gtk4::Snapshot) {
        let imp = self.imp();
        let h = imp.hsv.get().h;

        let angle = h * 360.0;

        let size = self.size() as f32;

        let center_x = size / 2.0;
        let center_y = size / 2.0;

        let outer = size / 2.0;
        let inner = outer - self.ring_diameter() as f32;

        let rect = Rect::new(inner, 0.0, self.ring_diameter() as f32, 2.0);

        let inverse_h = match h + 0.5 <= 1.0 {
            true => h + 0.5,
            false => h - 0.5,
        };
        let inverse_color = gdk::RGBA::from(Hsv {
            h: inverse_h,
            s: 1.0,
            v: 1.0,
        });
        let center = graphene::Point::new(center_x, center_y);
        let minus_center = graphene::Point::new(-center_x, -center_y);

        snapshot.translate(&center);
        snapshot.rotate(360.0 - angle);

        snapshot.append_color(&inverse_color, &rect);

        snapshot.rotate(angle);
        snapshot.translate(&minus_center);
    }

    fn snapshot_triangle(&self, snapshot: &gtk4::Snapshot) {
        let mut bytes: Vec<u8> = vec![];
        let width = self.size();
        let height = self.size();

        for y in 0..height {
            for x in 0..width {
                if !self.is_in_triangle(x as f32, y as f32) {
                    bytes.extend_from_slice(&[0, 0, 0, 0]);
                    continue;
                }

                let hsv = self.hsv_from_triangle_pos(x as f32, y as f32);
                let rgb = gdk::RGBA::from(hsv);

                let pixel = [
                    (rgb.red() * 255.0) as u8,
                    (rgb.green() * 255.0) as u8,
                    (rgb.blue() * 255.0) as u8,
                    255_u8,
                ];
                bytes.extend_from_slice(&pixel);
            }
        }

        let gbytes = glib::Bytes::from_owned(bytes);
        let format = gdk::MemoryFormat::R8g8b8a8;
        let stride = (width * 4) as usize;
        let texture = gdk::MemoryTexture::new(width, height, format, &gbytes, stride);

        texture.snapshot(snapshot, width as f64, height as f64);
    }

    fn size(&self) -> i32 {
        256
    }

    fn ring_diameter(&self) -> i32 {
        24
    }

    fn is_in_ring(&self, x: f32, y: f32) -> bool {
        let size = self.size() as f32;
        let center_x = size / 2.0;
        let center_y = size / 2.0;

        let outer = size / 2.0;
        let inner = outer - self.ring_diameter() as f32;

        let dx = x - center_x;
        let dy = y - center_y;
        let dist = dx * dx + dy * dy;

        dist >= inner * inner && dist <= outer * outer
    }

    fn is_in_triangle(&self, x: f32, y: f32) -> bool {
        let triangle = self.triangle();
        let point = Point3D::new(x, y, 0.0);

        triangle.contains_point(&point)
    }

    // FIXME: Borked.
    fn hsv_from_triangle_pos(&self, x: f32, y: f32) -> Hsv {
        let imp = self.imp();
        let triangle = self.triangle();
        let point = Point3D::new(x, y, 0.0);

        let uv = triangle.barycoords(Some(&point)).unwrap();

        // Inverted for some reason.
        let u = uv.y();
        let v = uv.x();

        let h = imp.hsv.get().h;

        let s = 1.0 - u;
        let v = 1.0 - v;

        Hsv { h, s, v }
    }

    fn h_from_ring_pos(&self, x: f32, y: f32) -> f32 {
        let width = self.size() as f32;
        let height = self.size() as f32;

        let center_x = width / 2.0;
        let center_y = height / 2.0;

        let dy = -(y - center_y);
        let dx = x - center_x;

        if (dx - dy).abs() <= f32::EPSILON {
            return 0.0;
        }

        let mut angle = dy.atan2(dx);
        if angle < 0.0 {
            angle += 2.0 * PI;
        }

        angle / (2.0 * PI)
    }

    fn triangle(&self) -> Triangle {
        let size = self.size() as f32;
        let center_x = size / 2.0;
        let center_y = size / 2.0;

        let outer = size / 2.0;
        let inner = outer - self.ring_diameter() as f32;

        let hx = center_x + inner;
        let hy = center_y;
        let sx = center_x + (2.0 * PI / 3.0).cos() * inner;
        let sy = center_y - (2.0 * PI / 3.0).sin() * inner;
        let vx = center_x + (4.0 * PI / 3.0).cos() * inner;
        let vy = center_y - (4.0 * PI / 3.0).sin() * inner;

        let h = Point3D::new(hx, hy, 0.0);
        let s = Point3D::new(sx, sy, 0.0);
        let v = Point3D::new(vx, vy, 0.0);

        Triangle::from_point3d(Some(&h), Some(&s), Some(&v))
    }

    pub(super) fn notify_all_colour_properties(&self) {
        for prop in underlying::ColourWheel::properties() {
            self.notify(prop.name());
        }
    }
}

mod underlying {
    use std::cell::{Cell, OnceCell};

    use once_cell::sync::Lazy;

    use super::*;
    use crate::ext::DisposeExt;

    #[derive(Debug)]
    pub struct ColourWheel {
        pub(super) hsv: Cell<Hsv>,
        pub(super) ring_texture: OnceCell<gdk::MemoryTexture>,

        state: Cell<Drag>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ColourWheel {
        const NAME: &'static str = "TriColorWheel";
        type Type = super::ColourWheel;
        type ParentType = gtk4::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk4::BinLayout>();
        }

        fn new() -> Self {
            Self {
                hsv: Cell::new(Hsv {
                    h: 0.0,
                    s: 1.0,
                    v: 1.0,
                }),
                state: Cell::new(Drag::None),
                ring_texture: Default::default(),
            }
        }
    }

    impl ObjectImpl for ColourWheel {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecInt::builder("h")
                        .minimum(1)
                        .maximum(360)
                        .default_value(1)
                        .build(),
                    glib::ParamSpecInt::builder("s")
                        .minimum(1)
                        .maximum(100)
                        .default_value(100)
                        .build(),
                    glib::ParamSpecInt::builder("v")
                        .minimum(1)
                        .maximum(100)
                        .default_value(100)
                        .build(),
                    glib::ParamSpecInt::builder("r")
                        .minimum(0)
                        .maximum(255)
                        .default_value(100)
                        .build(),
                    glib::ParamSpecInt::builder("g")
                        .minimum(0)
                        .maximum(255)
                        .default_value(100)
                        .build(),
                    glib::ParamSpecInt::builder("b")
                        .minimum(0)
                        .maximum(255)
                        .default_value(100)
                        .build(),
                    glib::ParamSpecBoxed::builder::<gdk::RGBA>("rgba").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        #[tracing::instrument]
        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "h" => self.hsv.get().as_int().0.to_value(),
                "s" => self.hsv.get().as_int().1.to_value(),
                "v" => self.hsv.get().as_int().2.to_value(),
                "r" => self.hsv.get().to_colour().red.to_value(),
                "g" => self.hsv.get().to_colour().green.to_value(),
                "b" => self.hsv.get().to_colour().blue.to_value(),
                "rgba" => gdk::RGBA::from(self.hsv.get()).to_value(),
                property => {
                    tracing::error!("Unknown property: {property}");
                    panic!()
                }
            }
        }

        #[tracing::instrument]
        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();

            match pspec.name() {
                "h" => {
                    let (_, s, v) = self.hsv.get().as_int();
                    let h = value.get().unwrap();
                    self.hsv.set(Hsv::from_int(h, s, v));
                    obj.notify_all_colour_properties();
                    obj.queue_draw();
                }
                "s" => {
                    let (h, _, v) = self.hsv.get().as_int();
                    let s = value.get().unwrap();
                    self.hsv.set(Hsv::from_int(h, s, v));
                    obj.notify_all_colour_properties();
                    obj.queue_draw();
                }
                "v" => {
                    let (h, s, _) = self.hsv.get().as_int();
                    let v = value.get().unwrap();
                    self.hsv.set(Hsv::from_int(h, s, v));
                    obj.notify_all_colour_properties();
                    obj.queue_draw();
                }
                "r" => {
                    let mut rgba: gdk::RGBA = self.hsv.get().into();
                    let r = value.get::<i32>().unwrap() as f32;
                    rgba.set_red(r / 255.0);
                    self.hsv.set(Hsv::from(rgba));
                    obj.notify_all_colour_properties();
                    obj.queue_draw();
                }
                "g" => {
                    let mut rgba: gdk::RGBA = self.hsv.get().into();
                    let g = value.get::<i32>().unwrap() as f32;
                    rgba.set_green(g / 255.0);
                    self.hsv.set(Hsv::from(rgba));
                    obj.notify_all_colour_properties();
                    obj.queue_draw();
                }
                "b" => {
                    let mut rgba: gdk::RGBA = self.hsv.get().into();
                    let b = value.get::<i32>().unwrap() as f32;
                    rgba.set_blue(b / 255.0);
                    self.hsv.set(Hsv::from(rgba));
                    obj.notify_all_colour_properties();
                    obj.queue_draw();
                }
                "rgba" => {
                    let color = value.get::<gdk::RGBA>().unwrap();

                    self.hsv.set(Hsv::from(color));
                    obj.notify_all_colour_properties();
                    obj.queue_draw();
                }
                property => tracing::error!("Unknown property: {property}"),
            };
        }
        fn constructed(&self) {
            self.parent_constructed();

            let click = gtk4::GestureClick::new();
            click.set_button(0);

            let obj = self.obj();
            click.connect_pressed(glib::clone!(@weak obj => move |_, _, x, y| {
                let (x, y) = (x as f32, y as f32);
                if obj.is_in_ring(x , y) {
                    let imp = obj.imp();

                    let mut hsv = imp.hsv.get();
                    hsv.h = obj.h_from_ring_pos(x, y);

                    obj.set_property("rgba", gdk::RGBA::from(hsv));
                    return;
                }

                if obj.is_in_triangle(x,y) {
                    let hsv = obj.hsv_from_triangle_pos(x,y);
                    obj.set_property("rgba", gdk::RGBA::from(hsv));
                }
            }));

            obj.add_controller(click);

            let drag = gtk4::GestureDrag::new();

            drag.connect_drag_begin(glib::clone!(@weak obj => move |_, x, y| {
                let (x, y) = (x as f32, y as f32);
                if obj.is_in_ring(x , y) {
                    let imp = obj.imp();
                    imp.state.set(Drag::Ring(x,y));

                    return;
                }

                if obj.is_in_triangle(x,y) {
                    let imp = obj.imp();
                    imp.state.set(Drag::Triangle(x,y));
                }
            }));

            drag.connect_drag_end(glib::clone!(@weak obj => move |_, _x, _y| {
                let imp = obj.imp();
                imp.state.set(Drag::None);
            }));

            drag.connect_drag_update(glib::clone!(@weak obj => move |_, x, y| {
                let (x, y) = (x as f32, y as f32);
                let imp = obj.imp();
                let state = imp.state.get();

                match state {
                    Drag::Ring(x_0,y_0) => {
                        let mut hsv = imp.hsv.get();
                        hsv.h = obj.h_from_ring_pos(x + x_0, y + y_0);

                        obj.set_property("rgba", gdk::RGBA::from(hsv));
                    },
                    Drag::Triangle(x_0, y_0) => {
                        if !obj.is_in_triangle(x + x_0, y + y_0) {
                            return;
                        }

                        let hsv = obj.hsv_from_triangle_pos(x + x_0, y + y_0);
                        obj.set_property("rgba", gdk::RGBA::from(hsv));
                    },
                    Drag::None => (),
                }
            }));

            obj.add_controller(drag);
        }

        fn dispose(&self) {
            self.obj().dispose_children();
        }
    }
    impl WidgetImpl for ColourWheel {
        fn snapshot(&self, snapshot: &gtk4::Snapshot) {
            self.parent_snapshot(snapshot);

            let widget = self.obj();

            widget.snapshot_triangle(snapshot);
            widget.snapshot_triangle_indicator(snapshot);

            widget.snapshot_ring(snapshot);
            widget.snapshot_ring_indicator(snapshot);
        }
    }
}
