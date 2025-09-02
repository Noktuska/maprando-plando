use std::{collections::HashMap, fs::File, io::Write, path::Path};

use anyhow::{bail, Result};
use self_update::update::{Release, ReleaseAsset};

fn main() {
    match perform_update() {
        Ok(_) => println!("Updated successfully"),
        Err(err) => println!("ERROR: {}", err.to_string())
    }
}

fn perform_update() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let mut args_map = HashMap::new();
    for arg in args {
        if let Some((k, v)) = arg.split_once('=') {
            args_map.insert(k.to_string(), v.to_string());
        }
    }

    let opt_name = args_map.get("name");
    let opt_url = args_map.get("url");

    let asset = if opt_name.is_some() && opt_url.is_some() {
        ReleaseAsset {
            name: opt_name.unwrap().clone(),
            download_url: opt_url.unwrap().clone()
        }
    } else {
        let release = check_update()?;
        if release.assets.is_empty() {
            bail!("No assets found for release {}", release.version);
        }
        println!("Found version {}, would you like to update? (Y/N)", release.version);
        std::io::stdout().flush()?;
        let mut s = String::new();

        std::io::stdin().read_line(&mut s)?;
        if s.trim().to_ascii_lowercase() != "y" {
            bail!("Declined updated");
        }

        release.assets[0].clone()
    };

    let tmp_dir_path = Path::new("./tmp/");
    let tmp_archive_path = tmp_dir_path.join(&asset.name);
    let file_ext = tmp_archive_path.extension().unwrap().to_str().unwrap().to_string();

    assert_eq!(file_ext, "zip", "Expected zip file");

    std::fs::create_dir_all(tmp_dir_path)?;
    let tmp_file = std::fs::File::create(&tmp_archive_path)?;

    println!("Downloading {}", asset.download_url);
    self_update::Download::from_url(&asset.download_url)
        .set_header(reqwest::header::ACCEPT, "application/octet-stream".parse()?)
        .download_to(tmp_file)?;

    let updater_path = std::env::current_exe()?;
    let updater_file_name = updater_path.file_name().unwrap();
    let dir_path = updater_path.parent().unwrap();
    let new_updater_path = dir_path.join(Path::new("maprando-plando-update__new__.exe"));

    let file = File::open(&tmp_archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    println!("Unpacking archive...");
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let path = match file.enclosed_name() {
            Some(path) => path,
            None => continue
        };

        let out_path = dir_path.join(&path);
        if file.is_dir() {
            // Remove all old files
            if std::fs::exists(&out_path)? {
                std::fs::remove_dir_all(&out_path)?;
            }
            std::fs::create_dir_all(out_path)?;
        } else if path.file_name() == Some(updater_file_name) {
            // Copy new version of updater into a temporary location
            let mut out_file = File::create(&new_updater_path)?;
            std::io::copy(&mut file, &mut out_file)?;
        } else {
            // Copy other new files
            if let Some(parent) = out_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            let mut out_file = File::create(out_path)?;
            std::io::copy(&mut file, &mut out_file)?;
        }
    }

    // If a new updater was shipped, replace the old one
    if std::fs::exists(&new_updater_path)? {
        self_update::self_replace::self_replace(&new_updater_path)?;
        std::fs::remove_file(new_updater_path)?;
    }

    println!("Deleting tmp directory...");
    std::fs::remove_dir_all(tmp_dir_path)?;

    Ok(())
}

fn check_update() -> Result<Release> {
    let release_list = self_update::backends::github::ReleaseList::configure()
        .repo_owner("noktuska")
        .repo_name("maprando-plando")
        .build()?.fetch()?;
    
    if release_list.is_empty() {
        bail!("No releases found");
    }
    let release = &release_list[0];
    if release.assets.is_empty() {
        bail!("No assets found with latest release");
    }
    Ok(release.clone())
}