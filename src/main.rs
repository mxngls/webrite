use std::path::Path;
use std::{fs, process};

mod page;

use crate::page::{
    BLOCK_DIR, Blocks, DRAFT_DIR, Error, IN_DIR, OUT_DIR, Page, PathContext, parse_header, split_page, write_page,
};

fn process_file(input_path: &Path, blocks: &Blocks) -> Result<Page, Error> {
    let page_str = fs::read_to_string(input_path).at_path(input_path)?;

    let (headers, body) = split_page(&page_str).at_path(input_path)?;
    let headers = parse_header(headers).at_path(input_path)?;

    let rel_path = input_path
        .strip_prefix(IN_DIR)
        .expect("file paths descend from the input directory");

    let page = Page::new(rel_path.to_path_buf(), headers, body.to_string());

    write_page(&page, blocks)?;

    Ok(page)
}

fn process_dir(input_dir: &Path) -> Result<(), Error> {
    let blocks = Blocks::from_dir(&input_dir.join(BLOCK_DIR))?;

    let mut dir_stack = vec![input_dir.to_path_buf()];

    while let Some(dir) = dir_stack.pop() {
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
            let out_dir = Path::new(OUT_DIR).join(rel_path);

            if typ.is_dir() {
                let dir_name = entry.file_name();
                if dir_name == DRAFT_DIR || dir_name == BLOCK_DIR {
                    continue;
                }
                fs::create_dir_all(&out_dir).at_path(&out_dir)?;
                dir_stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "htm") {
                process_file(&path, &blocks)?;
            } else {
                fs::copy(&path, &out_dir).at_path(&out_dir)?;
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
