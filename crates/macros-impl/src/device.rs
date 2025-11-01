mod input;
mod output;
use proc_macro2::Ident;
use syn::{LitInt, LitStr, Path};


#[derive(Clone, Debug)]
pub struct Device {
    docs: Vec<LitStr>,
    url: LitStr,
    name: Ident,
    values: Vec<Value>,
}

#[derive(Clone, Debug)]
struct Value {
    docs: Vec<LitStr>,
    mode: Mode,
    attribute_name: LitStr,
    value_name: Option<Ident>,
    value_type: Type,
}

#[derive(Clone, Debug, Copy, Eq, PartialEq)]
enum Mode {
    Stream,
    StreamGet,
    Set,
    StreamSet,
    StreamGetSet,
    SetToggle,
    StreamGetSetToggle
}

impl Mode {
    fn sub_pub(&self) -> SubPub {
        match self {
            Mode::Stream => SubPub::SubOnly,
            Mode::StreamGet => SubPub::Both,
            Mode::Set => SubPub::PubOnly,
            Mode::StreamSet => SubPub::Both,
            Mode::StreamGetSet => SubPub::Both,
            Mode::SetToggle => SubPub::PubOnly,
            Mode::StreamGetSetToggle => SubPub::Both,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SubPub {
    SubOnly,
    PubOnly,
    Both
}

#[derive(Clone, Debug)]
enum Type {
    Enum {
        path: Path,
        variants: Vec<Variant>,
    },
    Number {
        kind: NumericKind,
        range: Option<(LitInt, LitInt)>
    },
    Bool
}

#[derive(Clone, Debug)]
enum NumericKind {
    U8,
    U16,
    U32,
    U64,
    U128,
    I8,
    I16,
    I32,
    I64,
    I128,
}

#[derive(Clone, Debug)]
struct Variant {
    zigbee: syn::LitStr,
    rust: Ident,
}
