use std::path::Path;

use anyhow::Result;
use egui::Context;
use serde::Deserialize;
use tokio::task::JoinHandle;

pub struct Upload {
    pub is_open: bool,

    pub upload_handle: Option<JoinHandle<Result<UploadResponse>>>,

    form_data: FormData
}

#[derive(Deserialize)]
pub struct UploadResponse {
    pub seed_id: String
}

#[derive(Default, Clone)]
struct FormData {
    title: String,
    desc: String,
    allow_spoiler: bool,
    allow_download: bool
}

impl Upload {
    pub const TMP_FILE_PATH: &'static str = "../tmp_upload.json";
    pub const PLANDO_WEB_URL: &'static str = "https://maprando-plando.com";

    pub fn new() -> Self {
        Upload {
            is_open: false,
            upload_handle: None,
            form_data: FormData::default()
        }
    }

    pub fn draw(&mut self, ctx: &Context) {
        egui::Window::new("Upload Seed").resizable(false).collapsible(false).show(ctx, |ui| {
            ui.label("Upload your seed so other people can play it without having to download the Plando Software. These settings are optional and cannot be changed later.");
            egui::Grid::new("grid_upload").num_columns(2).show(ui, |ui| {
                ui.label("Title:");
                ui.text_edit_singleline(&mut self.form_data.title);
                ui.end_row();

                ui.label("Description:");
                ui.text_edit_multiline(&mut self.form_data.desc);
                ui.end_row();

                ui.checkbox(&mut self.form_data.allow_spoiler, "Unlock Spoiler Log");
                ui.checkbox(&mut self.form_data.allow_download, "Allow Plando JSON download");
                ui.end_row();

                if ui.button("Upload").clicked() && self.upload_handle.is_none() {
                    self.upload_handle = Some(tokio::spawn(upload_seed(self.form_data.clone())));
                    self.is_open = false;
                }
                if ui.button("Cancel").clicked() {
                    self.is_open = false;
                }
            });
        });
    }

}

async fn upload_seed(form_data: FormData) -> Result<UploadResponse> {
    let path = Path::new(Upload::TMP_FILE_PATH);
    let form = reqwest::multipart::Form::new()
        .text("name", form_data.title.clone())
        .text("desc", form_data.desc.clone())
        .text("allow_spoiler", form_data.allow_spoiler.to_string())
        .text("allow_download", form_data.allow_download.to_string())
        .file("file", path).await?;
    let client = reqwest::Client::new();
    let resp = client.post(format!("{}/upload-seed", Upload::PLANDO_WEB_URL))
        .multipart(form)
        .send()
        .await?
        .json()
        .await?;
    Ok(resp)
}