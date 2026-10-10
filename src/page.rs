use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

const HEADER_SEP: &str = "---";

trait KeyValueBlockBuilder<'a>: Default {
    type Key: FromStr;
    type Output;

    fn set(&mut self, key: &Self::Key, val: &'a str) -> Result<(), ParseKeyValueBlockKind>;

    fn build(self) -> Result<Self::Output, ParseKeyValueBlockKind>;
}

trait FromKeyValueBlock<'a>: Sized {
    type Builder: KeyValueBlockBuilder<'a, Output = Self>;
}

fn parse_kv_block<'a, T>(header_block: &'a str) -> Result<T, ParseKeyValueBlockError>
where
    T: FromKeyValueBlock<'a>,
{
    let mut builder = <<T as FromKeyValueBlock<'a>>::Builder as Default>::default();
    let mut ln = 0;

    for (i, line) in header_block.lines().enumerate() {
        ln = i + 1;

        let Some((key, val)) = parse_kv_line(line).map_err(|k| ParseKeyValueBlockKind::from(k).at(ln))? else {
            continue;
        };

        let Ok(key) = key.parse::<<T::Builder as KeyValueBlockBuilder<'a>>::Key>() else {
            return Err(ParseKeyValueBlockKind::UnknownKey(key.to_owned()).at(ln));
        };

        builder.set(&key, val).map_err(|k| k.at(ln))?;
    }

    builder.build().map_err(|k| k.at(ln))
}

fn parse_kv_line(line: &str) -> Result<Option<(&str, &str)>, ParseKeyValuePairError> {
    if line.trim().is_empty() {
        return Ok(None);
    }

    let Some((key, val)) = line.split_once(':') else {
        return Err(ParseKeyValuePairError::MissingColon);
    };

    let key = key.trim();
    let val = val.trim();

    if val.is_empty() {
        return Err(ParseKeyValuePairError::MissingValue(key.to_owned()));
    }

    Ok(Some((key, val)))
}

#[derive(Debug)]
pub struct Page<H = PageHeaders> {
    path: PathBuf,
    headers: H,
    body: String,
}

#[derive(Debug)]
pub enum PageHeaders {
    NonPost(NonPostHeaders),
    Post(PostHeaders),
}

#[derive(Debug)]
pub struct NonPostHeaders {
    common: CommonHeaders,

    // non-posts don't require special treatmeant
    description: Option<String>,
    created_at: Option<String>,
}

#[derive(Debug)]
pub struct PostHeaders {
    common: CommonHeaders,

    // while posts do
    description: String,
    created_at: String,
}

pub type Post = Page<PostHeaders>;

// Forward common getters without exposing the header representation.
#[cfg_attr(not(test), expect(dead_code, reason = "Some getters not used yet"))]
impl PageHeaders {
    const fn common(&self) -> &CommonHeaders {
        match self {
            Self::Post(headers) => &headers.common,
            Self::NonPost(headers) => &headers.common,
        }
    }

    pub const fn is_post(&self) -> bool {
        matches!(self, Self::Post(_))
    }

    pub fn description(&self) -> Option<&str> {
        match self {
            Self::Post(headers) => Some(headers.description()),
            Self::NonPost(headers) => headers.description(),
        }
    }

    pub fn created_at(&self) -> Option<&str> {
        match self {
            Self::Post(headers) => Some(headers.created_at()),
            Self::NonPost(headers) => headers.created_at(),
        }
    }

    // common headers
    pub fn title(&self) -> &str {
        self.common().title()
    }
    pub fn css_classes(&self) -> Option<&str> {
        self.common().css_classes()
    }
    pub fn css_stylesheet(&self) -> Option<&str> {
        self.common().css_stylesheet()
    }
    pub const fn include_header(&self) -> bool {
        self.common().include_header()
    }
    pub const fn include_footer(&self) -> bool {
        self.common().include_footer()
    }
    pub const fn include_title(&self) -> bool {
        self.common().include_title()
    }
    pub const fn include_styles(&self) -> bool {
        self.common().include_styles()
    }
}

// Forward common getters without exposing the header representation.
#[expect(dead_code, reason = "Some getters not used yet")]
impl PostHeaders {
    pub fn description(&self) -> &str {
        self.description.as_ref()
    }
    pub const fn created_at(&self) -> &str {
        self.created_at.as_str()
    }

    // common headers
    pub fn title(&self) -> &str {
        self.common.title()
    }
    pub fn css_classes(&self) -> Option<&str> {
        self.common.css_classes()
    }
    pub fn css_stylesheet(&self) -> Option<&str> {
        self.common.css_stylesheet()
    }
    pub const fn include_header(&self) -> bool {
        self.common.include_header()
    }
    pub const fn include_footer(&self) -> bool {
        self.common.include_footer()
    }
    pub const fn include_title(&self) -> bool {
        self.common.include_title()
    }
    pub const fn include_styles(&self) -> bool {
        self.common.include_styles()
    }
}

// Forward common getters without exposing the header representation.
#[expect(dead_code, reason = "Some getters not used yet")]
impl NonPostHeaders {
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    pub fn created_at(&self) -> Option<&str> {
        self.created_at.as_deref()
    }

    // common headers
    pub fn title(&self) -> &str {
        self.common.title()
    }
    pub fn css_classes(&self) -> Option<&str> {
        self.common.css_classes()
    }
    pub fn css_stylesheet(&self) -> Option<&str> {
        self.common.css_stylesheet()
    }
    pub const fn include_header(&self) -> bool {
        self.common.include_header()
    }
    pub const fn include_footer(&self) -> bool {
        self.common.include_footer()
    }
    pub const fn include_title(&self) -> bool {
        self.common.include_title()
    }
    pub const fn include_styles(&self) -> bool {
        self.common.include_styles()
    }
}

impl<H> Page<H> {
    pub const fn new(path: PathBuf, headers: H, body: String) -> Self {
        Self { path, headers, body }
    }

    pub const fn headers(&self) -> &H {
        &self.headers
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn body(&self) -> &str {
        &self.body
    }
}

impl Page<PageHeaders> {
    pub fn into_post(self) -> Option<Post> {
        let Self { path, headers, body } = self;
        let PageHeaders::Post(headers) = headers else {
            return None;
        };

        Some(Page::new(path, headers, body))
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
struct CommonHeaders {
    title: String,

    css_classes: Option<String>,
    css_stylesheet: Option<String>,

    include_header: bool,
    include_footer: bool,
    include_title: bool,
    include_styles: bool,
}

// getters-only impl
impl CommonHeaders {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn css_classes(&self) -> Option<&str> {
        self.css_classes.as_deref()
    }

    pub fn css_stylesheet(&self) -> Option<&str> {
        self.css_stylesheet.as_deref()
    }

    pub const fn include_header(&self) -> bool {
        self.include_header
    }

    pub const fn include_footer(&self) -> bool {
        self.include_footer
    }

    pub const fn include_title(&self) -> bool {
        self.include_title
    }

    pub const fn include_styles(&self) -> bool {
        self.include_styles
    }
}

impl<'a> FromKeyValueBlock<'a> for PageHeaders {
    type Builder = PageHeadersBuilder<'a>;
}

enum HeaderKey {
    Title,
    Description,
    CreatedAt,
    CSSClasses,
    CSSStylesheet,
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

            Self::CreatedAt => "created_at",

            Self::CSSClasses => "css_classes",
            Self::CSSStylesheet => "css_stylesheet",

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

            "created_at" => Self::CreatedAt,

            "css_classes" => Self::CSSClasses,
            "css_stylesheet" => Self::CSSStylesheet,

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

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default)]
struct PageHeadersBuilder<'a> {
    title: Option<&'a str>,
    description: Option<&'a str>,
    created_at: Option<&'a str>,
    css_classes: Option<&'a str>,
    css_stylesheet: Option<&'a str>,
    is_post: Option<bool>,
    include_header: Option<bool>,
    include_footer: Option<bool>,
    include_title: Option<bool>,
    include_styles: Option<bool>,
}

impl<'a> PageHeadersBuilder<'a> {
    fn set_once<T>(field: &mut Option<T>, key: &HeaderKey, val: T) -> Result<(), ParseKeyValueBlockKind> {
        if field.is_some() {
            return Err(ParseKeyValueBlockKind::DuplicateKey(key.to_string()));
        }
        *field = Some(val);
        Ok(())
    }

    fn parse_bool(val: &str) -> Result<bool, ParseKeyValueBlockKind> {
        match val.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => Ok(true),
            "n" | "no" => Ok(false),
            _ => Err(ParseKeyValueBlockKind::InvalidBool(val.trim().to_owned())),
        }
    }

    fn set(&mut self, key: &HeaderKey, val: &'a str) -> Result<(), ParseKeyValueBlockKind> {
        match key {
            // string headers
            HeaderKey::Title => Self::set_once(&mut self.title, key, val)?,
            HeaderKey::Description => Self::set_once(&mut self.description, key, val)?,
            HeaderKey::CreatedAt => Self::set_once(&mut self.created_at, key, val)?,
            HeaderKey::CSSClasses => Self::set_once(&mut self.css_classes, key, val)?,
            HeaderKey::CSSStylesheet => Self::set_once(&mut self.css_stylesheet, key, val)?,

            // boolean headers
            HeaderKey::IsPost => Self::set_once(&mut self.is_post, key, Self::parse_bool(val)?)?,
            HeaderKey::IncludeHeader => Self::set_once(&mut self.include_header, key, Self::parse_bool(val)?)?,
            HeaderKey::IncludeFooter => Self::set_once(&mut self.include_footer, key, Self::parse_bool(val)?)?,
            HeaderKey::IncludeTitle => Self::set_once(&mut self.include_title, key, Self::parse_bool(val)?)?,
            HeaderKey::IncludeStyles => Self::set_once(&mut self.include_styles, key, Self::parse_bool(val)?)?,
        }
        Ok(())
    }

    fn build(self) -> Result<PageHeaders, ParseKeyValueBlockKind> {
        let is_post = self.is_post.unwrap_or(true);
        let title = self
            .title
            .ok_or_else(|| ParseKeyValueBlockKind::MissingRequiredKey(HeaderKey::Title.to_string()))?;
        let common = CommonHeaders {
            title: title.to_string(),
            css_classes: self.css_classes.map(str::to_owned),
            css_stylesheet: self.css_stylesheet.map(str::to_owned),
            include_header: self.include_header.unwrap_or(true),
            include_footer: self.include_footer.unwrap_or(true),

            // post overrides
            include_title: self.include_title.unwrap_or(is_post),
            include_styles: self.include_styles.unwrap_or(is_post),
        };

        if is_post {
            let description = self
                .description
                .ok_or_else(|| ParseKeyValueBlockKind::MissingRequiredKey(HeaderKey::Description.to_string()))?;

            let created_at = self
                .created_at
                .ok_or_else(|| ParseKeyValueBlockKind::MissingRequiredKey(HeaderKey::CreatedAt.to_string()))?;

            Ok(PageHeaders::Post(PostHeaders {
                common,
                description: description.to_owned(),
                created_at: created_at.to_owned(),
            }))
        } else {
            Ok(PageHeaders::NonPost(NonPostHeaders {
                common,
                description: self.description.map(str::to_owned),
                created_at: self.created_at.map(str::to_owned),
            }))
        }
    }
}

impl<'a> KeyValueBlockBuilder<'a> for PageHeadersBuilder<'a> {
    type Key = HeaderKey;
    type Output = PageHeaders;

    fn set(&mut self, key: &Self::Key, val: &'a str) -> Result<(), ParseKeyValueBlockKind> {
        PageHeadersBuilder::set(self, key, val)
    }

    fn build(self) -> Result<Self::Output, ParseKeyValueBlockKind> {
        PageHeadersBuilder::build(self)
    }
}

pub fn parse_headers(header_str: &str) -> Result<PageHeaders, ParseKeyValueBlockError> {
    parse_kv_block(header_str)
}

pub fn parse_config(config_str: &'_ str) -> Result<SiteConfig<'_>, ParseKeyValueBlockError> {
    parse_kv_block(config_str)
}

// getters-only
pub struct BuildConfig<'a> {
    // build
    r#in: &'a str,
    out: &'a str,
    blocks: &'a str,
    drafts: &'a str,
}

// getters-only
impl BuildConfig<'_> {
    pub const fn r#in(&self) -> &str {
        self.r#in
    }

    pub const fn out(&self) -> &str {
        self.out
    }

    pub const fn blocks(&self) -> &str {
        self.blocks
    }

    pub const fn drafts(&self) -> &str {
        self.drafts
    }
}

impl<'a> Default for BuildConfig<'a> {
    fn default() -> Self {
        Self {
            r#in: "in",
            out: "out",
            blocks: "blocks",
            drafts: "drafts",
        }
    }
}

pub struct StyleConfig<'a> {
    // css
    stylesheet: &'a str,
    wrapper_id: &'a str,
}

// getters-only
impl StyleConfig<'_> {
    pub const fn stylesheet(&self) -> &str {
        self.stylesheet
    }

    pub const fn wrapper_id(&self) -> &str {
        self.wrapper_id
    }
}

impl<'a> Default for StyleConfig<'a> {
    fn default() -> Self {
        Self {
            stylesheet: "style.css",
            wrapper_id: "content",
        }
    }
}

pub struct FeedConfig<'a> {
    // feed
    author: &'a str,
    host: &'a str,
    title: &'a str,
    url: &'a str,
}

// getters only
impl FeedConfig<'_> {
    pub const fn author(&self) -> &str {
        self.author
    }

    pub const fn host(&self) -> &str {
        self.host
    }

    pub const fn title(&self) -> &str {
        self.title
    }

    pub const fn url(&self) -> &str {
        self.url
    }
}

pub struct SiteConfig<'a> {
    build: BuildConfig<'a>,
    styles: StyleConfig<'a>,
    feed: Option<FeedConfig<'a>>,
}

impl<'a> SiteConfig<'a> {
    pub const fn build(&self) -> &BuildConfig<'a> {
        &self.build
    }

    pub const fn styles(&self) -> &StyleConfig<'a> {
        &self.styles
    }

    pub const fn feed(&self) -> Option<&FeedConfig<'a>> {
        self.feed.as_ref()
    }
}

impl Default for SiteConfig<'_> {
    fn default() -> Self {
        SiteConfigBuilder::default().build()
    }
}

#[derive(Default)]
pub struct SiteConfigBuilder<'a> {
    build: Option<BuildConfig<'a>>,
    styles: Option<StyleConfig<'a>>,
    feed: Option<FeedConfig<'a>>,
}

impl<'a> SiteConfigBuilder<'a> {
    pub fn build(self) -> SiteConfig<'a> {
        SiteConfig {
            build: self.build.unwrap_or_default(),
            styles: self.styles.unwrap_or_default(),
            feed: self.feed,
        }
    }
}

impl<'a> FromKeyValueBlock<'a> for SiteConfig<'a> {
    type Builder = SiteConfigBlockBuilder<'a>;
}

enum SiteConfigKey {
    In,
    Out,
    Blocks,
    Drafts,
    Stylesheet,
    WrapperId,
    Author,
    Host,
    Title,
    Url,
}

impl SiteConfigKey {
    const fn as_str(&self) -> &str {
        match self {
            Self::In => "in",
            Self::Out => "out",
            Self::Blocks => "blocks",
            Self::Drafts => "drafts",
            Self::Stylesheet => "stylesheet",
            Self::WrapperId => "wrapper_id",
            Self::Author => "author",
            Self::Host => "host",
            Self::Title => "title",
            Self::Url => "url",
        }
    }
}

impl FromStr for SiteConfigKey {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "in" => Self::In,
            "out" => Self::Out,
            "blocks" => Self::Blocks,
            "drafts" => Self::Drafts,
            "stylesheet" => Self::Stylesheet,
            "wrapper_id" => Self::WrapperId,
            "author" => Self::Author,
            "host" => Self::Host,
            "title" => Self::Title,
            "url" => Self::Url,
            _ => return Err(()),
        })
    }
}

impl std::fmt::Display for SiteConfigKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Default)]
struct SiteConfigBlockBuilder<'a> {
    r#in: Option<&'a str>,
    out: Option<&'a str>,
    blocks: Option<&'a str>,
    drafts: Option<&'a str>,
    stylesheet: Option<&'a str>,
    wrapper_id: Option<&'a str>,
    title: Option<&'a str>,
    author: Option<&'a str>,
    host: Option<&'a str>,
    url: Option<&'a str>,
}

impl<'a> SiteConfigBlockBuilder<'a> {
    fn set_once<T>(field: &mut Option<T>, key: &SiteConfigKey, val: T) -> Result<(), ParseKeyValueBlockKind> {
        if field.is_some() {
            return Err(ParseKeyValueBlockKind::DuplicateKey(key.to_string()));
        }
        *field = Some(val);
        Ok(())
    }

    fn set(&mut self, key: &SiteConfigKey, val: &'a str) -> Result<(), ParseKeyValueBlockKind> {
        match key {
            SiteConfigKey::In => Self::set_once(&mut self.r#in, key, val),
            SiteConfigKey::Out => Self::set_once(&mut self.out, key, val),
            SiteConfigKey::Blocks => Self::set_once(&mut self.blocks, key, val),
            SiteConfigKey::Drafts => Self::set_once(&mut self.drafts, key, val),
            SiteConfigKey::Stylesheet => Self::set_once(&mut self.stylesheet, key, val),
            SiteConfigKey::WrapperId => Self::set_once(&mut self.wrapper_id, key, val),

            SiteConfigKey::Author => Self::set_once(&mut self.author, key, val),
            SiteConfigKey::Host => Self::set_once(&mut self.host, key, val),
            SiteConfigKey::Title => Self::set_once(&mut self.title, key, val),
            SiteConfigKey::Url => Self::set_once(&mut self.url, key, val),
        }
    }

    fn build(self) -> Result<SiteConfig<'a>, ParseKeyValueBlockKind> {
        let build_defaults = BuildConfig::default();
        let style_defaults = StyleConfig::default();
        let feed = if [self.author, self.host, self.title, self.url]
            .iter()
            .any(Option::is_some)
        {
            Some(FeedConfig {
                author: self
                    .author
                    .ok_or_else(|| ParseKeyValueBlockKind::MissingRequiredKey(SiteConfigKey::Author.to_string()))?,
                host: self
                    .host
                    .ok_or_else(|| ParseKeyValueBlockKind::MissingRequiredKey(SiteConfigKey::Host.to_string()))?,
                title: self
                    .title
                    .ok_or_else(|| ParseKeyValueBlockKind::MissingRequiredKey(SiteConfigKey::Title.to_string()))?,
                url: self
                    .url
                    .ok_or_else(|| ParseKeyValueBlockKind::MissingRequiredKey(SiteConfigKey::Url.to_string()))?,
            })
        } else {
            None
        };

        Ok(SiteConfigBuilder {
            build: Some(BuildConfig {
                r#in: self.r#in.unwrap_or(build_defaults.r#in),
                out: self.out.unwrap_or(build_defaults.out),
                blocks: self.blocks.unwrap_or(build_defaults.blocks),
                drafts: self.drafts.unwrap_or(build_defaults.drafts),
            }),
            styles: Some(StyleConfig {
                stylesheet: self.stylesheet.unwrap_or(style_defaults.stylesheet),
                wrapper_id: self.wrapper_id.unwrap_or(style_defaults.wrapper_id),
            }),
            feed,
        }
        .build())
    }
}

impl<'a> KeyValueBlockBuilder<'a> for SiteConfigBlockBuilder<'a> {
    type Key = SiteConfigKey;
    type Output = SiteConfig<'a>;

    fn set(&mut self, key: &Self::Key, val: &'a str) -> Result<(), ParseKeyValueBlockKind> {
        SiteConfigBlockBuilder::set(self, key, val)
    }

    fn build(self) -> Result<Self::Output, ParseKeyValueBlockKind> {
        SiteConfigBlockBuilder::build(self)
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

fn render_head(page: &Page, blocks: &Blocks, styles: &str) -> String {
    let headers = &page.headers;
    let escaped_title = escape_html(headers.title());

    let description_meta = headers
        .description()
        .map(|d| format!("<meta name=\"description\" content=\"{}\">\n", escape_html(d)))
        .unwrap_or_default();

    let style_link = if headers.include_styles() {
        format!(
            "<link href=\"{}\" rel=\"stylesheet\" type=\"text/css\"/>\n",
            escape_html(styles)
        )
    } else {
        String::new()
    };

    let custom_style_link = headers.css_stylesheet().map_or_else(String::new, |s| {
        format!(
            "<link href=\"{}\" rel=\"stylesheet\" type=\"text/css\"/>\n",
            escape_html(s)
        )
    });

    let head_block = blocks.head.as_deref().unwrap_or("<!-- head -->\n");

    // The followign two tags are mandatory and so are emitted for every page:
    //
    //   1. <meta charset>  Leads the head. The encoding declaration has to sit within the first
    //                      1024 bytes, and has to come before a title or description in case they
    //                      hold non-ASCII characters.
    //
    //                      Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/meta#charset>
    //
    //   2. <meta viewport> Omitting it makes mobile browsers lay out the page at 980px and then
    //                      zoom out.
    //
    //                      Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/meta/name/viewport>
    //
    // NOTE: The head block is appended last and may repeat either one to override it.
    format!(
        "<head>\n\
             <meta charset=\"utf-8\"/>\n\
             <title>{escaped_title}</title>\n\
             {description_meta}\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"/>\n\
             {style_link}\
             {custom_style_link}\
             <link href=\"/feed.atom\" type=\"application/atom+xml\" rel=\"alternate\"/>\n\
             {head_block}\
         </head>\n\
         "
    )
}

fn render_content(page: &Page) -> String {
    let body = &page.body;
    let heading = if page.headers.include_title() {
        format!("<h1>{}</h1>\n", escape_html(page.headers.title()))
    } else {
        String::new()
    };

    let content = format!("{heading}{body}\n");

    if page.headers.is_post() {
        format!("<article>\n{content}</article>\n")
    } else {
        content
    }
}

pub fn render_page(page: &Page, blocks: &Blocks, config: &SiteConfig) -> String {
    let wrapper_id = escape_html(config.styles().wrapper_id());
    let headers = &page.headers;

    let class_attr = headers
        .css_classes()
        .map(|c| format!(" class=\"{}\"", escape_html(c)))
        .unwrap_or_default();

    let header = if headers.include_header() {
        blocks.header.as_deref().unwrap_or("<!-- header -->\n")
    } else {
        ""
    };

    let footer = if headers.include_footer() {
        blocks.footer.as_deref().unwrap_or("<!-- footer -->\n")
    } else {
        ""
    };

    let head = render_head(page, blocks, config.styles().stylesheet());
    let content = render_content(page);

    // Indentation is only for the convencience of the reader of this __source code__;
    // actual HTML output will be flat.
    format!(
        "<!DOCTYPE html>\n\
         <html lang=\"en\">\n\
             {head}\
             <body>\n\
                 <div id=\"{wrapper_id}\"{class_attr}>\n\
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

pub fn write_page(page: &Page, blocks: &Blocks, config: &SiteConfig) -> Result<(), Error> {
    let out_path = Path::new(config.build().out()).join(&page.path);

    fs::write(&out_path, render_page(page, blocks, config)).at_path(&out_path)
}

// pub fn build_feed(pages: Vec<Page>) -> Result<(), Error> {
//     Ok(())
// }

// fn render_feed(page: &Page) -> Result<(), Error> -> {
// }
//
// fn write_feed(page: &Page) -> Result<(), Error> -> {
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
    ReadPage {
        path: PathBuf,
        source: ReadPageError,
    },
    ParseKeyValueBlock {
        path: PathBuf,
        source: ParseKeyValueBlockError,
    },
    ReadBlock {
        path: PathBuf,
        source: ReadBlockError,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path: None, .. } => {
                write!(f, "I/O error")
            }
            Self::ParseKeyValueBlock { path, source } => {
                write!(f, "{}:{}", path.display(), source.line)
            }
            Self::Io { path: Some(path), .. } | Self::ReadPage { path, .. } | Self::ReadBlock { path, .. } => {
                write!(f, "{}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::ParseKeyValueBlock { source, .. } => Some(source),
            Self::ReadPage { source, .. } => Some(source),
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
        Error::ReadPage { path, source: self }
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
pub struct ParseKeyValueBlockError {
    line: usize,
    kind: ParseKeyValueBlockKind,
}

impl fmt::Display for ParseKeyValueBlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ParseKeyValueBlockKind::MissingColon => write!(f, "missing ':'"),
            ParseKeyValueBlockKind::MissingValue(k) => write!(f, "missing value for '{k}' header"),
            ParseKeyValueBlockKind::DuplicateKey(k) => write!(f, "duplicate header key '{k}'"),
            ParseKeyValueBlockKind::MissingRequiredKey(k) => write!(f, "missing required '{k}' header"),
            ParseKeyValueBlockKind::UnknownKey(k) => write!(f, "unknown header key '{k}'"),
            ParseKeyValueBlockKind::InvalidBool(k) => {
                write!(f, "valid values for '{k}' are 'yes' ('y') or 'no' ('n')")
            }
        }
    }
}

impl std::error::Error for ParseKeyValueBlockError {}

impl WithPath for ParseKeyValueBlockError {
    fn with_path(self, path: PathBuf) -> Error {
        Error::ParseKeyValueBlock { path, source: self }
    }
}

#[derive(Debug)]
enum ParseKeyValueBlockKind {
    MissingColon,
    MissingValue(String),
    DuplicateKey(String),
    MissingRequiredKey(String),
    UnknownKey(String),
    InvalidBool(String),
}

impl ParseKeyValueBlockKind {
    const fn at(self, line: usize) -> ParseKeyValueBlockError {
        ParseKeyValueBlockError { line, kind: self }
    }
}

#[derive(Debug)]
enum ParseKeyValuePairError {
    MissingColon,
    MissingValue(String),
}

impl From<ParseKeyValuePairError> for ParseKeyValueBlockKind {
    fn from(kind: ParseKeyValuePairError) -> Self {
        match kind {
            ParseKeyValuePairError::MissingColon => Self::MissingColon,
            ParseKeyValuePairError::MissingValue(k) => Self::MissingValue(k),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site_config() -> SiteConfig<'static> {
        SiteConfig {
            build: BuildConfig {
                r#in: "in",
                out: "out",
                blocks: "blocks",
                drafts: "drafts",
            },
            styles: StyleConfig {
                stylesheet: "/style.css",
                wrapper_id: "wrap",
            },
            feed: Some(FeedConfig {
                author: "Example Author",
                host: "example.com",
                title: "Example Site",
                url: "https://example.com",
            }),
        }
    }

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

    #[test]
    fn into_post_preserves_page_content() {
        let headers =
            parse_kv_block::<PageHeaders>("title: Example\ndescription: Example description\ncreated_at: 2026-06-26\n")
                .unwrap();
        let page = Page::new(PathBuf::from("posts/example.htm"), headers, "<p>Body</p>".to_owned());

        let post: Post = page.into_post().unwrap();

        assert_eq!(post.path(), Path::new("posts/example.htm"));
        assert_eq!(post.body(), "<p>Body</p>");
        assert_eq!(post.headers().title(), "Example");
        assert_eq!(post.headers().description(), "Example description");
        assert_eq!(post.headers().created_at(), "2026-06-26");
    }

    #[test]
    fn into_post_excludes_nonposts_with_dates() {
        let headers = parse_kv_block::<PageHeaders>(
            "title: Example\ndescription: Example description\ncreated_at: 2026-06-26\nis_post: no\n",
        )
        .unwrap();
        let page = Page::new(PathBuf::from("example.htm"), headers, "<p>Body</p>".to_owned());

        assert!(page.into_post().is_none());
    }

    mod post_sorting {
        use super::*;

        fn post(title: &str, created_at: &str) -> Post {
            let headers = PageHeadersBuilder {
                title: Some(title),
                description: Some("Example description"),
                created_at: Some(created_at),
                ..Default::default()
            }
            .build()
            .unwrap();

            Page::new(PathBuf::from("example.htm"), headers, "<p>Body</p>".to_owned())
                .into_post()
                .unwrap()
        }

        #[test]
        fn sorts_posts_by_creation_date() {
            let mut posts = [
                post("Latest", "2026-07-01"),
                post("Earliest", "2025-12-31"),
                post("Middle", "2026-01-01"),
            ];

            posts.sort_by(|a, b| a.headers().created_at().cmp(b.headers().created_at()));

            assert_eq!(
                posts.each_ref().map(|post| post.headers().title()),
                ["Earliest", "Middle", "Latest"]
            );
        }

        #[test]
        fn equal_creation_dates_preserve_post_order() {
            let mut posts = [
                post("First", "2026-07-01"),
                post("Second", "2026-07-01"),
                post("Third", "2026-07-01"),
            ];

            posts.sort_by(|a, b| a.headers().created_at().cmp(b.headers().created_at()));

            assert_eq!(
                posts.each_ref().map(|post| post.headers().title()),
                ["First", "Second", "Third"]
            );
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
                created_at: 2026-06-26
            "};

            let headers = parse_kv_block::<PageHeaders>(header_str).unwrap();

            let PageHeaders::Post(headers) = headers else {
                panic!("pages default to posts");
            };

            assert_eq!(headers.title(), "Example", "got {:?}", headers.title());
            assert_eq!(
                headers.description(),
                "example post with separator and newlines",
                "got {:?}",
                headers.description()
            );
            assert_eq!(headers.created_at(), "2026-06-26");
        }

        #[test]
        fn preserves_creation_dates_for_posts_and_nonposts() {
            for is_post in ["yes", "no"] {
                let header_str = format!(
                    "title: Example\ndescription: Example description\ncreated_at: 2026-06-26\nis_post: {is_post}\n"
                );
                let headers = parse_kv_block::<PageHeaders>(&header_str).unwrap();

                assert_eq!(headers.created_at(), Some("2026-06-26"));
            }
        }

        #[test]
        fn rejects_removed_update_date_header() {
            for is_post in ["yes", "no"] {
                let header_str = format!(
                    "title: Example\ndescription: Example description\ncreated_at: 2026-06-26\n\
                     updated_at: 2026-07-01\nis_post: {is_post}\n"
                );
                let headers = parse_kv_block::<PageHeaders>(&header_str);

                assert!(matches!(
                    headers,
                    Err(ParseKeyValueBlockError {
                        line: 4,
                        kind: ParseKeyValueBlockKind::UnknownKey(key),
                    }) if key == "updated_at"
                ));
            }
        }

        #[test]
        fn nonposts_can_omit_description_and_creation_date() {
            let headers = parse_kv_block::<PageHeaders>("title: Example\nis_post: no\n").unwrap();
            let PageHeaders::NonPost(headers) = headers else {
                panic!("is_post: no produces nonpost headers");
            };

            assert_eq!(headers.description(), None);
            assert_eq!(headers.created_at(), None);
            assert_eq!(headers.title(), "Example");
        }

        #[test]
        fn rejects_duplicate_creation_dates() {
            let headers =
                parse_kv_block::<PageHeaders>("title: Example\ncreated_at: 2026-07-01\ncreated_at: 2026-07-02\n");

            assert!(matches!(
                headers,
                Err(ParseKeyValueBlockError {
                    line: 3,
                    kind: ParseKeyValueBlockKind::DuplicateKey(key),
                }) if key == "created_at"
            ));
        }

        #[test]
        fn rejects_missing_title() {
            let header_str = indoc! {"
                description: example post without a title
            "};

            let headers = parse_kv_block::<PageHeaders>(header_str);

            assert!(
                matches!(
                    &headers,
                    Err(ParseKeyValueBlockError {
                        line: 1,
                        kind: ParseKeyValueBlockKind::MissingRequiredKey(k),
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

            let headers = parse_kv_block::<PageHeaders>(header_str);

            assert!(
                matches!(
                    &headers,
                    Err(ParseKeyValueBlockError {
                        line: 2,
                        kind: ParseKeyValueBlockKind::MissingRequiredKey(k),
                    }) if k == "description"
                ),
                "got {headers:?}"
            );
        }

        #[test]
        fn rejects_missing_creation_date_for_post() {
            let header_str = indoc! {"
                title: Example
                description: Example Post
                is_post: yes
            "};

            let headers = parse_kv_block::<PageHeaders>(header_str);

            assert!(
                matches!(
                    &headers,
                    Err(ParseKeyValueBlockError {
                        line: 3,
                        kind: ParseKeyValueBlockKind::MissingRequiredKey(k),
                    }) if k == "created_at"
                ),
                "got {headers:?}"
            );
        }

        #[test]
        fn skips_blank_lines() {
            let header_str =
                "title: Example\n\n \t \ndescription: example post with blank lines\ncreated_at: 2026-06-26\n";

            let headers = parse_kv_block::<PageHeaders>(header_str).unwrap();

            assert_eq!(headers.title(), "Example");
            assert_eq!(headers.description(), Some("example post with blank lines"));
        }

        #[test]
        fn trims_whitespace_around_keys_and_values() {
            let header_str = "  title  :   Example Page  \n\
                              \tdescription\t:\texample post with padded fields\t\n\
                              \t created_at \t:\t2026-06-26 \t\n";

            let headers = parse_kv_block::<PageHeaders>(header_str).unwrap();

            assert_eq!(headers.title(), "Example Page");
            assert_eq!(headers.description(), Some("example post with padded fields"));
            assert_eq!(headers.created_at(), Some("2026-06-26"));
        }

        #[test]
        fn splits_on_the_first_colon_only() {
            let header_str = indoc! {"
                  title: Example: A Post
                  description: see https://example.com for details
                  created_at: 2026-06-26
                  "};

            let headers = parse_kv_block::<PageHeaders>(header_str).unwrap();

            assert_eq!(headers.title(), "Example: A Post");
            assert_eq!(headers.description(), Some("see https://example.com for details"));
        }

        #[test]
        fn rejects_missing_colon() {
            let header_str = indoc! {"
                title Example
                description: Title header misses a colon
            "};

            let headers = parse_kv_block::<PageHeaders>(header_str);

            assert!(
                matches!(
                    headers,
                    Err(ParseKeyValueBlockError {
                        line: 1,
                        kind: ParseKeyValueBlockKind::MissingColon
                    })
                ),
                "got {headers:#?}"
            );
        }

        #[test]
        fn rejects_missing_value() {
            let header_str = "title:\n";

            let headers = parse_kv_block::<PageHeaders>(header_str);

            assert!(
                matches!(
                    &headers,
                    Err(ParseKeyValueBlockError {
                        line: 1,
                        kind: ParseKeyValueBlockKind::MissingValue(k),
                    }) if k == "title"
                ),
                "got {headers:?}"
            );
        }

        #[test]
        fn reports_missing_value_before_unknown_key() {
            let header_str = "subtitle:\n";

            let headers = parse_kv_block::<PageHeaders>(header_str);

            assert!(
                matches!(
                    &headers,
                    Err(ParseKeyValueBlockError {
                        line: 1,
                        kind: ParseKeyValueBlockKind::MissingValue(k),
                    }) if k == "subtitle"
                ),
                "got {headers:?}"
            );
        }

        #[test]
        fn rejects_unknown_key() {
            let header_str = indoc! {"
                title: Example
                subtitle: Our first example
                description: A post with a subtitle
            "};

            let headers = parse_kv_block::<PageHeaders>(header_str);

            assert!(
                matches!(
                    headers,
                    Err(ParseKeyValueBlockError {
                        line: 2,
                        kind: ParseKeyValueBlockKind::UnknownKey(ref key)
                    }) if key == "subtitle"
                ),
                "got {headers:#?}"
            );
        }

        #[test]
        fn rejects_empty_string_key() {
            let header_str = ": Example\n";

            let headers = parse_kv_block::<PageHeaders>(header_str);

            assert!(
                matches!(
                    &headers,
                    Err(ParseKeyValueBlockError {
                        line: 1,
                        kind: ParseKeyValueBlockKind::UnknownKey(k),
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

            let headers = parse_kv_block::<PageHeaders>(header_str);

            assert!(
                matches!(
                    &headers,
                    Err(ParseKeyValueBlockError {
                        line: 3,
                        kind: ParseKeyValueBlockKind::DuplicateKey(k),
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
                created_at: 2026-06-26
            "};

            let headers = parse_kv_block::<PageHeaders>(header_str).unwrap();

            assert_eq!(headers.css_classes(), None, "got {:?}", headers.css_classes());
            assert_eq!(headers.css_stylesheet(), None, "got {:?}", headers.css_stylesheet());

            // boolean fields
            assert!(headers.is_post(), "got {:?}", headers.is_post());
            assert!(headers.include_header(), "got {:?}", headers.include_header());
            assert!(headers.include_footer(), "got {:?}", headers.include_footer());
            assert!(headers.include_title(), "got {:?}", headers.include_title());
            assert!(headers.include_styles(), "got {:?}", headers.include_styles());
        }

        #[test]
        fn explicit_defaults_override() {
            let header_str = indoc! {"
                title: Example
                description: example page with every header set explicitly

                created_at: 2026-06-26

                css_classes: wide
                css_stylesheet: /style/page.css

                is_post: no
                include_header: no
                include_footer: no
                include_title: yes
                include_styles: yes
            "};

            let headers = parse_kv_block::<PageHeaders>(header_str).unwrap();

            assert_eq!(headers.created_at(), Some("2026-06-26"));

            assert_eq!(headers.css_classes(), Some("wide"));
            assert_eq!(headers.css_stylesheet(), Some("/style/page.css"));

            assert!(!headers.is_post(), "got {:?}", headers.is_post());

            // added even despite page not being a post
            assert!(headers.include_title(), "got {:?}", headers.include_title());
            assert!(headers.include_styles(), "got {:?}", headers.include_styles());

            // set by default
            assert!(!headers.include_header(), "got {:?}", headers.include_header());
            assert!(!headers.include_footer(), "got {:?}", headers.include_footer());
        }

        #[test]
        fn accepts_bool_aliases_case_insensitively() {
            let header_str = indoc! {"
                  title: Example
                  description: example post with assorted boolean spellings
                  created_at: 2026-06-26

                  is_post: YES
                  include_header: y

                  include_styles: NO
                  include_footer: No
                  include_title: n
                  "};

            let headers = parse_kv_block::<PageHeaders>(header_str).unwrap();

            assert!(headers.is_post(), "got {:?}", headers.is_post());
            assert!(headers.include_header(), "got {:?}", headers.include_header());
            assert!(!headers.include_footer(), "got {:?}", headers.include_footer());
            assert!(!headers.include_title(), "got {:?}", headers.include_title());
            assert!(!headers.include_styles(), "got {:?}", headers.include_styles());
        }
    }

    mod parse_config {
        use super::*;
        use indoc::indoc;

        const CONFIG: &str = indoc! {"
            in: content
            out: public
            blocks: fragments
            drafts: unpublished
            stylesheet: /assets/site.css
            wrapper_id: site-content
            author: Example Author
            host: example.com
            title: Example Site
            url: https://example.com
        "};

        #[test]
        fn absent_settings_use_defaults() {
            for config in [
                SiteConfig::default(),
                parse_config("").unwrap(),
                parse_config("\n  \n").unwrap(),
            ] {
                assert_eq!(config.build().r#in(), "in");
                assert_eq!(config.build().out(), "out");
                assert_eq!(config.build().blocks(), "blocks");
                assert_eq!(config.build().drafts(), "drafts");
                assert_eq!(config.styles().stylesheet(), "style.css");
                assert_eq!(config.styles().wrapper_id(), "content");
                assert!(config.feed().is_none());
            }
        }

        #[test]
        fn partial_settings_override_only_supplied_values() {
            let config = parse_config("out: public\nstylesheet: /assets/site.css\n").unwrap();

            assert_eq!(config.build().r#in(), "in");
            assert_eq!(config.build().out(), "public");
            assert_eq!(config.build().blocks(), "blocks");
            assert_eq!(config.build().drafts(), "drafts");
            assert_eq!(config.styles().stylesheet(), "/assets/site.css");
            assert_eq!(config.styles().wrapper_id(), "content");
            assert!(config.feed().is_none());
        }

        #[test]
        fn preserves_complete_config() {
            let config = parse_config(CONFIG).unwrap();

            assert_eq!(config.build().r#in(), "content");
            assert_eq!(config.build().out(), "public");
            assert_eq!(config.build().blocks(), "fragments");
            assert_eq!(config.build().drafts(), "unpublished");
            assert_eq!(config.styles().stylesheet(), "/assets/site.css");
            assert_eq!(config.styles().wrapper_id(), "site-content");
            let feed = config.feed().unwrap();
            assert_eq!(feed.author(), "Example Author");
            assert_eq!(feed.host(), "example.com");
            assert_eq!(feed.title(), "Example Site");
            assert_eq!(feed.url(), "https://example.com");
        }

        #[test]
        fn partial_feed_reports_missing_metadata() {
            for key in ["author", "host", "title", "url"] {
                let text = CONFIG
                    .lines()
                    .filter(|line| !line.starts_with(&format!("{key}:")))
                    .collect::<Vec<_>>()
                    .join("\n");

                assert!(matches!(
                    parse_config(&text),
                    Err(ParseKeyValueBlockError {
                        kind: ParseKeyValueBlockKind::MissingRequiredKey(missing),
                        ..
                    }) if missing == key
                ));
            }
        }

        #[test]
        fn defaults_do_not_hide_duplicate_settings() {
            assert!(matches!(
                parse_config("out: public\nout: other\n"),
                Err(ParseKeyValueBlockError {
                    line: 2,
                    kind: ParseKeyValueBlockKind::DuplicateKey(key),
                }) if key == "out"
            ));
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
            render_page(page, &Blocks::default(), &site_config())
        }

        fn render_with(page: &Page, blocks: impl IntoIterator<Item = (BlockKind, &'static str)>) -> String {
            let mut collected = Blocks::default();
            for (kind, content) in blocks {
                collected.insert(Block::new(kind, content.to_owned()).expect("block content to be non-empty"));
            }

            render_page(page, &collected, &site_config())
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
            PageHeaders::Post(PostHeaders {
                common: CommonHeaders {
                    title: "Example Post".to_string(),
                    css_classes: Some("post".to_string()),
                    css_stylesheet: Some("/styles/example.css".to_string()),
                    include_header: true,
                    include_footer: true,
                    include_title: true,
                    include_styles: true,
                },
                description: "An example post".to_string(),
                created_at: "2026-06-26".to_string(),
            })
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
                created_at: Some("2026-06-26"),
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
                css_classes: Some("class with escaped <, >, &, \" and '"),
                is_post: Some(false),
                ..Default::default()
            }
            .build()
            .unwrap();

            let page = render(&default_page(headers));

            assert!(
                page.contains(&format!(
                    "<div id=\"{id}\" class=\"class with escaped &lt;, &gt;, &amp;, &quot; and &#39;\">",
                    id = site_config().styles().wrapper_id()
                )),
                "got {page}"
            );
        }

        #[test]
        fn escapes_stylesheet() {
            let headers = PageHeadersBuilder {
                title: Some("Test"),
                css_stylesheet: Some("/styles/escaped <, >, &, \" and '.css"),
                is_post: Some(false),
                ..Default::default()
            }
            .build()
            .unwrap();

            let page = render(&default_page(headers));

            assert!(
                page.contains(
                    "<link href=\"/styles/escaped &lt;, &gt;, &amp;, &quot; and &#39;.css\" rel=\"stylesheet\" type=\"text/css\"/>"
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
                created_at: Some("2026-06-26"),
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
                created_at: Some("2026-06-26"),
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
                css_stylesheet: Some("/styles/post.css"),
                is_post: Some(true),
                created_at: Some("2026-06-26"),
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
                !page.contains(&format!(
                    "<link href=\"{styles}\" rel=\"stylesheet\" type=\"text/css\"/>",
                    styles = site_config().styles().stylesheet()
                )),
                "got {page}"
            );
            // custom stylesheet does not depend on the default one
            assert!(
                page.contains("<link href=\"/styles/post.css\" rel=\"stylesheet\" type=\"text/css\"/>"),
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
                page.contains(&format!(
                    "<link href=\"{styles}\" rel=\"stylesheet\" type=\"text/css\"/>",
                    styles = site_config().styles().stylesheet()
                )),
                "got {page}"
            );
        }

        #[test]
        fn default_stylesheet_preceeds_custom_one() {
            let headers = PageHeadersBuilder {
                title: Some("Page"),
                is_post: Some(false),
                include_styles: Some(true),
                css_stylesheet: Some("/styles/override.css"),
                ..Default::default()
            }
            .build()
            .unwrap();

            let page = render(&default_page(headers));

            eprintln!("{page}");

            assert!(
                page.contains(
                    "<link href=\"/style.css\" rel=\"stylesheet\" type=\"text/css\"/>\n\
                     <link href=\"/styles/override.css\" rel=\"stylesheet\" type=\"text/css\"/>\n"
                ),
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
                page.contains(&format!(
                    "<div id=\"{id}\">\n<main>\n",
                    id = site_config().styles().wrapper_id()
                )),
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
                    "<div id=\"{id}\">\n<nav>site nav</nav>\n<main>\n",
                    id = site_config().styles().wrapper_id()
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
                [(BlockKind::Head, "<meta name=\"author\" content=\"Example\">\n")],
            );

            // the mandatory tags survive, charset still leading the head ...
            assert!(page.contains("<head>\n<meta charset=\"utf-8\"/>\n"), "got {page}");
            assert!(
                page.contains("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"/>"),
                "got {page}"
            );
            // ... as do the rest of the defaults ...
            assert!(page.contains("<title>Page</title>"), "got {page}");
            assert!(
                page.contains("<link href=\"/feed.atom\" type=\"application/atom+xml\" rel=\"alternate\"/>"),
                "got {page}"
            );
            // ... and the block is added to them, verbatim
            assert!(
                page.contains("<meta name=\"author\" content=\"Example\">"),
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
                    "<link href=\"/styles/override.css\" rel=\"stylesheet\" type=\"text/css\"/>\n\
                     <meta name=\"viewport\" content=\"width=device-width\">\n",
                )],
            );

            assert!(
                page.contains(
                    "<link href=\"/feed.atom\" type=\"application/atom+xml\" rel=\"alternate\"/>\n\
                     <link href=\"/styles/override.css\" rel=\"stylesheet\" type=\"text/css\"/>\n\
                     <meta name=\"viewport\" content=\"width=device-width\">\n\
                     </head>"
                ),
                "got {page}"
            );
            // the mandatory charset is not displaced by a block, so it keeps the lead
            assert!(page.contains("<head>\n<meta charset=\"utf-8\"/>\n"), "got {page}");
        }
    }
}
