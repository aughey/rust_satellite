use std::path::{Path, PathBuf};

use anyhow::Result;

fn process_dir(
    dirpath: &PathBuf,
) -> Result<(impl Iterator<Item = std::path::PathBuf>, impl Iterator<Item = std::path::PathBuf>)> {
    let dir = std::fs::read_dir(dirpath)?.filter_map(|entry| entry.ok()).collect::<Vec<_>>();
    let files_with_links = dir.into_iter().filter_map(|entry| {
        let path = entry.path();
        let metadata = entry.metadata().ok()?;
        let filetype = metadata.file_type();
        if filetype.is_symlink() {
            Some(path)
        } else {
            None
        }
    });
    let files_with_slashlib = files_with_links.filter_map(|path| {
        let target = std::fs::read_link(&path).ok()?;
        let target = target.to_str()?;
        if target.starts_with("/lib") {
            Some(path)
        } else {
            None
        }
    });

    let dir = std::fs::read_dir(dirpath)?.filter_map(|entry| entry.ok()).collect::<Vec<_>>();
    let directories = dir.into_iter().filter_map(|entry| {
        let path = entry.path();
        let metadata = entry.metadata().ok()?;
        let filetype = metadata.file_type();
        if filetype.is_dir() && !filetype.is_symlink() {
            Some(path.clone())
        } else {
            None
        }
    });

    Ok((directories, files_with_slashlib))
}

fn main() -> Result<()> {
    // first argument is the path to the executable
    let dirpath = std::env::args()
        .nth(1)
        .ok_or(anyhow::anyhow!("use path on command line"))?;

    // add dirpath to dirs
    let mut dirs = vec![Path::new(&dirpath).to_path_buf()];

    while let Some(dir) = dirs.pop() {
        let (directories, files_with_slashlib) = process_dir(&dir)?;
        
        dirs.append(&mut directories.collect::<Vec<_>>());

        for file in files_with_slashlib {
            println!("{}", file.display());
            // The link started with /lib replace the link to start with /pilib
            let target = std::fs::read_link(&file)?;
            let target = target.to_str().ok_or_else(||anyhow::anyhow!("Couldn't read the link of a file that should have"))?;
            let target = target.replace("/lib", "/pilib");
            std::fs::remove_file(&file)?;
            std::os::unix::fs::symlink(&target, &file)?;
        }
    }


    Ok(())
}
