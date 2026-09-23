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
pub static DEFAULT_STYLESHEET: &str = "/style.css";
pub static DEFAULT_WRAP_ID: &str = "wrap";

#[derive(Debug)]
pub struct Page {
    path: PathBuf,
    headers: PageHeaders,
    body: String,
}

impl Page {
    pub const fn new(path: PathBuf, headers: PageHeaders, body: String) -> Self {
        Self { path, headers, body }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
pub struct PageHeaders {
    title: String,
    description: Option<String>,
    css_classes: Option<String>,
    css_stylesheet_path: Option<String>,
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

    fn build(self) -> Result<PageHeaders, ParsePageHeaderErrorKind> {
        let is_post = self.is_post.unwrap_or(true);
        let description = if is_post {
            Some(
                self.description
                    .ok_or_else(|| ParsePageHeaderErrorKind::MissingRequiredHeader(HeaderKey::Description.to_string()))?
                    .to_owned(),
            )
        } else {
            self.description.map(str::to_owned)
        };
        let title = self
            .title
            .ok_or_else(|| ParsePageHeaderErrorKind::MissingRequiredHeader(HeaderKey::Title.to_string()))?;

        Ok(PageHeaders {
            // required
            title: title.to_string(),
            description,

            css_classes: self.class.map(str::to_owned),
            css_stylesheet_path: self.stylesheet.map(str::to_owned),
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

pub fn parse_header(header_block: &str) -> Result<PageHeaders, ParsePageHeaderError> {
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

// TODO: Instead of the naive escaping approach chosen here we could probably go with something
// along the lines of: https://github.com/k0kubun/hescape-ruby
pub fn escape_html(s: &str) -> impl fmt::Display + '_ {
    fmt::from_fn(move |f| {
        let mut last = 0;
        for (i, b) in s.bytes().enumerate() {
            let escaped = match b {
                b'&' => "&amp;",
                b'<' => "&lt;",
                b'>' => "&gt;",
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

fn render_head(page: &Page, blocks: &Blocks) -> String {
    let headers = &page.headers;
    let escaped_title = escape_html(&headers.title);

    let description_meta = headers
        .description
        .as_ref()
        .map(|d| format!("<meta name=\"description\" content=\"{}\">\n", escape_html(d)))
        .unwrap_or_default();

    let custom_style_link = headers.css_stylesheet_path.as_ref().map_or_else(String::new, |s| {
        format!("<link href=\"{}\" rel=\"stylesheet\"/>\n", escape_html(s))
    });

    let style_link = if headers.include_styles {
        format!("<link href=\"{DEFAULT_STYLESHEET}\" rel=\"stylesheet\"/>\n")
    } else {
        String::new()
    };

    let head_block = blocks.head.as_deref().unwrap_or("<!-- head -->\n");

    format!(
        "<head>\n\
             <title>{escaped_title}</title>\n\
             {description_meta}\
             {custom_style_link}\
             {style_link}\
             <link href=\"/feed.atom\" type=\"application/atom+xml\" rel=\"alternate\"/>\n\
             {head_block}\
         </head>\n\
         "
    )
}

fn render_content(page: &Page) -> String {
    let body = &page.body;
    let heading = if page.headers.include_title {
        format!("<h1>{}</h1>\n", escape_html(&page.headers.title))
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

pub fn render_page(page: &Page, blocks: &Blocks) -> String {
    let headers = &page.headers;

    let class_attr = headers
        .css_classes
        .as_ref()
        .map(|c| format!(" class=\"{}\"", escape_html(c)))
        .unwrap_or_default();

    let header = if headers.include_header {
        blocks.header.as_deref().unwrap_or("<!-- header -->\n")
    } else {
        ""
    };

    let footer = if headers.include_footer {
        blocks.footer.as_deref().unwrap_or("<!-- footer -->\n")
    } else {
        ""
    };

    let head = render_head(page, blocks);
    let content = render_content(page);

    // Indentation is only for the convencience of the reader of this __source code__;
    // actual HTML output will be flat.
    format!(
        "<!DOCTYPE html>\n\
         <html lang=\"en\">\n\
             {head}\
             <body>\n\
                 <div id=\"{DEFAULT_WRAP_ID}\"{class_attr}>\n\
                     {header}\
                     <main>\n\
                         <!-- content start -->\n\
                         {content}\
                         <!-- content end -->\n\
                     </main>\n\
                     {footer}\
                 </div>\n\
             </body>\n\
         </html>\n"
    )
}

pub fn write_page(page: &Page, blocks: &Blocks) -> Result<(), Error> {
    let out_path = Path::new(OUT_DIR).join(&page.path);

    fs::write(&out_path, render_page(page, blocks)).at_path(&out_path)
}

// pub fn render_feed(page: &Page) -> Result<(), Error> -> {
// }
//
// pub fn write_feed(page: &Page) -> Result<(), Error> -> {
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Head,
    Header,
    Footer,
}

impl fmt::Display for BlockKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Head => write!(f, "Head"),
            Self::Header => write!(f, "Header"),
            Self::Footer => write!(f, "Footer"),
        }
    }
}

impl FromStr for BlockKind {
    type Err = ReadBlockError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "head.htm" => Ok(Self::Head),
            "header.htm" => Ok(Self::Header),
            "footer.htm" => Ok(Self::Footer),
            _ => Err(ReadBlockError::Unrecognized),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block(BlockKind, String);

impl Block {
    fn new(kind: BlockKind, content: String) -> Result<Self, ReadBlockError> {
        if content.trim().is_empty() {
            return Err(ReadBlockError::Empty(kind));
        }

        Ok(Self(kind, content))
    }

    pub fn from_path(block_path: &Path) -> Result<Self, Error> {
        let kind = block_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(ReadBlockError::Unrecognized)
            .and_then(BlockKind::from_str)
            .at_path(block_path)?;

        let content = fs::read_to_string(block_path).at_path(block_path)?;

        Self::new(kind, content).at_path(block_path)
    }

    fn into_parts(self) -> (BlockKind, String) {
        (self.0, self.1)
    }
}

#[derive(Debug, Default)]
pub struct Blocks {
    head: Option<String>,
    header: Option<String>,
    footer: Option<String>,
}

impl Blocks {
    pub fn from_dir(dir: &Path) -> Result<Self, Error> {
        let mut blocks = Self::default();

        for entry in fs::read_dir(dir).at_path(dir)? {
            let path = entry.at_path(dir)?.path();

            match Block::from_path(&path) {
                Ok(block) => blocks.insert(block),
                Err(Error::ReadBlock {
                    source: ReadBlockError::Unrecognized,
                    ..
                }) => (),
                Err(e) => return Err(e),
            }
        }

        Ok(blocks)
    }

    fn insert(&mut self, block: Block) {
        let (kind, content) = block.into_parts();

        match kind {
            BlockKind::Head => self.head = Some(content),
            BlockKind::Header => self.header = Some(content),
            BlockKind::Footer => self.footer = Some(content),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Io {
        path: Option<PathBuf>,
        source: io::Error,
    },
    ParsePageHeader {
        path: PathBuf,
        source: ParsePageHeaderError,
    },
    ReadPageHeader {
        path: PathBuf,
        source: ReadPageError,
    },
    ReadBlock {
        path: PathBuf,
        source: ReadBlockError,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path: Some(path), .. } | Self::ReadPageHeader { path, .. } | Self::ReadBlock { path, .. } => {
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
            Self::ReadBlock { source, .. } => Some(source),
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
pub enum ReadBlockError {
    Empty(BlockKind),
    Unrecognized,
}

impl fmt::Display for ReadBlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty(kind) => write!(f, "{kind} block is empty"),
            Self::Unrecognized => write!(f, "expected one of 'head.htm', 'header.htm', 'footer.htm'"),
        }
    }
}

impl std::error::Error for ReadBlockError {}

impl WithPath for ReadBlockError {
    fn with_path(self, path: PathBuf) -> Error {
        Error::ReadBlock { path, source: self }
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
                headers.description.as_ref().unwrap(),
                "example post with separator and newlines",
                "got {:?}",
                headers.description.as_ref().unwrap()
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
            assert_eq!(headers.description, Some("example post with blank lines".to_string()));
        }

        #[test]
        fn trims_whitespace_around_keys_and_values() {
            let header_str = "  title  :   Example Page  \n\tdescription\t:\texample post with padded fields\t\n";

            let headers = parse_header(header_str).unwrap();

            assert_eq!(headers.title, "Example Page");
            assert_eq!(headers.description, Some("example post with padded fields".to_string()));
        }

        #[test]
        fn splits_on_the_first_colon_only() {
            let header_str = indoc! {"
                  title: Example: A Post
                  description: see https://example.com for details
                  "};

            let headers = parse_header(header_str).unwrap();

            assert_eq!(headers.title, "Example: A Post");
            assert_eq!(
                headers.description,
                Some("see https://example.com for details".to_string())
            );
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

            assert_eq!(headers.css_classes, None, "got {:?}", headers.css_classes);
            assert_eq!(
                headers.css_stylesheet_path, None,
                "got {:?}",
                headers.css_stylesheet_path
            );

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

            assert_eq!(headers.css_classes, Some("wide".to_string()));
            assert_eq!(headers.css_stylesheet_path, Some("/style/page.css".to_string()));

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

    mod blocks {
        use super::*;

        fn block(kind: BlockKind, content: &str) -> Block {
            Block::new(kind, content.to_owned()).expect("block content to be non-empty")
        }

        #[test]
        fn recognizes_block_file_names() {
            assert_eq!(BlockKind::from_str("head.htm").unwrap(), BlockKind::Head);
            assert_eq!(BlockKind::from_str("header.htm").unwrap(), BlockKind::Header);
            assert_eq!(BlockKind::from_str("footer.htm").unwrap(), BlockKind::Footer);
        }

        #[test]
        fn rejects_other_file_names() {
            for name in ["index.htm", "head.html", "Head.htm", "head", ""] {
                let kind = BlockKind::from_str(name);

                assert!(
                    matches!(kind, Err(ReadBlockError::Unrecognized)),
                    "got {kind:?} for {name:?}"
                );
            }
        }

        #[test]
        fn rejects_blank_block() {
            let block = Block::new(BlockKind::Head, " \n\t\n".to_owned());

            assert!(
                matches!(block, Err(ReadBlockError::Empty(BlockKind::Head))),
                "got {block:?}"
            );
        }

        #[test]
        fn keeps_block_content_verbatim() {
            let content = "\n<nav>\n  <a href=\"/\">home</a>\n</nav>\n";

            let (kind, kept) = block(BlockKind::Header, content).into_parts();

            assert_eq!(kind, BlockKind::Header);
            assert_eq!(kept, content);
        }

        #[test]
        fn places_blocks_by_kind() {
            let mut blocks = Blocks::default();

            blocks.insert(block(BlockKind::Head, "<meta charset=\"utf-8\">"));
            blocks.insert(block(BlockKind::Header, "<nav>nav</nav>"));
            blocks.insert(block(BlockKind::Footer, "<footer>footer</footer>"));

            assert_eq!(blocks.head.as_deref(), Some("<meta charset=\"utf-8\">"));
            assert_eq!(blocks.header.as_deref(), Some("<nav>nav</nav>"));
            assert_eq!(blocks.footer.as_deref(), Some("<footer>footer</footer>"));
        }

        #[test]
        fn can_override_block() {
            let mut blocks = Blocks::default();

            blocks.insert(block(BlockKind::Header, "<nav>first</nav>"));
            blocks.insert(block(BlockKind::Header, "<nav>second</nav>"));

            assert_eq!(blocks.header.as_deref(), Some("<nav>second</nav>"));
            assert_eq!(blocks.head, None);
            assert_eq!(blocks.footer, None);
        }
    }

    mod render_page {
        use super::*;
        use std::env;
        use std::fmt::Write;

        fn default_page(headers: PageHeaders) -> Page {
            Page::new(
                Path::new("full.html").to_owned(),
                headers,
                "<p>example page body</p>".to_string(),
            )
        }

        fn render(page: &Page) -> String {
            render_page(page, &Blocks::default())
        }

        fn render_with(page: &Page, blocks: impl IntoIterator<Item = (BlockKind, &'static str)>) -> String {
            let mut collected = Blocks::default();
            for (kind, content) in blocks {
                collected.insert(Block::new(kind, content.to_owned()).expect("block content to be non-empty"));
            }

            render_page(page, &collected)
        }

        fn page_headers() -> PageHeaders {
            PageHeadersBuilder {
                title: Some("Page"),
                is_post: Some(false),
                ..Default::default()
            }
            .build()
            .unwrap()
        }

        fn headers_full() -> PageHeaders {
            PageHeaders {
                title: "Example Post".to_string(),
                description: Some("An example post".to_string()),
                css_classes: Some("post".to_string()),
                css_stylesheet_path: Some("/styles/example.css".to_string()),
                is_post: true,
                include_header: true,
                include_footer: true,
                include_title: true,
                include_styles: true,
            }
        }

        fn diff(actual: &str, expected: &str) -> Result<String, fmt::Error> {
            let act_len = actual.lines().count();
            let exp_len = expected.lines().count();

            let start = actual.lines().zip(expected.lines()).take_while(|(a, e)| a == e).count();
            let end = actual
                .lines()
                .rev()
                .zip(expected.lines().rev())
                .take_while(|(e, a)| a == e)
                .count()
                .min(act_len.min(exp_len) - start);

            let mut out = String::new();

            writeln!(out, "--- expected")?;
            writeln!(out, "+++ actual")?;
            for (i, l) in expected.lines().enumerate().skip(start).take(exp_len - start - end) {
                writeln!(out, "-{:>4}   {l}", i + 1)?;
            }
            for (i, l) in actual.lines().enumerate().skip(start).take(act_len - start - end) {
                writeln!(out, "+{:>4}   {l}", i + 1)?;
            }

            Ok(out)
        }

        fn compare_page_snapshot(page_name: &str, actual: &str) {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
            let path = dir.join(page_name);

            if env::var_os("WEBRITE_UPDATE_SNAPSHOTS").is_some() {
                fs::create_dir_all(&dir).unwrap_or_else(|_| panic!("Directory {} to be createable", dir.display()));
                fs::write(&path, actual).unwrap_or_else(|_| panic!("Snapshot to be writeable to {}", path.display()));
            }

            let expected = fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!(
                    "cannot read snapshot {}: {e}\nrun `WEBRITE_UPDATE_SNAPSHOTS=1 cargo test` to create it",
                    path.display()
                );
            });

            if actual == expected {
                return;
            }

            panic!(
                "snapshot mismatch: {}\n{}run `WEBRITE_UPDATE_SNAPSHOTS=1 cargo test` to update",
                path.display(),
                diff(actual, &expected).unwrap(),
            );
        }

        #[test]
        fn full_page() {
            compare_page_snapshot("full.html", &render(&default_page(headers_full())));
        }

        #[test]
        fn minimal_page() {
            let headers_minimal = PageHeadersBuilder {
                title: Some("Minimal"),
                is_post: Some(false),
                ..Default::default() // 1. default styles NOT included
                                     // 2. NO post title header included
                                     // 3. content NOT wrapped in article tag
            }
            .build()
            .unwrap();

            compare_page_snapshot("minimal.html", &render(&default_page(headers_minimal)));
        }

        #[test]
        fn escapes_title() {
            let headers = PageHeadersBuilder {
                title: Some("Title with escaped <, >, &, \" and '"),
                is_post: Some(true),
                description: Some("Post title escaped"),
                ..Default::default()
            }
            .build()
            .unwrap();

            let page = render(&default_page(headers));

            assert!(
                page.contains("<title>Title with escaped &lt;, &gt;, &amp;, &quot; and &#39;</title>"),
                "got {page}"
            );
            assert!(
                page.contains("<h1>Title with escaped &lt;, &gt;, &amp;, &quot; and &#39;</h1>"),
                "got {page}"
            );
        }

        #[test]
        fn escapes_classes() {
            let headers = PageHeadersBuilder {
                title: Some("Test"),
                class: Some("class with escaped <, >, &, \" and '"),
                is_post: Some(false),
                ..Default::default()
            }
            .build()
            .unwrap();

            let page = render(&default_page(headers));

            assert!(
                page.contains(&format!(
                    "<div id=\"{DEFAULT_WRAP_ID}\" class=\"class with escaped &lt;, &gt;, &amp;, &quot; and &#39;\">"
                )),
                "got {page}"
            );
        }

        #[test]
        fn escapes_stylesheet() {
            let headers = PageHeadersBuilder {
                title: Some("Test"),
                stylesheet: Some("/styles/escaped <, >, &, \" and '.css"),
                is_post: Some(false),
                ..Default::default()
            }
            .build()
            .unwrap();

            let page = render(&default_page(headers));

            assert!(
                page.contains(
                    "<link href=\"/styles/escaped &lt;, &gt;, &amp;, &quot; and &#39;.css\" rel=\"stylesheet\"/>"
                ),
                "got {page}"
            );
        }

        #[test]
        fn escapes_description() {
            let headers = PageHeadersBuilder {
                title: Some("Test"),
                description: Some("Description with escaped <, >, &, \" and '"),
                is_post: Some(true),
                ..Default::default()
            }
            .build()
            .unwrap();

            let page = render(&default_page(headers));

            assert!(
                page.contains(
                    "<meta name=\"description\" content=\"Description with escaped &lt;, &gt;, &amp;, &quot; and &#39;\">"
                ),
                "got {page}"
            );
        }

        #[test]
        fn minimal_post() {
            let headers_minimal_post = PageHeadersBuilder {
                title: Some("Minimal Post"),
                description: Some("Minimal post with description"),
                is_post: Some(true),
                ..Default::default()
            }
            .build()
            .unwrap();

            compare_page_snapshot("minimal_post.html", &render(&default_page(headers_minimal_post)));
        }

        #[test]
        fn post_can_omit_title_and_default_styles() {
            let headers = PageHeadersBuilder {
                title: Some("Post"),
                description: Some("Post without title and default styles"),
                stylesheet: Some("/styles/post.css"),
                is_post: Some(true),
                include_title: Some(false),
                include_styles: Some(false),
                ..Default::default()
            }
            .build()
            .unwrap();

            let page = render(&default_page(headers));

            assert!(
                page.contains("<article>\n<p>example page body</p>\n</article>\n"),
                "got {page}"
            );
            assert!(!page.contains("<h1>"), "got {page}");
            assert!(
                !page.contains(&format!("<link href=\"{DEFAULT_STYLESHEET}\" rel=\"stylesheet\"/>")),
                "got {page}"
            );
            // custom stylesheet does not depend on the default one
            assert!(
                page.contains("<link href=\"/styles/post.css\" rel=\"stylesheet\"/>"),
                "got {page}"
            );
        }

        #[test]
        fn page_can_include_title_and_default_styles() {
            let headers = PageHeadersBuilder {
                title: Some("Page"),
                is_post: Some(false),
                include_title: Some(true),
                include_styles: Some(true),
                ..Default::default()
            }
            .build()
            .unwrap();

            let page = render(&default_page(headers));

            assert!(page.contains("<h1>Page</h1>\n<p>example page body</p>\n"), "got {page}");
            assert!(!page.contains("<article>"), "got {page}");
            assert!(
                page.contains(&format!("<link href=\"{DEFAULT_STYLESHEET}\" rel=\"stylesheet\"/>")),
                "got {page}"
            );
        }

        #[test]
        fn omits_header_and_footer() {
            let headers = PageHeadersBuilder {
                title: Some("Page"),
                is_post: Some(false),
                include_header: Some(false),
                include_footer: Some(false),
                ..Default::default()
            }
            .build()
            .unwrap();

            let page = render(&default_page(headers));

            // nothing may be rendered between the wrapper and <main>
            assert!(
                page.contains(&format!("<div id=\"{DEFAULT_WRAP_ID}\">\n<main>\n")),
                "got {page}"
            );
            assert!(page.contains("</main>\n</div>\n"), "got {page}");
        }

        #[test]
        fn includes_header_and_footer_blocks() {
            let headers = PageHeadersBuilder {
                title: Some("Page"),
                is_post: Some(false),
                include_header: Some(true),
                include_footer: Some(true),
                ..Default::default()
            }
            .build()
            .unwrap();

            let page = render_with(
                &default_page(headers),
                [
                    (BlockKind::Header, "<nav>site nav</nav>\n"),
                    (BlockKind::Footer, "<footer>site footer</footer>\n"),
                ],
            );

            assert!(
                page.contains(&format!(
                    "<div id=\"{DEFAULT_WRAP_ID}\">\n<nav>site nav</nav>\n<main>\n"
                )),
                "got {page}"
            );
            assert!(
                page.contains("</main>\n<footer>site footer</footer>\n</div>\n"),
                "got {page}"
            );
        }

        #[test]
        fn footer_block_does_not_depend_on_the_header() {
            let headers = PageHeadersBuilder {
                title: Some("Page"),
                is_post: Some(false),
                include_header: Some(false),
                include_footer: Some(true),
                ..Default::default()
            }
            .build()
            .unwrap();

            let page = render_with(
                &default_page(headers),
                [
                    (BlockKind::Header, "<nav>site nav</nav>\n"),
                    (BlockKind::Footer, "<footer>site footer</footer>\n"),
                ],
            );

            assert!(!page.contains("<nav>site nav</nav>"), "got {page}");
            assert!(
                page.contains("</main>\n<footer>site footer</footer>\n</div>\n"),
                "got {page}"
            );
        }

        #[test]
        fn head_block_extends_the_defaults() {
            let page = render_with(
                &default_page(page_headers()),
                [(
                    BlockKind::Head,
                    "<meta name=\"viewport\" content=\"width=device-width\">\n",
                )],
            );

            // the defaults survive ...
            assert!(page.contains("<title>Page</title>"), "got {page}");
            assert!(
                page.contains("<link href=\"/feed.atom\" type=\"application/atom+xml\" rel=\"alternate\"/>"),
                "got {page}"
            );
            // ... and the block is added to them, verbatim
            assert!(
                page.contains("<meta name=\"viewport\" content=\"width=device-width\">"),
                "got {page}"
            );
        }

        #[test]
        fn head_block_can_override_the_defaults() {
            // the block closes the head, so a tag it repeats takes precedence
            let page = render_with(
                &default_page(page_headers()),
                [(
                    BlockKind::Head,
                    "<link href=\"/styles/override.css\" rel=\"stylesheet\"/>\n",
                )],
            );

            assert!(
                page.contains(
                    "<link href=\"/feed.atom\" type=\"application/atom+xml\" rel=\"alternate\"/>\n\
                     <link href=\"/styles/override.css\" rel=\"stylesheet\"/>\n\
                     </head>"
                ),
                "got {page}"
            );
        }
    }
}
