// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Request handling for `easysearch`.

use easysearch_core::index::EsIndex;
use easysearch_core::usn::EsUsnEvent;

use crate::config::EsConfig;
use crate::drive_manager::{self, DriveManager};
use crate::process_lifetime::ShutdownReason;
use crate::protocol::{Request, Response, ResponseItem};

/// EasySearch backend service state.
#[derive(Debug)]
pub(crate) struct EsService {
    config: EsConfig,
    drives: DriveManager,
    /// `true` while the initial index build is still running.
    indexing: bool,
    shutdown_reason: Option<ShutdownReason>,
}

impl EsService {
    /// Create a service in the *indexing* state.
    #[must_use]
    pub(crate) fn new(config: EsConfig) -> Self {
        Self {
            config,
            drives: DriveManager::new(),
            indexing: true,
            shutdown_reason: None,
        }
    }

    /// Install a freshly built index for a drive.
    pub(crate) fn install_index(&mut self, drive_letter: char, index: EsIndex) {
        self.drives.install(drive_letter, index);
    }

    /// Flip the indexing flag once the initial build completes.
    pub(crate) fn set_indexing(&mut self, indexing: bool) {
        self.indexing = indexing;
    }

    /// Return the USN cursor `(journal_id, last_usn)` for a drive.
    pub(crate) fn cursor(&self, drive_letter: char) -> Option<(u64, i64)> {
        self.drives.cursor(drive_letter)
    }

    /// Apply incremental USN events to a drive.
    pub(crate) fn apply_events(
        &mut self,
        drive_letter: char,
        events: &[EsUsnEvent],
        new_last_usn: i64,
        journal_id: u64,
    ) {
        self.drives
            .apply(drive_letter, events, new_last_usn, journal_id);
    }

    /// Handle one raw NDJSON line.
    #[must_use]
    pub(crate) fn handle_json_line(&mut self, line: &str) -> Response {
        match serde_json::from_str::<Request>(line) {
            Ok(request) => self.handle_request(request),
            Err(error) => Response::error(0, format!("invalid request JSON: {error}")),
        }
    }

    /// Return whether the service should stop.
    #[must_use]
    pub(crate) const fn should_shutdown(&self) -> bool {
        self.shutdown_reason.is_some()
    }

    fn handle_request(&mut self, request: Request) -> Response {
        match request.method.as_str() {
            "status" => self.status(request.id),
            "search" => self.search(request),
            "enumerate" => self.enumerate(request),
            "rebuild" => self.rebuild(request),
            "shutdown" => self.shutdown(request.id),
            other => Response::error(request.id, format!("unknown method: {other}")),
        }
    }

    fn status(&self, id: u64) -> Response {
        let drives = self.drives.drive_labels();
        let ready = !self.indexing && !drives.is_empty();
        let last_usn = self
            .drives
            .indexes()
            .map(|index| index.status.last_usn)
            .max()
            .unwrap_or(0);
        Response {
            id,
            ok: true,
            ready: Some(ready),
            drives: Some(drives),
            records: Some(self.drives.record_count()),
            indexing: Some(self.indexing),
            last_usn: Some(last_usn),
            items: None,
            started: None,
            error: None,
        }
    }

    fn search(&self, request: Request) -> Response {
        let query = request.query.unwrap_or_default();
        let limit = request.limit.unwrap_or(100);
        let items: Vec<ResponseItem> = self
            .drives
            .indexes()
            .flat_map(|index| index.search(&query, limit))
            .take(limit)
            .map(ResponseItem::from)
            .collect();
        Response {
            id: request.id,
            ok: true,
            ready: None,
            drives: None,
            records: None,
            indexing: None,
            last_usn: None,
            items: Some(items),
            started: None,
            error: None,
        }
    }

    fn enumerate(&self, request: Request) -> Response {
        let Some(path) = request.path else {
            return Response::error(request.id, "enumerate requires path");
        };
        let query = request.query.unwrap_or_default();
        let recursive = request.recursive.unwrap_or(false);
        let limit = request.limit.unwrap_or(100);

        let mut items: Vec<ResponseItem> = Vec::new();
        let mut last_error: Option<String> = None;
        for index in self.drives.indexes() {
            match index.enumerate(&path, &query, recursive, limit) {
                Ok(results) => items.extend(results.into_iter().map(ResponseItem::from)),
                Err(error) => last_error = Some(error.to_string()),
            }
            if items.len() >= limit {
                items.truncate(limit);
                break;
            }
        }

        if items.is_empty() {
            if let Some(error) = last_error {
                return Response::error(request.id, error);
            }
        }

        Response {
            id: request.id,
            ok: true,
            ready: None,
            drives: None,
            records: None,
            indexing: None,
            last_usn: None,
            items: Some(items),
            started: None,
            error: None,
        }
    }

    fn rebuild(&mut self, request: Request) -> Response {
        let drive = request.drive.unwrap_or_else(|| {
            self.config
                .auto_index_drives
                .first()
                .copied()
                .unwrap_or('C')
                .to_string()
        });

        let letter = drive
            .chars()
            .next()
            .map(|c| c.to_ascii_uppercase())
            .unwrap_or('C');

        match drive_manager::build_index(letter, self.config.cache_dir.as_deref()) {
            Ok(index) => {
                self.drives.install(letter, index);
                Response {
                    id: request.id,
                    ok: true,
                    ready: None,
                    drives: None,
                    records: None,
                    indexing: None,
                    last_usn: None,
                    items: None,
                    started: Some(true),
                    error: None,
                }
            }
            Err(err) => Response::error(request.id, format!("rebuild failed: {err}")),
        }
    }

    fn shutdown(&mut self, id: u64) -> Response {
        self.shutdown_reason = Some(ShutdownReason::Requested);
        Response {
            id,
            ok: true,
            ready: None,
            drives: None,
            records: None,
            indexing: None,
            last_usn: None,
            items: None,
            started: None,
            error: None,
        }
    }
}
