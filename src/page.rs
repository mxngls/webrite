use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

pub static OUT_DIR: &str = "out";
pub static IN_DIR: &str = "in";
pub static BLOCK_DIR: &str = "blocks";
pub static HEADER_SEP: &str = "---";
pub static DRAFT_DIR: &str = "drafts";

#[derive(Debug)]
pub struct Page<'a> {
    path: &'a Path,
    headers: PageHeaders<'a>,
    body: &'a str,
}

impl<'a> Page<'a> {
    pub const fn new(path: &'a Path, headers: PageHeaders<'a>, body: &'a str) -> Self {
        Page { path, headers, body }
    }

    pub const fn path(&self) -> &'a Path {
        self.path
    }
    pub const fn headers(&self) -> &PageHeaders<'a> {
        &self.headers
    }
    pub const fn body(&self) -> &'a str {
        self.body
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
pub struct PageHeaders<'a> {
    title: &'a str,
    description: Option<&'a str>,
    class: Option<&'a str>,
    stylesheet: Option<&'a str>,
    is_post: bool,
    include_header: bool,
    include_footer: bool,
    include_title: bool,
    include_styles: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default)]
struct PageHeadersBuilder<'a> {
    title: Option<&'a str>,
    description: Option<&'a str>,
    class: Option<&'a str>,
    stylesheet: Option<&'a str>,
    is_post: Option<bool>,
    include_header: Option<bool>,
    include_footer: Option<bool>,
    include_title: Option<bool>,
    include_styles: Option<bool>,
}

impl<'a> PageHeadersBuilder<'a> {
    fn set_once<T>(field: &mut Option<T>, key: &HeaderKey, val: T) -> Result<(), ParsePageHeaderErrorKind> {
        if field.is_some() {
            return Err(ParsePageHeaderErrorKind::DuplicateKey(key.to_string()));
        }
        *field = Some(val);
        Ok(())
    }

    fn parse_bool(val: &str) -> Result<bool, ParsePageHeaderErrorKind> {
        match val.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => Ok(true),
            "n" | "no" => Ok(false),
            _ => Err(ParsePageHeaderErrorKind::InvalidBool(val.trim().to_owned())),
        }
    }

    fn set(&mut self, key: &HeaderKey, val: &'a str) -> Result<(), ParsePageHeaderErrorKind> {
        match key {
            // string headers
            HeaderKey::Title => Self::set_once(&mut self.title, key, val)?,
            HeaderKey::Description => Self::set_once(&mut self.description, key, val)?,
            HeaderKey::Class => Self::set_once(&mut self.class, key, val)?,
            HeaderKey::Stylesheet => Self::set_once(&mut self.stylesheet, key, val)?,

            // boolean headers
            HeaderKey::IsPost => Self::set_once(&mut self.is_post, key, Self::parse_bool(val)?)?,
            HeaderKey::IncludeHeader => Self::set_once(&mut self.include_header, key, Self::parse_bool(val)?)?,
            HeaderKey::IncludeFooter => Self::set_once(&mut self.include_footer, key, Self::parse_bool(val)?)?,
            HeaderKey::IncludeTitle => Self::set_once(&mut self.include_title, key, Self::parse_bool(val)?)?,
            HeaderKey::IncludeStyles => Self::set_once(&mut self.include_styles, key, Self::parse_bool(val)?)?,
        }
        Ok(())
    }

    fn build(self) -> Result<PageHeaders<'a>, ParsePageHeaderErrorKind> {
        let is_post = self.is_post.unwrap_or(true);
        let description =
            if is_post {
                Some(self.description.ok_or_else(|| {
                    ParsePageHeaderErrorKind::MissingRequiredHeader(HeaderKey::Description.to_string())
                })?)
            } else {
                self.description
            };
        let title = self
            .title
            .ok_or_else(|| ParsePageHeaderErrorKind::MissingRequiredHeader(HeaderKey::Title.to_string()))?;

        Ok(PageHeaders {
            // required
            title,
            description,

            class: self.class,
            stylesheet: self.stylesheet,
            include_header: self.include_header.unwrap_or(true),
            include_footer: self.include_footer.unwrap_or(true),

            is_post,
            // post overrides
            include_title: self.include_title.unwrap_or(is_post),
            include_styles: self.include_styles.unwrap_or(is_post),
        })
    }
}

enum HeaderKey {
    Title,
    Description,
    Class,
    Stylesheet,
    IsPost,
    IncludeHeader,
    IncludeFooter,
    IncludeTitle,
    IncludeStyles,
}

impl HeaderKey {
    const fn as_str(&self) -> &str {
        match self {
            Self::Title => "title",
            Self::Description => "description",
            Self::Class => "class",
            Self::Stylesheet => "stylesheet",
            Self::IsPost => "is_post",
            Self::IncludeHeader => "include_header",
            Self::IncludeFooter => "include_footer",
            Self::IncludeTitle => "include_title",
            Self::IncludeStyles => "include_styles",
        }
    }
}

impl FromStr for HeaderKey {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "title" => Self::Title,
            "description" => Self::Description,
            "class" => Self::Class,
            "stylesheet" => Self::Stylesheet,
            "is_post" => Self::IsPost,
            "include_header" => Self::IncludeHeader,
            "include_footer" => Self::IncludeFooter,
            "include_title" => Self::IncludeTitle,
            "include_styles" => Self::IncludeStyles,
            _ => return Err(()),
        })
    }
}

impl std::fmt::Display for HeaderKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub fn split_page(page_str: &str) -> Result<(&str, &str), ReadPageError> {
    let mut offset = 0;

    if page_str.trim().is_empty() {
        return Err(ReadPageError::EmptyPage);
    }

    let (header, rest) = page_str
        .split_inclusive('\n')
        .find_map(|line| {
            let trimmed = line.trim();
            let header_end = offset;
            let content_start = offset + line.len();

            if trimmed.is_empty() || trimmed == HEADER_SEP {
                let h = page_str[..header_end].trim();
                let r = page_str[content_start..].trim();
                return Some((h, r));
            }

            offset = content_start;

            None
        })
        .ok_or(ReadPageError::MissingHeaderTerminator)?;

    let body = match rest.lines().next() {
        Some(line) if line == HEADER_SEP => rest[line.len()..].trim_start(),
        _ => rest,
    };

    if header.is_empty() {
        return Err(ReadPageError::MissingHeader);
    }

    if body.is_empty() {
        return Err(ReadPageError::MissingContent);
    }

    Ok((header, body))
}

pub fn parse_header(header_block: &str) -> Result<PageHeaders<'_>, ParsePageHeaderError> {
    let mut headers_builder = PageHeadersBuilder::default();
    let mut ln = 0;
    for (i, line) in header_block.lines().enumerate() {
        ln = i + 1;
        if line.trim().is_empty() {
            continue;
        }
        let Some((key, val)) = line.split_once(':') else {
            return Err(ParsePageHeaderErrorKind::MissingColon.at(ln));
        };
        let Ok(key) = key.trim().parse::<HeaderKey>() else {
            return Err(ParsePageHeaderErrorKind::UnknownKey(key.trim().to_owned()).at(ln));
        };

        let val = val.trim();
        if val.is_empty() {
            return Err(ParsePageHeaderErrorKind::MissingValue(key.to_string().trim().to_owned()).at(ln));
        }

        headers_builder.set(&key, val).map_err(|k| k.at(ln))?;
    }

    headers_builder.build().map_err(|k| k.at(ln))
}

pub fn escape_html(s: &str) -> impl fmt::Display + '_ {
    fmt::from_fn(move |f| {
        let mut last = 0;
        for (i, b) in s.bytes().enumerate() {
            let escaped = match b {
                b'&' => "&amp;",
                b'>' => "&lt;",
                b'<' => "&gt;",
                b'"' => "&quot;",
                b'\'' => "&#39;",
                _ => continue,
            };
            f.write_str(&s[last..i])?;
            f.write_str(escaped)?;
            last = i + 1;
        }
        f.write_str(&s[last..])
    })
}

fn render_content(page: &Page) -> String {
    let body = page.body;
    let heading = if page.headers.include_title {
        format!("<h1>{}</h1>\n", escape_html(page.headers.title))
    } else {
        String::new()
    };

    let content = format!("{heading}{body}\n");

    if page.headers.is_post {
        format!("<article>\n{content}</article>\n")
    } else {
        content
    }
}

pub fn render_page(page: &Page) -> String {
    let headers = &page.headers;

    let escaped_title = escape_html(headers.title);

    let description_meta = headers
        .description
        .map(|d| format!("<meta name=\"description\" content=\"{}\">\n", escape_html(d)))
        .unwrap_or_default();
    let class_attr = headers.class.map(|c| format!(" class=\"{c}\"")).unwrap_or_default();

    // TODO: Fill in placeholder with actual parsed header block
    let header = if headers.include_header {
        todo!("block extraction not yet implemented")
    } else {
        ""
    };

    let content = render_content(page);

    // Indentation is only for the convencience of the reader of this __source code__;
    // actual HTML output will be flat.
    format!(
        "<!DOCTYPE html>\n\
         <html lang=\"en\">\n\
             <head>\n\
                 <title>{escaped_title}</title>\n\
                 {description_meta}\
                 <link href=\"/feed.atom\" type=\"application/atom+xml\" rel=\"alternate\"/>\n\
             </head>\n\
             <body>\n\
                 <div id=\"wrap\"{class_attr}>\n\
                     {header}\
                     <main>\n\
                         {content}\
                     </main>\n\
                 </div>\n\
             </body>\n\
         </html>\n"
    )
}

pub fn write_page(page: &Page) -> Result<(), Error> {
    let out_path = Path::new(OUT_DIR).join(page.path);

    fs::write(&out_path, render_page(page)).at_path(&out_path)
}

#[derive(Debug)]
pub enum Error {
    Io {
        path: Option<PathBuf>,
        source: io::Error,
    },
    ReadPageHeader {
        path: PathBuf,
        source: ReadPageError,
    },
    ParsePageHeader {
        path: PathBuf,
        source: ParsePageHeaderError,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path: Some(path), .. } | Self::ReadPageHeader { path, .. } => {
                write!(f, "{}", path.display())
            }
            Self::Io { path: None, .. } => {
                write!(f, "I/O error")
            }
            Self::ParsePageHeader { path, source } => {
                write!(f, "{}:{}", path.display(), source.line)
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::ReadPageHeader { source, .. } => Some(source),
            Self::ParsePageHeader { source, .. } => Some(source),
        }
    }
}

trait WithPath: std::error::Error + 'static {
    fn with_path(self, path: PathBuf) -> Error;
}

pub trait PathContext<T> {
    fn at_path(self, path: impl Into<PathBuf>) -> Result<T, Error>;
}

impl<T, E: WithPath> PathContext<T> for Result<T, E> {
    fn at_path(self, path: impl Into<PathBuf>) -> Result<T, Error> {
        self.map_err(|e| e.with_path(path.into()))
    }
}

impl WithPath for io::Error {
    fn with_path(self, path: PathBuf) -> Error {
        Error::Io {
            path: Some(path),
            source: self,
        }
    }
}

#[derive(Debug)]
pub enum ReadPageError {
    EmptyPage,
    MissingHeaderTerminator,
    MissingHeader,
    MissingContent,
}

impl fmt::Display for ReadPageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Self::EmptyPage => write!(f, "page is empty"),
            Self::MissingHeaderTerminator => write!(f, "missing header ending '---'"),
            Self::MissingContent => write!(f, "body has no content"),
            Self::MissingHeader => write!(f, "header has no content"),
        }
    }
}

impl std::error::Error for ReadPageError {}

impl WithPath for ReadPageError {
    fn with_path(self, path: PathBuf) -> Error {
        Error::ReadPageHeader { path, source: self }
    }
}

#[derive(Debug)]
pub struct ParsePageHeaderError {
    line: usize,
    kind: ParsePageHeaderErrorKind,
}

impl fmt::Display for ParsePageHeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ParsePageHeaderErrorKind::MissingColon => write!(f, "missing ':'"),
            ParsePageHeaderErrorKind::MissingRequiredHeader(k) => write!(f, "missing required '{k}' header"),
            ParsePageHeaderErrorKind::MissingValue(k) => write!(f, "missing value for '{k}' header"),
            ParsePageHeaderErrorKind::UnknownKey(k) => write!(f, "unkown header key '{k}'"),
            ParsePageHeaderErrorKind::DuplicateKey(k) => write!(f, "duplicate header key '{k}"),
            ParsePageHeaderErrorKind::InvalidBool(k) => {
                write!(f, "valid values for '{k}' are 'yes' ('y') or 'no' ('n')")
            }
        }
    }
}

impl std::error::Error for ParsePageHeaderError {}

impl WithPath for ParsePageHeaderError {
    fn with_path(self, path: PathBuf) -> Error {
        Error::ParsePageHeader { path, source: self }
    }
}

#[derive(Debug)]
enum ParsePageHeaderErrorKind {
    MissingColon,
    MissingValue(String),
    MissingRequiredHeader(String),
    UnknownKey(String),
    DuplicateKey(String),
    InvalidBool(String),
}

impl ParsePageHeaderErrorKind {
    const fn at(self, line: usize) -> ParsePageHeaderError {
        ParsePageHeaderError { line, kind: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod split_page {
        use super::*;
        use indoc::indoc;

        #[test]
        fn splits_by_separator() {
            let page = indoc! {"
                title: Example
                description: example post with separator
                ---
                <p>Hello, World!</p>
            "};

            let (header, content) = split_page(page).expect("page to be split by the '---' separator");

            assert_eq!(header, "title: Example\ndescription: example post with separator");
            assert_eq!(content, "<p>Hello, World!</p>");
        }

        #[test]
        fn splits_by_newlines() {
            let page = indoc! {"
                title: Example
                description: example post with newlines



                <p>Hello, World!</p>
            "};

            let (header, content) = split_page(page).expect("page to be split by the '\n' separator");

            assert_eq!(header, "title: Example\ndescription: example post with newlines");
            assert_eq!(content, "<p>Hello, World!</p>");
        }

        #[test]
        fn splits_by_newlines_and_separator() {
            let page = indoc! {"
                title: Example
                description: example post with separator and newlines


                ---


                <p>Hello, World!</p>
            "};

            let (header, content) = split_page(page).expect("page to be split by the '\\n' separator");

            assert_eq!(
                header,
                "title: Example\ndescription: example post with separator and newlines"
            );
            assert_eq!(content, "<p>Hello, World!</p>");
        }

        #[test]
        fn rejects_empty_page() {
            let page = "";

            let r = split_page(page);

            assert!(matches!(r, Err(ReadPageError::EmptyPage)), "got {r:?}");
        }

        #[test]
        fn rejects_page_conisting_of_newlines() {
            let page = "\n\n\n";

            let r = split_page(page);

            assert!(matches!(r, Err(ReadPageError::EmptyPage)), "got {r:?}");
        }
        #[test]
        fn rejects_missing_separator() {
            let page = indoc! {"
                title: Example
                description: example post with separator and newlines
                <p>Hello, World!</p>
            "};

            let r = split_page(page);

            assert!(matches!(r, Err(ReadPageError::MissingHeaderTerminator)), "got {r:?}");
        }

        // this is essentially the above test duplicated, but kept as it reads semantically different
        #[test]
        fn rejects_missing_separator_with_no_body() {
            let page = indoc! {"
                title: Example
                description: example post with separator and newlines
            "};

            let r = split_page(page);

            assert!(matches!(r, Err(ReadPageError::MissingHeaderTerminator)), "got {r:?}");
        }

        #[test]
        fn rejects_empty_header_with_separator() {
            let page = indoc! {"
                ---
                <p>Hello, World!</p>
            "};

            let r = split_page(page);

            assert!(matches!(r, Err(ReadPageError::MissingHeader)), "got {r:?}");
        }

        #[test]
        fn rejects_empty_header_without_separator() {
            let page = indoc! {"

                <p>Hello, World!</p>
            "};

            let r = split_page(page);

            assert!(matches!(r, Err(ReadPageError::MissingHeader)), "got {r:?}");
        }

        #[test]
        fn rejects_empty_content_with_separator() {
            let page = indoc! {"
                title: Example
                description: example post with separator and newlines
                ---
            "};

            let r = split_page(page);

            assert!(matches!(r, Err(ReadPageError::MissingContent)), "got {r:?}");
        }
    }

    mod parse_headers {
        use super::*;
        use indoc::indoc;

        #[test]
        fn parse_required_headers() {
            let header_str = indoc! {"
                title: Example
                description: example post with separator and newlines
            "};

            let headers = parse_header(header_str).unwrap();

            assert_eq!(headers.title, "Example", "got {:?}", headers.title);
            assert_eq!(
                headers.description.unwrap(),
                "example post with separator and newlines",
                "got {:?}",
                headers.description.unwrap()
            );
        }

        #[test]
        fn rejects_missing_title() {
            let header_str = indoc! {"
                description: example post without a title
            "};

            let headers = parse_header(header_str);

            assert!(
                matches!(
                    &headers,
                    Err(ParsePageHeaderError {
                        line: 1,
                        kind: ParsePageHeaderErrorKind::MissingRequiredHeader(k),
                    }) if k == "title"
                ),
                "got {headers:?}"
            );
        }

        #[test]
        fn rejects_missing_description_for_post() {
            let header_str = indoc! {"
                title: Example
                is_post: yes
            "};

            let headers = parse_header(header_str);

            assert!(
                matches!(
                    &headers,
                    Err(ParsePageHeaderError {
                        line: 2,
                        kind: ParsePageHeaderErrorKind::MissingRequiredHeader(k),
                    }) if k == "description"
                ),
                "got {headers:?}"
            );
        }

        #[test]
        fn skips_blank_lines() {
            let header_str = "title: Example\n\n \t \ndescription: example post with blank lines\n";

            let headers = parse_header(header_str).unwrap();

            assert_eq!(headers.title, "Example");
            assert_eq!(headers.description, Some("example post with blank lines"));
        }

        #[test]
        fn trims_whitespace_around_keys_and_values() {
            let header_str = "  title  :   Example Page  \n\tdescription\t:\texample post with padded fields\t\n";

            let headers = parse_header(header_str).unwrap();

            assert_eq!(headers.title, "Example Page");
            assert_eq!(headers.description, Some("example post with padded fields"));
        }

        #[test]
        fn splits_on_the_first_colon_only() {
            let header_str = indoc! {"
                  title: Example: A Post
                  description: see https://example.com for details
                  "};

            let headers = parse_header(header_str).unwrap();

            assert_eq!(headers.title, "Example: A Post");
            assert_eq!(headers.description, Some("see https://example.com for details"));
        }

        #[test]
        fn rejects_missing_colon() {
            let header_str = indoc! {"
                title Example
                description: Title header misses a colon
            "};

            let headers = parse_header(header_str);

            assert!(
                matches!(
                    headers,
                    Err(ParsePageHeaderError {
                        line: 1,
                        kind: ParsePageHeaderErrorKind::MissingColon
                    })
                ),
                "got {headers:#?}"
            );
        }

        #[test]
        fn rejects_unkown_key() {
            let header_str = indoc! {"
                title: Example
                subtitle: Our first example
                description: A post with a subtitle
            "};

            let headers = parse_header(header_str);

            assert!(
                matches!(
                    headers,
                    Err(ParsePageHeaderError {
                        line: 2,
                        kind: ParsePageHeaderErrorKind::UnknownKey(ref key)
                    }) if key == "subtitle"
                ),
                "got {headers:#?}"
            );
        }

        #[test]
        fn rejects_empty_string_key() {
            let header_str = ": Example\n";

            let headers = parse_header(header_str);

            assert!(
                matches!(
                    &headers,
                    Err(ParsePageHeaderError {
                        line: 1,
                        kind: ParsePageHeaderErrorKind::UnknownKey(k),
                    }) if k.is_empty()
                ),
                "got {headers:?}"
            );
        }

        #[test]
        fn rejects_duplicate_boolean_key() {
            let header_str = indoc! {"
                title: Example
                is_post: yes
                is_post: no
            "};

            let headers = parse_header(header_str);

            assert!(
                matches!(
                    &headers,
                    Err(ParsePageHeaderError {
                        line: 3,
                        kind: ParsePageHeaderErrorKind::DuplicateKey(k),
                    }) if k == "is_post"
                ),
                "got {headers:?}"
            );
        }

        #[test]
        fn ensure_defaults() {
            let header_str = indoc! {"
                title: Example
                description: example post with separator and newlines
            "};

            let headers = parse_header(header_str).unwrap();

            assert_eq!(headers.class, None, "got {:?}", headers.class);
            assert_eq!(headers.stylesheet, None, "got {:?}", headers.stylesheet);

            // boolean fields
            assert!(headers.is_post, "got {:?}", headers.is_post);
            assert!(headers.include_header, "got {:?}", headers.include_header);
            assert!(headers.include_footer, "got {:?}", headers.include_footer);
            assert!(headers.include_title, "got {:?}", headers.include_title);
            assert!(headers.include_styles, "got {:?}", headers.include_styles);
        }

        #[test]
        fn explicit_defaults_override() {
            let header_str = indoc! {"
                title: Example
                description: example page with every header set explicitly

                class: wide
                stylesheet: /style/page.css

                is_post: no
                include_header: no
                include_footer: no
                include_title: yes
                include_styles: yes
            "};

            let headers = parse_header(header_str).unwrap();

            assert_eq!(headers.class, Some("wide"));
            assert_eq!(headers.stylesheet, Some("/style/page.css"));

            assert!(!headers.is_post, "got {:?}", headers.is_post);

            // added even despite page not being a post
            assert!(headers.include_title, "got {:?}", headers.include_title);
            assert!(headers.include_styles, "got {:?}", headers.include_styles);

            // set by default
            assert!(!headers.include_header, "got {:?}", headers.include_header);
            assert!(!headers.include_footer, "got {:?}", headers.include_footer);
        }

        #[test]
        fn accepts_bool_aliases_case_insensitively() {
            let header_str = indoc! {"
                  title: Example
                  description: example post with assorted boolean spellings

                  is_post: YES
                  include_header: y

                  include_styles: NO
                  include_footer: No
                  include_title: n
                  "};

            let headers = parse_header(header_str).unwrap();

            assert!(headers.is_post, "got {:?}", headers.is_post);
            assert!(headers.include_header, "got {:?}", headers.include_header);
            assert!(!headers.include_footer, "got {:?}", headers.include_footer);
            assert!(!headers.include_title, "got {:?}", headers.include_title);
            assert!(!headers.include_styles, "got {:?}", headers.include_styles);
        }
    }
}
