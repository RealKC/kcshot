/// This enum is like [Operations] but without any associated data
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    CropAndSave = 0,
    Line = 1,
    Arrow = 2,
    Rectangle = 3,
    Ellipse = 4,
    Highlight = 5,
    Pixelate = 6,
    Blur = 7,
    AutoincrementBubble = 8,
    Text = 9,
    Pencil = 10,

    // These are used for the editing starts with cropping mode

    // Unlike CropAndSave, this one is not visible
    Crop = 11,
    Save = 12,
}

impl Tool {
    pub const fn path(self) -> &'static str {
        match self {
            Tool::CropAndSave => "/kc/kcshot/editor/tool-rectanglecrop.png",
            Tool::Line => "/kc/kcshot/editor/tool-line.png",
            Tool::Arrow => "/kc/kcshot/editor/tool-arrow.png",
            Tool::Rectangle => "/kc/kcshot/editor/tool-rectangle.png",
            Tool::Ellipse => "/kc/kcshot/editor/tool-ellipse.png",
            Tool::Highlight => "/kc/kcshot/editor/tool-highlight.png",
            Tool::Pixelate => "/kc/kcshot/editor/tool-pixelate.png",
            Tool::Blur => "/kc/kcshot/editor/tool-blur.png",
            Tool::AutoincrementBubble => "/kc/kcshot/editor/tool-autoincrementbubble.png",
            Tool::Text => "/kc/kcshot/editor/tool-text.png",
            Tool::Pencil => "/kc/kcshot/editor/tool-pencil.png",
            Tool::Crop => panic!("Nothing should try to get the associated path of the simple Crop tool, as it intentionally does not have a button"),
            Tool::Save => "/kc/kcshot/editor/tool-checkmark.png",
        }
    }

    pub fn from_unicode(key: char) -> Option<Self> {
        use Tool::*;
        Some(match key {
            'c' | 'C' => CropAndSave,
            'l' | 'L' => Line,
            'a' | 'A' => Arrow,
            'r' | 'R' => Rectangle,
            'e' | 'E' => Ellipse,
            'h' | 'H' => Highlight,
            'x' | 'X' => Pixelate,
            'b' | 'B' => Blur,
            'i' | 'I' => AutoincrementBubble,
            't' | 'T' => Text,
            'p' | 'P' => Pencil,
            _ => None?,
        })
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Tool::CropAndSave => "<u>C</u>rop tool",
            Tool::Line => "<u>L</u>ine tool",
            Tool::Arrow => "<u>A</u>rrow tool",
            Tool::Rectangle => "<u>R</u>ectangle tool",
            Tool::Ellipse => "<u>E</u>llipse tool",
            Tool::Highlight => "<u>H</u>ighlight tool",
            Tool::Pixelate => "Pi<u>x</u>elate tool",
            Tool::Blur => "<u>B</u>lur tool",
            Tool::AutoincrementBubble => "Auto<u>i</u>crement bubble tool",
            Tool::Text => "<u>T</u>ext tool",
            Tool::Pencil => "Pe<u>n</u>cil tool",
            Tool::Crop => panic!("Nothing should try to get the tooltip of the simple Crop tool, as it does not have a button"),
            Tool::Save => "Save current screenshot",
        }
    }

    pub const fn is_saving_tool(self) -> bool {
        matches!(self, Self::CropAndSave | Self::Save)
    }

    pub const fn is_cropping_tool(self) -> bool {
        matches!(self, Self::CropAndSave | Self::Crop)
    }
}
