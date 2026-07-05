#[cfg(feature = "colours")]
pub use owo_colors;
#[cfg(feature = "colours")]
use owo_colors::{OwoColorize, Style};

use valence_nbt::{Compound, List, Value};

type Result<T = (), E = std::fmt::Error> = std::result::Result<T, E>;

pub struct SnbtWriter<'writer, W> {
    output: &'writer mut W,
    options: WriterOptions,
}

pub struct WriterOptions {
    pub depth: u32,
    pub pretty: bool,
    pub prefix_lists: bool,
    pub postfix_primitives: bool,
    pub whitespace: String,
    #[cfg(feature = "colours")]
    pub styles: Option<WriterStyles>,
}

#[cfg(not(feature = "colours"))]
type Style = ();

#[cfg(not(feature = "colours"))]
trait Unstylish {
    fn style(&self, style: Style) -> &'static str {
        ""
    }
}

#[cfg(not(feature = "colours"))]
impl<T> Unstylish for T {}

#[cfg_attr(not(feature = "colours"), derive(Default))]
pub struct WriterStyles {
    pub strings: Style,
    pub keys: Style,
    pub primitives: Style,
    pub primitive_postfix: Style,
}

#[cfg(feature = "colours")]
impl Default for WriterStyles {
    fn default() -> Self {
        use owo_colors::style;

        Self {
            strings: style().green(),
            keys: style().bright_blue().bold(),
            primitives: style().purple(),
            primitive_postfix: style().purple().bold(),
        }
    }
}

impl Default for WriterOptions {
    fn default() -> Self {
        Self {
            depth: 0,
            pretty: false,
            prefix_lists: true,
            postfix_primitives: true,
            whitespace: "    ".to_string(),
            #[cfg(feature = "colours")]
            styles: None,
        }
    }
}

impl WriterOptions {
    pub fn styles(&self) -> Option<&WriterStyles> {
        cfg_select! {
            feature = "colours" => self.styles.as_ref(),
            _ => None,
        }
    }

    pub fn styles_mut(&mut self) -> Option<&mut WriterStyles> {
        cfg_select! {
            feature = "colours" => self.styles.as_mut(),
            _ => None,
        }
    }
}

pub fn write_snbt_string<W: std::fmt::Write>(
    writer: &mut W,
    value: &Value,
    options: WriterOptions,
) -> Result {
    let mut writer = SnbtWriter::new(writer, options);
    writer.write_element(value)?;
    Ok(())
}

pub fn to_snbt_string(value: &Value, options: WriterOptions) -> Result<String> {
    let mut s = String::new();
    let mut writer = SnbtWriter::new(&mut s, options);
    writer.write_element(value)?;
    Ok(s)
}

impl<'writer, W: std::fmt::Write> SnbtWriter<'writer, W> {
    pub fn new(output: &'writer mut W, options: WriterOptions) -> Self {
        Self { output, options }
    }

    fn new_line(&mut self) -> Result {
        if self.options.pretty {
            self.output.write_char('\n')?;

            for _ in 0..self.options.depth {
                self.output.write_str(&self.options.whitespace)?;
            }
        }

        Ok(())
    }

    fn write_string(&mut self, s: &str) -> Result {
        let mut need_quote = false;
        for c in s.chars() {
            if !matches!(c, 'a'..='z' | 'A'..='Z' | '_' | '-' | '+' | '.') {
                need_quote = true;
                break;
            }
        }

        if need_quote {
            if let Some(styles) = self.options.styles() {
                write!(self.output, "{:?}", s.style(styles.strings))?;
            } else {
                write!(self.output, "{s:?}")?;
            }
        } else {
            if let Some(styles) = self.options.styles() {
                write!(self.output, "{}", s.style(styles.strings))?;
            } else {
                self.output.write_str(s)?;
            }
        }

        Ok(())
    }

    fn write_primitive_array<'b>(
        &mut self,
        prefix: &str,
        iter: &'b [impl Into<Value> + 'b + Copy],
    ) -> Result {
        if iter.is_empty() {
            self.output.write_str("[]")?;
            return Ok(());
        }

        self.output.write_char('[')?;
        if self.options.prefix_lists {
            self.output.write_str(prefix)?;
        }

        self.options.depth += 1;

        let mut first = true;

        for v in iter {
            if !first {
                self.output.write_char(',')?;
            }
            first = false;

            self.new_line()?;
            self.write_element(&(*v).into())?;
        }

        self.options.depth -= 1;
        self.new_line()?;
        self.output.write_char(']')?;

        Ok(())
    }

    fn write_primitive(&mut self, postfix: &str, value: impl std::fmt::Display) -> Result {
        if let Some(styles) = self.options.styles() {
            write!(self.output, "{}", value.style(styles.primitives))?;
            if self.options.postfix_primitives {
                self.output
                    .write_str(&postfix.style(styles.primitive_postfix).to_string())?;
            }
        } else {
            self.output.write_str(&value.to_string())?;
            if self.options.postfix_primitives {
                self.output.write_str(postfix)?;
            }
        }
        Ok(())
    }

    fn write_list(&mut self, list: &List) -> Result {
        macro_rules! variant_impl {
            ($v:expr, $handle:expr) => {{
                if $v.is_empty() {
                    self.output.write_str("[]")?;
                    return Ok(());
                }

                self.output.write_char('[')?;

                self.options.depth += 1;
                let mut first = true;
                for v in $v.iter() {
                    if !first {
                        self.output.write_char(',')?;
                    }

                    self.new_line()?;
                    first = false;
                    $handle(v)?;
                }

                self.options.depth -= 1;
                self.new_line()?;
                self.output.write_char(']')?;

                Ok(())
            }};
        }
        #[allow(clippy::redundant_closure_call)]
        match list {
            List::Byte(v) => variant_impl!(v, |v| self.write_primitive("b", v)),
            List::Short(v) => variant_impl!(v, |v| self.write_primitive("s", v)),
            List::Int(v) => variant_impl!(v, |v| self.write_primitive("", v)),
            List::Long(v) => variant_impl!(v, |v| self.write_primitive("l", v)),
            List::Float(v) => variant_impl!(v, |v| self.write_primitive("f", v)),
            List::Double(v) => variant_impl!(v, |v| self.write_primitive("d", v)),
            List::ByteArray(v) => {
                variant_impl!(v, |v: &Vec<i8>| self.write_primitive_array("B", v))
            }
            List::IntArray(v) => {
                variant_impl!(v, |v: &Vec<i32>| self.write_primitive_array("", v))
            }
            List::LongArray(v) => {
                variant_impl!(v, |v: &Vec<i64>| self.write_primitive_array("L", v))
            }
            List::String(v) => variant_impl!(v, |v| self.write_string(v)),
            List::List(v) => variant_impl!(v, |v| self.write_list(v)),
            List::Compound(v) => variant_impl!(v, |v| self.write_compound(v)),
            List::End => self.output.write_str("[]"),
        }
    }

    fn write_compound(&mut self, compound: &Compound) -> Result {
        if compound.is_empty() {
            self.output.write_str("{}")?;
            return Ok(());
        }

        self.output.write_char('{')?;

        self.options.depth += 1;

        let mut first = true;
        for (k, v) in compound.iter() {
            if !first {
                self.output.write_char(',')?;
            }

            first = false;

            self.new_line()?;

            if let Some(styles) = self.options.styles_mut() {
                std::mem::swap(&mut styles.strings, &mut styles.keys);
            }
            let result = self.write_string(k);
            if let Some(styles) = self.options.styles_mut() {
                std::mem::swap(&mut styles.strings, &mut styles.keys);
            }
            result?;

            if self.options.pretty {
                self.output.write_str(": ")?;
            } else {
                self.output.write_char(':')?;
            }
            self.write_element(v)?;
        }

        self.options.depth -= 1;
        self.new_line()?;

        self.output.write_char('}')?;
        Ok(())
    }

    /// Write a value to the output.
    pub fn write_element(&mut self, value: &Value) -> Result {
        use Value::*;

        match value {
            Byte(v) => self.write_primitive("b", v)?,
            Short(v) => self.write_primitive("s", v)?,
            Int(v) => self.write_primitive("", v)?,
            Long(v) => self.write_primitive("l", v)?,
            Float(v) => self.write_primitive("f", v)?,
            Double(v) => self.write_primitive("d", v)?,
            ByteArray(v) => self.write_primitive_array("B;", v)?,
            IntArray(v) => self.write_primitive_array("I;", v)?,
            LongArray(v) => self.write_primitive_array("L;", v)?,
            String(v) => self.write_string(v)?,
            List(v) => self.write_list(v)?,
            Compound(v) => self.write_compound(v)?,
        }

        Ok(())
    }
}
