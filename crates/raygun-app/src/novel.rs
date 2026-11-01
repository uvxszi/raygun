//! Novels (M31): `novel.source` catalogues through the shared `Catalog`, and chapters as text
//! blocks for the novel reader. Progress is a character offset into the chapter's text.

use std::collections::HashMap;

use raygun_ext_host::novel::{NOVEL_SOURCE, PassageBlock};
use raygun_model::{
    MangaBrowseMode, MangaLibraryEntry, MangaListingPage, MangaSource, MangaTitleView,
    NovelChapter, TextBlock,
};
use raygun_net::ImageLinks;
use tauri::AppHandle;

use crate::catalog::{Catalog, chapter_info};

const KIND: &str = "novel";

pub struct NovelService {
    catalog: Catalog,
}

impl NovelService {
    pub fn new(app: AppHandle, images: ImageLinks) -> Self {
        Self {
            catalog: Catalog::new(app, images, KIND, NOVEL_SOURCE),
        }
    }

    pub async fn sources(&self) -> Vec<MangaSource> {
        self.catalog.sources().await
    }

    pub async fn browse(
        &self,
        source_id: &str,
        mode: MangaBrowseMode,
        query: Option<&str>,
        page: u32,
    ) -> Result<MangaListingPage, String> {
        self.catalog.browse(source_id, mode, query, page).await
    }

    pub async fn title(
        &self,
        source_id: &str,
        remote_id: &str,
        refresh: bool,
    ) -> Result<MangaTitleView, String> {
        let row = self
            .catalog
            .title_row(source_id, remote_id, refresh)
            .await?;
        self.catalog.title_view(row, &HashMap::new()).await
    }

    pub async fn set_in_library(&self, title_id: &str, in_library: bool) -> Result<(), String> {
        self.catalog.set_in_library(title_id, in_library).await
    }

    pub async fn library(&self) -> Result<Vec<MangaLibraryEntry>, String> {
        self.catalog.library().await
    }

    /// One chapter's text, with its neighbours in reading order.
    pub async fn chapter(&self, title_id: &str, unit_id: &str) -> Result<NovelChapter, String> {
        let context = self.catalog.chapter_context(title_id, unit_id).await?;
        let passage = {
            let extensions = self.catalog.extensions();
            let (_ready, host) = extensions.ready().await;
            host.novel_passage(
                &context.title.fields.source_id,
                &context.unit.fields.remote_id,
            )
            .await?
        };
        Ok(NovelChapter {
            chapter: chapter_info(&context.unit, &HashMap::new()),
            title_id: context.title.id,
            title: context.title.fields.title,
            blocks: passage.into_iter().map(|block| self.block(block)).collect(),
            previous_chapter_id: context.previous_id,
            next_chapter_id: context.next_id,
        })
    }

    fn block(&self, block: PassageBlock) -> TextBlock {
        match block {
            PassageBlock::Paragraph(text) => TextBlock::Paragraph { text },
            PassageBlock::Heading(text) => TextBlock::Heading { text },
            PassageBlock::Separator => TextBlock::Separator,
            PassageBlock::Image(url) => TextBlock::Image {
                url: self.catalog.images.link(&url, &Default::default()),
            },
        }
    }

    /// Stores the character offset reached; reaching the end (`offset >= total - 1`) completes
    /// the chapter.
    pub async fn set_progress(&self, unit_id: &str, offset: u32, total: u32) -> Result<(), String> {
        self.catalog.set_progress(unit_id, offset, total).await
    }

    pub async fn set_read(&self, unit_id: &str, read: bool) -> Result<(), String> {
        self.catalog.set_read(unit_id, read).await
    }
}
