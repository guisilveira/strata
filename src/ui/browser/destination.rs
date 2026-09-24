// SPDX-License-Identifier: MIT

use crate::services::{SearchEvent, index_tree};
use crate::ui::browser::paths::compact_native_path;
use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

pub(super) struct TransferSearchScope {
    pub(super) base: std::path::PathBuf,
    pub(super) search_root: std::path::PathBuf,
    pub(super) root_limit: Option<std::path::PathBuf>,
    pub(super) show_hidden: bool,
}

fn render_transfer_suggestions(
    suggestions: &gtk::Box,
    items: Vec<crate::services::SearchItem>,
    field: &gtk::Entry,
    root_limit: Option<&Path>,
) {
    while let Some(child) = suggestions.first_child() {
        suggestions.remove(&child);
    }
    let mut dirs: Vec<_> = items
        .into_iter()
        .filter_map(|mut item| {
            if !item.is_directory {
                return None;
            }
            if let Some(root) = root_limit {
                item.path = canonical_directory_within(root, &item.path)?;
            }
            Some(item)
        })
        .collect();
    dirs.sort_by_key(|item| item.path.ancestors().count());
    dirs.truncate(8);
    if dirs.is_empty() {
        let empty = gtk::Label::new(Some("No matching folders"));
        empty.add_css_class("transfer-suggestions-empty");
        empty.set_xalign(0.0);
        suggestions.append(&empty);
        return;
    }
    for item in dirs {
        append_suggestion(suggestions, &item.path, field);
    }
}

fn append_suggestion(suggestions: &gtk::Box, path: &Path, field: &gtk::Entry) {
    let option = gtk::Button::new();
    option.add_css_class("transfer-suggestion");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 9);
    row.append(&crate::assets::primary_icon(
        crate::assets::icons::FOLDER,
        16,
    ));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let label = gtk::Label::new(Some(&name));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    row.append(&label);
    if let Some(parent) = path.parent().map(compact_native_path) {
        let parent_label = gtk::Label::new(Some(&parent));
        parent_label.add_css_class("transfer-suggestion-parent");
        parent_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        parent_label.set_xalign(1.0);
        row.append(&parent_label);
    }
    option.set_child(Some(&row));
    option.set_tooltip_text(Some(&path.to_string_lossy()));
    let selected_field = field.clone();
    let path = path.to_path_buf();
    option.connect_clicked(move |_| {
        selected_field.remove_css_class("error");
        selected_field.set_text(&folder_input_path(&path));
        selected_field.set_position(-1);
        selected_field.grab_focus();
    });
    suggestions.append(&option);
}

pub(super) fn setup_transfer_search(
    field: &gtk::Entry,
    suggestions: &gtk::Box,
    generation: &Rc<Cell<u64>>,
    scope: TransferSearchScope,
    on_changed: impl Fn(&gtk::Entry) + 'static,
) {
    let TransferSearchScope {
        base,
        search_root,
        root_limit,
        show_hidden,
    } = scope;
    let (search_handle, search_receiver) = index_tree(search_root, show_hidden);
    let search_handle = Rc::new(search_handle);
    let query_handle = Rc::downgrade(&search_handle);
    let search_mode = Rc::new(Cell::new(false));
    let poll_suggestions = suggestions.downgrade();
    let poll_field = field.downgrade();
    let poll_mode = search_mode.clone();
    let poll_root_limit = root_limit.clone();
    let _poll = glib::timeout_add_local(Duration::from_millis(16), move || {
        let _keep_search_alive = &search_handle;
        let (Some(suggestions), Some(field)) = (poll_suggestions.upgrade(), poll_field.upgrade())
        else {
            return glib::ControlFlow::Break;
        };
        if field.root().is_none() {
            return glib::ControlFlow::Break;
        }
        if !poll_mode.get() {
            return glib::ControlFlow::Continue;
        }
        let mut latest = None;
        for _ in 0..8 {
            match search_receiver.try_recv() {
                Ok(event) => latest = Some(event),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return glib::ControlFlow::Break;
                }
            }
        }
        if let Some(SearchEvent::Results { query, items, .. }) = latest
            && query == field.text().trim()
        {
            render_transfer_suggestions(&suggestions, items, &field, poll_root_limit.as_deref());
        }
        glib::ControlFlow::Continue
    });
    let suggestions_clone = suggestions.clone();
    let generation_clone = generation.clone();
    field.connect_changed(move |field| {
        on_changed(field);
        let input = field.text().to_string();
        let request = generation_clone.get().saturating_add(1);
        generation_clone.set(request);
        let looks_like_path = input.trim().contains(std::path::MAIN_SEPARATOR)
            || input.trim().starts_with('~')
            || input.trim().is_empty();
        if looks_like_path {
            search_mode.set(false);
            let gen_check = generation_clone.clone();
            let home = glib::home_dir();
            let base = base.clone();
            let root_limit = root_limit.clone();
            let field_clone = field.clone();
            let suggestions_clone = suggestions_clone.clone();
            glib::MainContext::default().spawn_local(async move {
                let matches = gio::spawn_blocking(move || {
                    path_suggestions(&input, &base, &home, root_limit.as_deref())
                })
                .await;
                if gen_check.get() != request {
                    return;
                }
                while let Some(child) = suggestions_clone.first_child() {
                    suggestions_clone.remove(&child);
                }
                let Ok(paths) = matches else { return };
                if paths.is_empty() {
                    let empty = gtk::Label::new(Some("No matching folders"));
                    empty.add_css_class("transfer-suggestions-empty");
                    empty.set_xalign(0.0);
                    suggestions_clone.append(&empty);
                }
                for path in paths {
                    append_suggestion(&suggestions_clone, &path, &field_clone);
                }
            });
        } else {
            search_mode.set(true);
            if let Some(search_handle) = query_handle.upgrade() {
                search_handle.query(&input);
            }
        }
    });
}

pub(super) fn folder_input_path(path: &Path) -> String {
    let path = compact_native_path(path);
    if path.ends_with(std::path::MAIN_SEPARATOR) {
        path
    } else {
        format!("{path}{}", std::path::MAIN_SEPARATOR)
    }
}

pub(super) fn resolve_destination_path(
    input: &str,
    base: &Path,
    home: &Path,
) -> std::path::PathBuf {
    let input = input.trim();
    if input == "~" {
        home.to_path_buf()
    } else if let Some(relative) = input.strip_prefix("~/") {
        home.join(relative)
    } else {
        let path = Path::new(input);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            base.join(path)
        }
    }
}

fn path_suggestions(
    input: &str,
    base: &Path,
    home: &Path,
    root_limit: Option<&Path>,
) -> Vec<std::path::PathBuf> {
    let resolved = resolve_destination_path(input, base, home);
    let trailing_separator = input.trim_end().ends_with(std::path::MAIN_SEPARATOR);
    let (directory, prefix) = if trailing_separator {
        (resolved, String::new())
    } else {
        (
            resolved.parent().unwrap_or(base).to_path_buf(),
            resolved
                .file_name()
                .map(|name| name.to_string_lossy().to_lowercase())
                .unwrap_or_default(),
        )
    };
    let directory = match root_limit {
        Some(root) => {
            let Some(directory) = canonical_directory_within(root, &directory) else {
                return Vec::new();
            };
            directory
        }
        None => directory,
    };
    let Ok(children) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut matches = children
        .filter_map(Result::ok)
        .map(|child| child.path())
        .filter_map(|path| {
            if !path.is_dir() {
                return None;
            }
            path.file_name()
                .is_some_and(|name| {
                    let name = name.to_string_lossy().to_lowercase();
                    (prefix.starts_with('.') || !name.starts_with('.')) && name.starts_with(&prefix)
                })
                .then_some(())?;
            match root_limit {
                Some(root) => canonical_directory_within(root, &path),
                None => Some(path),
            }
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });
    matches.truncate(8);
    matches
}

pub(super) fn canonical_existing_directory(path: &Path) -> Option<std::path::PathBuf> {
    let canonical = std::fs::canonicalize(path).ok()?;
    canonical.is_dir().then_some(canonical)
}

pub(super) fn canonical_directory_within(
    root: &Path,
    candidate: &Path,
) -> Option<std::path::PathBuf> {
    let root = canonical_existing_directory(root)?;
    let candidate = canonical_existing_directory(candidate)?;
    candidate.strip_prefix(root).ok()?;
    Some(candidate)
}

pub(super) fn rebind_directory_within_root(
    opened_root: &Path,
    selected: &Path,
    current_root: &Path,
) -> Option<std::path::PathBuf> {
    let relative = selected.strip_prefix(opened_root).ok()?;
    let current_root = canonical_existing_directory(current_root)?;
    canonical_directory_within(&current_root, &current_root.join(relative))
}
