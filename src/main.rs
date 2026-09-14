use std::fmt;
use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::str::FromStr;

static OUT_DIR: &str = "out";
static IN_DIR: &str = "in";
static BLOCK_DIR: &str = "blocks";
static HEADER_SEP: &str = "---";
static DRAFT_DIR: &str = "drafts";

#[derive(Debug)]
pub enum Error {
    Io {
        path: PathBuf,
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

trait WithPath: std::error::Error + 'static {
    fn with_path(self, path: PathBuf) -> Error;
}

trait PathContext<T> {
    fn at_path(self, path: impl Into<PathBuf>) -> Result<T, Error>;
}

impl<T, E: WithPath> PathContext<T> for Result<T, E> {
    fn at_path(self, path: impl Into<PathBuf>) -> Result<T, Error> {
        self.map_err(|e| e.with_path(path.into()))
    }
}

impl WithPath for io::Error {
    fn with_path(self, path: PathBuf) -> Error {
        Error::Io { path, source: self }
    }
}

impl WithPath for ReadPageError {
    fn with_path(self, path: PathBuf) -> Error {
        Error::ReadPageHeader { path, source: self }
    }
}

impl WithPath for ParsePageHeaderError {
    fn with_path(self, path: PathBuf) -> Error {
        Error::ParsePageHeader { path, source: self }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, .. } | Self::ReadPageHeader { path, .. } => {
                write!(f, "{}", path.display())
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

impl std::error::Error for ReadPageError {}

impl std::error::Error for ParsePageHeaderError {}

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

#[derive(Debug)]
pub struct ParsePageHeaderError {
    line: usize,
    kind: ParsePageHeaderErrorKind,
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
    include_date: Option<bool>,
    include_styles: Option<bool>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
struct PageHeaders<'a> {
    title: &'a str,
    description: Option<&'a str>,
    class: Option<&'a str>,
    stylesheet: Option<&'a str>,
    is_post: bool,
    include_header: bool,
    include_footer: bool,
    include_title: bool,
    include_date: bool,
    include_styles: bool,
}

impl<'a> PageHeadersBuilder<'a> {
    fn set_once<T>(field: &mut Option<T>, key: &HeaderKey, val: T) -> Result<(), ParsePageHeaderErrorKind> {
        if field.is_some() {
            return Err(ParsePageHeaderErrorKind::DuplicateKey(key.to_string()));
        }
        *field = Some(val);
        Ok(())
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
            HeaderKey::IncludeDate => Self::set_once(&mut self.include_date, key, Self::parse_bool(val)?)?,
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
            include_date: self.include_date.unwrap_or(is_post),
            include_styles: self.include_styles.unwrap_or(is_post),
        })
    }

    fn parse_bool(val: &str) -> Result<bool, ParsePageHeaderErrorKind> {
        match val.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => Ok(true),
            "n" | "no" => Ok(false),
            _ => Err(ParsePageHeaderErrorKind::InvalidBool(val.trim().to_owned())),
        }
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
    IncludeDate,
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
            Self::IncludeDate => "include_date",
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
            "include_date" => Self::IncludeDate,
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

#[derive(Debug)]
struct Page<'a> {
    path: &'a Path,
    headers: PageHeaders<'a>,
    body: &'a str,
}

// impl Page<'_> {
//     fn to_url(&self) -> String {
//         let parts: Vec<_> = self
//             .site_path
//             .with_extension("html")
//             .components()
//             .map(|c| c.as_os_str().to_string_lossy().into_owned())
//             .collect();
//         format!("/{}", parts.join("/"))
//     }
// }

fn split_page(page_str: &str) -> Result<(&str, &str), ReadPageError> {
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

fn parse_header(header_block: &str) -> Result<PageHeaders<'_>, ParsePageHeaderError> {
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

fn write_page(page: &Page) -> String {
    let out_path = Path::new(OUT_DIR).join(page.path);
    // TODO: replace with writing to out file
    out_path.to_string_lossy().into_owned()
}

fn process_file(input_path: &Path) -> Result<(), Error> {
    let mut reader = io::BufReader::new(fs::File::open(input_path).at_path(input_path)?);
    let mut page_string = String::new();
    reader.read_to_string(&mut page_string).at_path(input_path)?;

    let (headers, body) = split_page(&page_string).at_path(input_path)?;
    let headers = parse_header(headers).at_path(input_path)?;

    let rel_path = input_path
        .strip_prefix(IN_DIR)
        .expect("file paths descend from the input directory");

    let page = Page {
        path: rel_path,
        headers,
        body,
    };
    eprintln!("{}", write_page(&page));

    Ok(())
}

fn process_dir(input_dir: &Path) -> Result<(), Error> {
    let mut stack = vec![input_dir.to_path_buf()];

    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).at_path(&dir)? {
            let entry = entry.at_path(&dir)?;
            let path = entry.path();
            let typ = entry.file_type().at_path(&path)?;

            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }

            let rel_path = path
                .strip_prefix(input_dir)
                .expect("paths descend from the input directory");
            let out_path = Path::new(OUT_DIR).join(rel_path);

            if typ.is_dir() {
                let dir_name = entry.file_name();
                if dir_name == BLOCK_DIR || dir_name == DRAFT_DIR {
                    continue;
                }
                fs::create_dir_all(&out_path).at_path(&out_path)?;
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "htm") {
                process_file(&path)?;
            } else {
                fs::copy(&path, &out_path).at_path(&out_path)?;
            }
        }
    }

    Ok(())
}

fn main() -> process::ExitCode {
    let content_dir = Path::new(&IN_DIR);
    match process_dir(content_dir) {
        Ok(()) => process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            if let Some(source) = std::error::Error::source(&e) {
                eprintln!("    {source}");
            }
            process::ExitCode::FAILURE
        }
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
            <p>Hello, World!</p>"};

            let (header, content) = split_page(page).expect("page to be split by the '---' separator");
            assert_eq!(header, "title: Example\ndescription: example post with separator");
            assert_eq!(content, "<p>Hello, World!</p>");
        }

        #[test]
        fn splits_by_newlines() {
            let page = indoc! {"
            title: Example
            description: example post with newlines



            <p>Hello, World!</p>"};

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


            <p>Hello, World!</p>"};

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
            <p>Hello, World!</p>"};

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
            <p>Hello, World!</p>"};

            let r = split_page(page);
            assert!(matches!(r, Err(ReadPageError::MissingHeader)), "got {r:?}");
        }

        #[test]
        fn rejects_empty_header_without_separator() {
            let page = indoc! {"

            <p>Hello, World!</p>"};

            let r = split_page(page);
            assert!(matches!(r, Err(ReadPageError::MissingHeader)), "got {r:?}");
        }

        #[test]
        fn rejects_empty_content_with_separator() {
            let page = indoc! {"
            title: Example
            description: example post with separator and newlines
            ---"
            };

            let r = split_page(page);
            assert!(matches!(r, Err(ReadPageError::MissingContent)), "got {r:?}");
        }

        #[test]
        fn rejects_empty_content_without_separator() {
            let page = indoc! {"
            title: Example
            description: example post with separator and newlines

            "};

            let r = split_page(page);
            assert!(matches!(r, Err(ReadPageError::MissingContent)), "got {r:?}");
        }
    }
}
