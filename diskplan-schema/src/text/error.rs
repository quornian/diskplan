use std::fmt::Display;

/// A detailed error for an issue encountered during parsing
#[derive(Debug, PartialEq)]
pub struct ParseError<'a> {
    error: String,
    text: &'a str,
    span: &'a str,
    next: Option<Box<ParseError<'a>>>,
}

impl Display for ParseError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lineno = self.line_number();
        let line = self.text.lines().nth(lineno - 1).unwrap_or("<EOF>");
        let column = self.span.as_ptr() as usize - line.as_ptr() as usize;
        writeln!(f, "Error: {}", self.error)?;
        writeln!(f, "     |")?;
        writeln!(f, "{lineno:4} | {line}")?;
        if column == 0 {
            writeln!(f, "     |")?;
        } else {
            writeln!(f, "     | {0:1$}^", "", column)?;
        }
        if let Some(next) = &self.next {
            write!(f, "{next}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseError<'_> {}

impl<'a> ParseError<'a> {
    /// Constructs a detailed error for an issue encountered during parsing
    pub fn new(
        error: String,
        text: &'a str,
        span: &'a str,
        next: Option<Box<ParseError<'a>>>,
    ) -> ParseError<'a> {
        ParseError {
            error,
            text,
            span,
            next,
        }
    }

    /// Returns the calculated line number of the span within the text
    pub fn line_number(&self) -> usize {
        let pos = self.span.as_ptr() as usize - self.text.as_ptr() as usize;
        self.text[..pos].chars().filter(|&c| c == '\n').count() + 1
    }
}

impl<'a, 'b> IntoIterator for &'b ParseError<'a> {
    type IntoIter = ParseErrorIter<'a, 'b>;
    type Item = &'b ParseError<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ParseErrorIter { err: Some(self) }
    }
}

pub struct ParseErrorIter<'a, 'b> {
    err: Option<&'b ParseError<'a>>,
}

impl<'a, 'b> Iterator for ParseErrorIter<'a, 'b> {
    type Item = &'b ParseError<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let cur = self.err;
        if let Some(err) = cur {
            self.err = err.next.as_deref();
        }
        cur
    }
}
