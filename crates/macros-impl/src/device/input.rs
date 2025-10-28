use proc_macro2::Span;
use syn::parse::{Parse, ParseStream};
use syn::{braced, Ident, Path, Token};
use crate::*;
use crate::device::{Mode, NumericKind, Type, Value, Variant};

mod kw {
    use syn::custom_keyword;
    custom_keyword!(bool);
}

impl Parse for Device {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![pub]>()?;
        let name: Ident = input.parse()?;

        let values_content;
        braced!(values_content in input);

        let values = values_content.parse_terminated(Value::parse, Token![,])?;
        drop(values_content);
        let values = values.into_iter().collect();
        Ok(Self { name, values })
    }
}

impl Parse for Value {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut modifier_span = Option::<Span>::None;
        let mut stream_span = None;
        let mut get = false;
        let mut set = false;
        let mut toggle = false;
        let mut correct_order = true;
        while input.peek(Ident) {
            let ident: Ident = input.parse()?;
            if let Some(span) = modifier_span {
                modifier_span = span.join(ident.span());
            }
            match ident.to_string().as_str() {
                "stream" => {
                    if get || set || toggle {
                        correct_order = false;
                    }
                    stream_span = Some(ident.span());
                }
                "get" => {
                    if let Some(stream) = stream_span {
                        return Err(syn::Error::new(
                            stream,
                            "stream is implied by get, both modifiers cannot be used together",
                        ));
                    }
                    if set || toggle {
                        correct_order = false;
                    }
                    get = true;
                }
                "set" => {
                    if toggle {
                        correct_order = false;
                    }
                    set = true;
                }
                "toggle" => {
                    toggle = true
                },
                s => {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!("unknown modifier: '{s}'"),
                    ));
                }
            }
        }
        let stream = stream_span.is_some();
        if !correct_order {
            let mut correct = String::new();
            if stream {
                correct.push_str("stream ")
            }
            if get {
                correct.push_str("get ")
            }
            if set {
                correct.push_str("set ")
            }
            if toggle {
                correct.push_str("toggle ")
            }
            let span = modifier_span.unwrap_or_else(Span::call_site);
            return Err(syn::Error::new(
                span,
                format!("modifiers out of order, use: '{correct}'"),
            ));
        }
        let mode = match (stream, get, set, toggle) {
            (true, false, false, false) => Mode::Stream,
            (true, false, true, false) => Mode::StreamSet,
            (_, true, false, false) => Mode::StreamGet,
            (_, true, true, false) => Mode::StreamGetSet,
            (_, true, true, true) => Mode::StreamGetSetToggle,
            (false, false, true, false) => Mode::Set,
            (false, false, true, true) => Mode::SetToggle,
            tuple => return Err(syn::Error::new(modifier_span.unwrap_or_else(Span::call_site), format!("unanticipated modifier combination: {tuple:?}")))
        };
        if toggle && !set {
            return Err(syn::Error::new(modifier_span.unwrap_or_else(Span::call_site), "toggle cannot be used without set"))
        }
        if !stream && !get && !set && !toggle {
            return Err(syn::Error::new(Span::call_site(), "expected on of stream, get,set or toggle modifiers"))
        }

        let attribute_name = input.parse()?;
        input.parse::<Token![=>]>()?;

        let mut value_name = None;
        if input.peek(Ident) && input.peek2(Token![:]) {
            value_name = input.parse()?;
            input.parse::<Token![:]>()?;
        }
        let value_type = input.parse()?;
        Ok(Self {
            mode,
            attribute_name,
            value_name,
            value_type,
        })
    }
}

impl Parse for Type {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![enum]) {
            input.parse::<Token![enum]>()?;
            let path: Path = input.parse()?;
            let variants;
            braced!(variants in input);
            let variants = variants.parse_terminated(Variant::parse, Token![,])?;
            let variants = variants.into_iter().collect();
            Ok(Self::Enum { path, variants })
        } else if input.peek(kw::bool) {
            input.parse::<kw::bool>()?;
            Ok(Self::Bool)
        } else {
            let kind = input.parse()?;
            let range = if input.peek(Token![<]) {
                input.parse::<Token![<]>()?;
                let min = input.parse()?;
                input.parse::<Token![,]>()?;
                let max = input.parse()?;
                input.parse::<Token![>]>()?;
                Some((min, max))
            } else {
                None
            };
            Ok(Self::Number {
                kind,
                range
            })
        }
    }
}

impl Parse for Variant {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let zigbee = input.parse()?;
        input.parse::<Token![=>]>()?;
        let rust = input.parse()?;
        Ok(Self { zigbee, rust })
    }
}

impl Parse for NumericKind {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let number_type: Ident = input.parse()?;
        Ok(match number_type.to_string().as_str() {
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            "u128" => Self::U128,
            "i8" => Self::I8,
            "i16" => Self::I16,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "i128" => Self::I128,
            _ => return Err(syn::Error::new(number_type.span(), "unknown/unsupported integer type")),
        })
    }
}
