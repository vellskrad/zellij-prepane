use std::collections::BTreeMap;

use kdl::KdlDocument;
use zellij_tile::prelude::*;

#[derive(Clone, Debug)]
struct TemplateEntry {
    label: String,
    layout_name: String,
}

#[derive(Default)]
struct State {
    templates_kdl: String,
    entries: Vec<TemplateEntry>,
    selected: usize,
    scroll: usize,
    error: Option<String>,
    permissions_resolved: bool,
}

register_plugin!(State);

impl State {
    fn sanitize_templates_kdl(raw: &str) -> String {
        let mut s = raw.to_string();
        if s.starts_with('\u{feff}') {
            s = s.trim_start_matches('\u{feff}').to_string();
        }
        let s = s.trim().to_string();
        if s.is_empty() {
            return s;
        }
        if s.ends_with('\n') {
            s
        } else {
            format!("{s}\n")
        }
    }

    fn debug_preview(s: &str, max_len: usize) -> String {
        let mut out = String::new();
        for ch in s.chars().take(max_len) {
            match ch {
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                _ => out.push(ch),
            }
        }
        if s.chars().count() > max_len {
            out.push_str("…");
        }
        out
    }

    fn wrap_text_to_width(text: &str, width: usize) -> Vec<String> {
        let width = width.max(1);
        let mut lines: Vec<String> = Vec::new();

        for paragraph in text.split('\n') {
            let paragraph = paragraph.trim_end_matches('\r');
            if paragraph.is_empty() {
                lines.push(String::new());
                continue;
            }

            let mut current = String::new();
            for word in paragraph.split_whitespace() {
                if current.is_empty() {
                    current.push_str(word);
                    continue;
                }

                if current.len() + 1 + word.len() <= width {
                    current.push(' ');
                    current.push_str(word);
                } else {
                    lines.push(current);
                    current = word.to_string();
                }
            }

            if !current.is_empty() {
                lines.push(current);
            }
        }

        lines
    }

    fn load_entries(&mut self) {
        self.entries.clear();
        self.error = None;
        self.selected = 0;
        self.scroll = 0;

        let sanitized = Self::sanitize_templates_kdl(&self.templates_kdl);
        let doc: KdlDocument = match sanitized.parse() {
            Ok(d) => d,
            Err(e) => {
                let preview = Self::debug_preview(&sanitized, 160);
                self.error = Some(format!(
                    "Failed to parse templates KDL ({}) len={} preview=\"{}\"",
                    e,
                    sanitized.len(),
                    preview
                ));
                return;
            }
        };

        for node in doc.nodes() {
            if node.name().value() != "template" {
                continue;
            }
            let label = node
                .get("label")
                .and_then(|e| e.value().as_string())
                .unwrap_or("")
                .trim();
            let layout_name = node
                .get("layout_name")
                .and_then(|e| e.value().as_string())
                .unwrap_or("")
                .trim();

            if label.is_empty() || layout_name.is_empty() {
                continue;
            }
            self.entries.push(TemplateEntry {
                label: label.to_string(),
                layout_name: layout_name.to_string(),
            });
        }

        if self.entries.is_empty() && self.error.is_none() {
            self.error = Some("Manifest is empty.".to_string());
        }
    }

    fn selected_entry(&self) -> Option<&TemplateEntry> {
        self.entries.get(self.selected)
    }

    fn move_selection(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as isize;
        let cur = self.selected as isize;
        let next = (cur + delta).clamp(0, len - 1);
        self.selected = next as usize;
    }

    fn open_selected(&mut self) {
        let Some(entry) = self.selected_entry().cloned() else {
            return;
        };

        let layout_kdl = match dump_layout(&entry.layout_name) {
            Ok(kdl) => kdl,
            Err(e) => {
                self.error = Some(format!(
                    "dump_layout(\"{}\") failed: {} (hint: grant ReadApplicationState; restart Zellij after layout_dir changes; ensure layout exists under layout_dir)",
                    entry.layout_name, e
                ));
                return;
            }
        };

        let layout_kdl = Self::sanitize_templates_kdl(&layout_kdl);
        if let Err(e) = layout_kdl.parse::<KdlDocument>() {
            let preview = Self::debug_preview(&layout_kdl, 200);
            self.error = Some(format!(
                "dump_layout(\"{}\") returned unparsable KDL: {} len={} preview=\"{}\"",
                entry.layout_name,
                e,
                layout_kdl.len(),
                preview
            ));
            return;
        }

        let created = new_tabs_with_layout(&layout_kdl);
        if created.is_empty() {
            let preview = Self::debug_preview(&layout_kdl, 200);
            self.error = Some(format!(
                "new_tabs_with_layout returned no tabs for \"{}\" (len={} preview=\"{}\")",
                entry.layout_name,
                layout_kdl.len(),
                preview
            ));
            return;
        }

        close_self();
    }

    fn ensure_visible(&mut self, visible_rows: usize) {
        if self.entries.is_empty() || visible_rows == 0 {
            self.scroll = 0;
            return;
        }
        let sel = self.selected;
        if sel < self.scroll {
            self.scroll = sel;
        } else if sel >= self.scroll + visible_rows {
            self.scroll = sel.saturating_sub(visible_rows.saturating_sub(1));
        }
        let max_scroll = self.entries.len().saturating_sub(visible_rows);
        self.scroll = self.scroll.min(max_scroll);
    }
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.templates_kdl = configuration
            .get("templates_kdl")
            .cloned()
            .unwrap_or_default();

        subscribe(&[
            EventType::Key,
            EventType::Mouse,
            EventType::PermissionRequestResult,
        ]);

        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(status) => {
                match status {
                    PermissionStatus::Granted => {
                        self.permissions_resolved = true;
                        if self.templates_kdl.trim().is_empty() {
                            self.error = Some(
                                "No templates configured. Set plugin config key \"templates_kdl\"."
                                    .to_string(),
                            );
                        } else {
                            self.load_entries();
                        }
                    }
                    PermissionStatus::Denied => {
                        self.permissions_resolved = true;
                        self.error = Some(
                            "Plugin permissions denied (need ReadApplicationState and ChangeApplicationState)."
                                .to_string(),
                        );
                    }
                }
                true
            }
            Event::Key(key) => {
                if !self.permissions_resolved {
                    return true;
                }
                match key.bare_key {
                    BareKey::Esc => close_self(),
                    BareKey::Enter => self.open_selected(),
                    BareKey::Up | BareKey::Char('k') if key.has_no_modifiers() => {
                        self.move_selection(-1)
                    }
                    BareKey::Down | BareKey::Char('j') if key.has_no_modifiers() => {
                        self.move_selection(1)
                    }
                    BareKey::Char('r') if key.has_no_modifiers() => self.load_entries(),
                    _ => {}
                }
                true
            }
            Event::Mouse(mouse) => {
                if !self.permissions_resolved {
                    return true;
                }
                match mouse {
                    Mouse::LeftClick(line, _col) => {
                        let line = line as usize;
                        const HEADER_ROWS: usize = 2; // title + hint
                        if line >= HEADER_ROWS {
                            let idx = self.scroll + (line - HEADER_ROWS);
                            if idx < self.entries.len() {
                                self.selected = idx;
                            }
                        }
                        true
                    }
                    Mouse::ScrollUp(_) => {
                        self.move_selection(-1);
                        true
                    }
                    Mouse::ScrollDown(_) => {
                        self.move_selection(1);
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let cols = cols.max(1);

        let title = "PrePane";
        println!("{title: <width$}", width = cols);

        if !self.permissions_resolved {
            let msg = "Waiting for permissions…";
            println!("{msg: <width$}", width = cols);
            for _ in 2..rows {
                println!("{: <width$}", "", width = cols);
            }
            return;
        }

        let content_rows = rows.saturating_sub(1);
        if let Some(err) = &self.error {
            let mut err_lines = Self::wrap_text_to_width(err, cols);
            if err_lines.is_empty() {
                err_lines.push(String::new());
            }

            let max_err_rows = content_rows.max(1);
            if err_lines.len() > max_err_rows {
                err_lines.truncate(max_err_rows.saturating_sub(1));
                err_lines.push("… (error truncated; widen floating pane)".to_string());
            }

            for line in &err_lines {
                println!("{line: <width$}", width = cols);
            }

            for _ in err_lines.len()..content_rows {
                println!("{: <width$}", "", width = cols);
            }
            return;
        }

        let hint = "Up/Down: select • Enter: open • Esc: close • r: reload";
        let hint_reserved = 1usize.min(content_rows);
        let hint_line = Self::wrap_text_to_width(hint, cols)
            .into_iter()
            .next()
            .unwrap_or_default();
        println!("{hint_line: <width$}", width = cols);
        for _ in 1..hint_reserved {
            println!("{: <width$}", "", width = cols);
        }

        let list_rows = content_rows.saturating_sub(hint_reserved);
        self.ensure_visible(list_rows);

        for i in 0..list_rows {
            let idx = self.scroll + i;
            if idx >= self.entries.len() {
                println!("{: <width$}", "", width = cols);
                continue;
            }
            let entry = &self.entries[idx];
            let is_selected = idx == self.selected;

            let prefix = if is_selected { "> " } else { "  " };
            let mut line = format!("{prefix}{}", entry.label);
            if line.len() > cols {
                line.truncate(cols.saturating_sub(1));
            }
            println!("{line: <width$}", width = cols);
        }
    }
}

