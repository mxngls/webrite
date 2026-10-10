use std::path::Path;
use std::{fs, process};

mod page;

static CONFIG_PATH: &str = "webrite.txt";

use crate::page::{Blocks, Error, Page, PathContext, SiteConfig, parse_config, parse_headers, split_page, write_page};

fn build_page(page_path: &Path, blocks: &Blocks, config: &SiteConfig) -> Result<Page, Error> {
    let page_str = fs::read_to_string(page_path).at_path(page_path)?;

    let (headers, body) = split_page(&page_str).at_path(page_path)?;
    let headers = parse_headers(headers).at_path(page_path)?;

    let rel_path = page_path
        .strip_prefix(config.build().r#in())
        .expect("file paths descend from the input directory");

    let page = Page::new(rel_path.to_path_buf(), headers, body.to_string());

    write_page(&page, blocks, config)?;

    Ok(page)
}

fn build_site(config_path: &Path) -> Result<(), Error> {
    let config_str = match fs::read_to_string(config_path) {
        Ok(config_str) => config_str,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error).at_path(config_path),
    };
    let config = parse_config(&config_str).at_path(config_path)?;

    let in_path = Path::new(config.build().r#in());
    let blocks = Blocks::from_dir(&in_path.to_path_buf().join(config.build().blocks()))?;

    let mut dir_stack = vec![in_path.to_path_buf()];

    while let Some(dir) = dir_stack.pop() {
        for entry in fs::read_dir(&dir).at_path(&dir)? {
            let entry = entry.at_path(&dir)?;
            let path = entry.path();
            let typ = entry.file_type().at_path(&path)?;

            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }

            let rel_path = path
                .strip_prefix(config.build().r#in())
                .expect("paths descend from the input directory");
            let out_dir = Path::new(config.build().out()).join(rel_path);

            if typ.is_dir() {
                let dir_name = entry.file_name();
                if dir_name == config.build().drafts() || dir_name == config.build().blocks() {
                    continue;
                }
                fs::create_dir_all(&out_dir).at_path(&out_dir)?;
                dir_stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "htm") {
                build_page(&path, &blocks, &config)?;
            } else {
                fs::copy(&path, &out_dir).at_path(&out_dir)?;
            }
        }
    }

    Ok(())
}

fn main() -> process::ExitCode {
    match build_site(Path::new(CONFIG_PATH)) {
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
