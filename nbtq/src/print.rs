use crab_nbt::{NbtCompound, NbtList, NbtTag};
#[cfg(feature = "colours")]
pub use owo_colors;
#[cfg(feature = "colours")]
use owo_colors::{OwoColorize, Style};

type Result<T = (), E = std::fmt::Error> = std::result::Result<T, E>;

pub struct SnbtWriter<'writer, W> {
    output: &'writer mut W,
    options: WriterOptions,
}

pub struct WriterOptions {
    pub depth: u32,
    pub pretty: bool,
    pub prefix_arrays: bool,
    pub suffix_numbers: bool,
    pub whitespace: String,
    pub quote_mode: QuoteMode,
    #[cfg(feature = "colours")]
    pub styles: Option<WriterStyles>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QuoteMode {
    Always,
    IfNeeded,
    Never,
}

#[cfg(not(feature = "colours"))]
type Style = ();

#[cfg(not(feature = "colours"))]
#[allow(unused)]
trait Unstylish {
    fn style(&self, style: Style) -> &'static str {
        ""
    }
}

#[cfg(not(feature = "colours"))]
impl<T> Unstylish for T {}

#[cfg_attr(not(feature = "colours"), derive(Default))]
#[derive(Clone, Copy)]
pub struct WriterStyles {
    pub strings: Style,
    pub keys: Style,
    pub primitives: Style,
    pub primitive_postfix: Style,
    pub list_prefix: Style,
}

#[cfg(feature = "colours")]
impl Default for WriterStyles {
    fn default() -> Self {
        use owo_colors::style;

        Self {
            strings: style().green(),
            keys: style().bright_blue().bold(),
            primitives: style().purple(),
            primitive_postfix: style().yellow().bold(),
            list_prefix: style().yellow().bold(),
        }
    }
}

impl Default for WriterOptions {
    fn default() -> Self {
        Self {
            depth: 0,
            pretty: false,
            prefix_arrays: true,
            suffix_numbers: true,
            whitespace: "    ".to_string(),
            quote_mode: QuoteMode::IfNeeded,
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
    value: &NbtTag,
    options: WriterOptions,
) -> Result {
    let mut writer = SnbtWriter::new(writer, options);
    writer.write_element(value)?;
    Ok(())
}

pub fn to_snbt_string(value: &NbtTag, options: WriterOptions) -> Result<String> {
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
        let should_quote = match self.options.quote_mode {
            QuoteMode::Always => true,
            QuoteMode::IfNeeded => !s.chars().all(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '_')),
            QuoteMode::Never => false,
        };

        if should_quote {
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
        iter: &'b [impl Into<NbtTag> + 'b + Copy],
    ) -> Result {
        if iter.is_empty() {
            self.output.write_str("[]")?;
            return Ok(());
        }

        self.output.write_char('[')?;
        if self.options.prefix_arrays {
            if let Some(styles) = self.options.styles() {
                self.output
                    .write_str(&prefix.style(styles.list_prefix).to_string())?;
            } else {
                self.output.write_str(prefix)?;
            }
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
            if self.options.suffix_numbers {
                self.output
                    .write_str(&postfix.style(styles.primitive_postfix).to_string())?;
            }
        } else {
            self.output.write_str(&value.to_string())?;
            if self.options.suffix_numbers {
                self.output.write_str(postfix)?;
            }
        }
        Ok(())
    }

    fn write_list(&mut self, list: &NbtList) -> Result {
        if list.is_empty() {
            self.output.write_str("[]")?;
            return Ok(());
        }

        self.output.write_char('[')?;

        self.options.depth += 1;
        let mut first = true;
        for v in list {
            if !first {
                self.output.write_char(',')?;
            }

            self.new_line()?;
            first = false;
            self.write_element(v)?;
        }

        self.options.depth -= 1;
        self.new_line()?;
        self.output.write_char(']')?;

        Ok(())
    }

    fn write_compound(&mut self, compound: &NbtCompound) -> Result {
        if compound.child_tags.is_empty() {
            self.output.write_str("{}")?;
            return Ok(());
        }

        self.output.write_char('{')?;

        self.options.depth += 1;

        let mut first = true;
        for (k, v) in compound.child_tags.iter() {
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
    pub fn write_element(&mut self, value: &NbtTag) -> Result {
        use NbtTag::*;

        match value {
            Byte(v) => self.write_primitive("b", v)?,
            Short(v) => self.write_primitive("s", v)?,
            Int(v) => self.write_primitive("", v)?,
            Long(v) => self.write_primitive("l", v)?,
            Float(v) => self.write_primitive("f", v)?,
            Double(v) => self.write_primitive("d", v)?,
            ByteArray(v) => self.write_primitive_array(
                "B;",
                &v.iter()
                    .copied()
                    .map(|v| i8::from_ne_bytes([v]))
                    .collect::<Box<[_]>>(),
            )?,
            IntArray(v) => self.write_primitive_array("I;", v)?,
            LongArray(v) => self.write_primitive_array("L;", v)?,
            String(v) => self.write_string(v)?,
            List(v) => self.write_list(v)?,
            Compound(v) => self.write_compound(v)?,
            End => (),
        }

        Ok(())
    }
}
