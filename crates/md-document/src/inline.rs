//! Inline content: flat text runs, formatting marks, and atomic inline items.

use bitflags::bitflags;
use md_core::{TextBuf, TextOffset};

use crate::{DocumentError, DocumentResult};

bitflags! {
    /// Formatting flags applied to an inline run.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct MarkFlags: u8 {
        /// Strong emphasis (`**` or `__`).
        const STRONG = 1 << 0;
        /// Emphasis (`*` or `_`).
        const EMPHASIS = 1 << 1;
        /// Strikethrough (`~~`).
        const STRIKE = 1 << 2;
        /// Underline (`<u>`).
        const UNDERLINE = 1 << 3;
        /// Inline code (backticks).
        const CODE = 1 << 4;
        /// Highlight (`==`).
        const HIGHLIGHT = 1 << 5;
    }
}

/// A link destination and optional title attached to a text range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkRef {
    destination: String,
    title: Option<String>,
}

impl LinkRef {
    /// Creates a link reference without a title.
    #[must_use]
    pub fn new(destination: impl Into<String>) -> Self {
        Self {
            destination: destination.into(),
            title: None,
        }
    }

    /// Adds a title to this link reference.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Returns the link destination.
    #[must_use]
    pub fn destination(&self) -> &str {
        &self.destination
    }

    /// Returns the optional link title.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

/// An image destination and optional title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRef {
    destination: String,
    title: Option<String>,
}

impl ImageRef {
    /// Creates an image reference without a title.
    #[must_use]
    pub fn new(destination: impl Into<String>) -> Self {
        Self {
            destination: destination.into(),
            title: None,
        }
    }

    /// Adds a title to this image reference.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Returns the image destination.
    #[must_use]
    pub fn destination(&self) -> &str {
        &self.destination
    }

    /// Returns the optional image title.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

/// Inline formatting applied to a run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    /// Formatting flags.
    pub flags: MarkFlags,
    /// Optional link range.
    pub link: Option<LinkRef>,
}

impl Marks {
    /// Returns marks with no formatting and no link.
    #[must_use]
    pub const fn plain() -> Self {
        Self {
            flags: MarkFlags::empty(),
            link: None,
        }
    }

    /// Creates marks with the given flags.
    #[must_use]
    pub const fn with_flags(flags: MarkFlags) -> Self {
        Self { flags, link: None }
    }

    /// Adds a link to these marks.
    #[must_use]
    pub fn with_link(mut self, link: LinkRef) -> Self {
        self.link = Some(link);
        self
    }

    /// Returns `true` when no flags and no link are set.
    #[must_use]
    pub fn is_plain(&self) -> bool {
        self.flags.is_empty() && self.link.is_none()
    }
}

/// An indivisible inline item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtomKind {
    /// A hard line break.
    HardBreak,
    /// A soft line break.
    SoftBreak,
    /// An inline image.
    Image(ImageRef),
    /// Inline math source.
    InlineMath(TextBuf),
    /// A footnote reference label.
    FootnoteRef(String),
}

/// A contiguous piece of inline content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Run {
    /// Text with formatting marks.
    Text {
        /// UTF-8 text.
        text: TextBuf,
        /// Formatting marks.
        marks: Marks,
    },
    /// An atomic inline item.
    Atom {
        /// Atom payload.
        kind: AtomKind,
        /// Formatting marks.
        marks: Marks,
    },
}

impl Run {
    /// Creates a text run.
    #[must_use]
    pub fn text(text: impl Into<TextBuf>, marks: Marks) -> Self {
        Self::Text {
            text: text.into(),
            marks,
        }
    }

    /// Creates an atom run.
    #[must_use]
    pub fn atom(kind: AtomKind, marks: Marks) -> Self {
        Self::Atom { kind, marks }
    }

    /// Returns the run length in `DocPos` units.
    ///
    /// Text runs use UTF-8 byte length; atoms occupy one unit.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Text { text, .. } => text.len_bytes(),
            Self::Atom { .. } => 1,
        }
    }

    /// Returns `true` for an empty text run.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Text { text, .. } if text.is_empty())
    }

    /// Returns the formatting marks.
    #[must_use]
    pub fn marks(&self) -> &Marks {
        match self {
            Self::Text { marks, .. } | Self::Atom { marks, .. } => marks,
        }
    }

    /// Replaces the formatting marks.
    pub fn set_marks(&mut self, marks: Marks) {
        match self {
            Self::Text { marks: current, .. } | Self::Atom { marks: current, .. } => {
                *current = marks
            }
        }
    }

    /// Returns the text buffer for a text run.
    #[must_use]
    pub fn text_ref(&self) -> Option<&TextBuf> {
        match self {
            Self::Text { text, .. } => Some(text),
            Self::Atom { .. } => None,
        }
    }

    /// Returns the atom payload for an atom run.
    #[must_use]
    pub fn atom_kind(&self) -> Option<&AtomKind> {
        match self {
            Self::Text { .. } => None,
            Self::Atom { kind, .. } => Some(kind),
        }
    }
}

/// A summary of changes made by [`InlineContent::normalize_with_report`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InlineNormalizeChange {
    /// Number of empty text runs removed.
    pub removed_empty_runs: usize,
    /// Number of adjacent text runs merged.
    pub merged_text_runs: usize,
}

impl InlineNormalizeChange {
    /// Returns `true` when normalization changed nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.removed_empty_runs == 0 && self.merged_text_runs == 0
    }
}

/// Flat inline content: an ordered sequence of text and atom runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InlineContent {
    runs: Vec<Run>,
    len: usize,
}

impl InlineContent {
    /// Creates empty inline content.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates inline content from plain text.
    #[must_use]
    pub fn from_text(text: impl Into<TextBuf>) -> Self {
        let text = text.into();
        if text.is_empty() {
            return Self::new();
        }

        let len = text.len_bytes();
        Self {
            runs: vec![Run::text(text, Marks::plain())],
            len,
        }
    }

    /// Returns the content length in `DocPos` units.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when there are no runs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Returns `true` when `offset` is a legal local inline boundary.
    ///
    /// Text runs require a UTF-8 character boundary; atom runs only accept
    /// their start or end offset.
    #[must_use]
    pub fn is_valid_offset(&self, offset: usize) -> bool {
        if offset > self.len {
            return false;
        }

        let mut remaining = offset;
        for run in &self.runs {
            let run_len = run.len();
            if remaining <= run_len {
                return match run {
                    Run::Text { text, .. } => text.is_char_boundary(TextOffset::new(remaining)),
                    Run::Atom { .. } => remaining == 0 || remaining == 1,
                };
            }
            remaining -= run_len;
        }

        remaining == 0
    }

    /// Returns the previous grapheme/atom boundary before `offset`.
    #[must_use]
    pub fn prev_offset(&self, offset: usize) -> Option<usize> {
        if offset == 0 || offset > self.len || !self.is_valid_offset(offset) {
            return None;
        }

        let mut run_start = 0;
        for run in &self.runs {
            let run_end = run_start + run.len();
            if offset <= run_end {
                let local = offset - run_start;
                return match run {
                    Run::Text { text, .. } => text
                        .prev_grapheme_boundary(TextOffset::new(local))
                        .map(|previous| run_start + previous.get()),
                    Run::Atom { .. } => Some(run_start),
                };
            }
            run_start = run_end;
        }

        None
    }

    /// Returns the next grapheme/atom boundary after `offset`.
    #[must_use]
    pub fn next_offset(&self, offset: usize) -> Option<usize> {
        if offset >= self.len || !self.is_valid_offset(offset) {
            return None;
        }

        let mut run_start = 0;
        for run in &self.runs {
            let run_end = run_start + run.len();
            if offset < run_end {
                let local = offset - run_start;
                return match run {
                    Run::Text { text, .. } => text
                        .next_grapheme_boundary(TextOffset::new(local))
                        .map(|next| run_start + next.get()),
                    Run::Atom { .. } => Some(run_end),
                };
            }
            run_start = run_end;
        }

        None
    }

    /// Returns the run slice.
    #[must_use]
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// Appends a non-empty run without normalizing adjacent marks.
    ///
    /// # Errors
    ///
    /// Returns an invariant error for an empty text run.
    pub fn push_run(&mut self, run: Run) -> DocumentResult<()> {
        if run.is_empty() {
            return Err(DocumentError::Invariant {
                message: "empty text run".to_owned(),
            });
        }

        self.len += run.len();
        self.runs.push(run);
        Ok(())
    }

    /// Inserts a non-empty run at `index`.
    ///
    /// # Errors
    ///
    /// Returns an invariant error for an out-of-bounds index or empty text run.
    pub fn insert_run(&mut self, index: usize, run: Run) -> DocumentResult<()> {
        if index > self.runs.len() {
            return Err(DocumentError::Invariant {
                message: format!("inline run index {index} is out of bounds"),
            });
        }
        if run.is_empty() {
            return Err(DocumentError::Invariant {
                message: "empty text run".to_owned(),
            });
        }

        self.len += run.len();
        self.runs.insert(index, run);
        Ok(())
    }

    /// Removes the run at `index`, updating the content length.
    pub fn remove_run(&mut self, index: usize) -> Option<Run> {
        if index >= self.runs.len() {
            return None;
        }

        let run = self.runs.remove(index);
        self.len -= run.len();
        Some(run)
    }

    /// Appends plain text, merging with the previous run when marks match.
    pub fn push_text(&mut self, text: &str, marks: Marks) {
        if text.is_empty() {
            return;
        }

        if let Some(Run::Text {
            text: previous,
            marks: previous_marks,
        }) = self.runs.last_mut()
            && previous_marks == &marks
        {
            previous.append(text);
            self.len += text.len();
            return;
        }

        self.runs.push(Run::text(text, marks));
        self.len += text.len();
    }

    /// Appends an atom.
    pub fn push_atom(&mut self, kind: AtomKind, marks: Marks) {
        self.runs.push(Run::atom(kind, marks));
        self.len += 1;
    }

    /// Inserts an atom at `index`.
    ///
    /// # Errors
    ///
    /// Returns an invariant error when `index` is out of bounds.
    pub fn insert_atom(
        &mut self,
        index: usize,
        kind: AtomKind,
        marks: Marks,
    ) -> DocumentResult<()> {
        self.insert_run(index, Run::atom(kind, marks))
    }

    /// Splits a text run at a local byte offset.
    ///
    /// Returns `true` when a split occurred and `false` at the run endpoints.
    ///
    /// # Errors
    ///
    /// Returns an invariant error for an invalid run index or non-text run, and
    /// a text error when the offset is not a valid UTF-8 boundary.
    pub fn split_text_run(&mut self, index: usize, offset: TextOffset) -> DocumentResult<bool> {
        let run = self
            .runs
            .get_mut(index)
            .ok_or_else(|| DocumentError::Invariant {
                message: format!("inline run index {index} is out of bounds"),
            })?;

        let Run::Text { text, marks } = run else {
            return Err(DocumentError::Invariant {
                message: "cannot split an atom run".to_owned(),
            });
        };

        text.validate_offset(offset)?;
        if offset.get() == 0 || offset.get() == text.len_bytes() {
            return Ok(false);
        }

        let marks = marks.clone();
        let tail = text.split_off(offset)?;
        self.runs.insert(index + 1, Run::text(tail, marks));
        Ok(true)
    }

    /// Removes empty text runs and merges adjacent text runs with equal marks.
    ///
    /// Returns `true` when the content changed.
    pub fn normalize(&mut self) -> bool {
        !self.normalize_with_report().is_empty()
    }

    /// Normalizes and reports what changed for later position mapping.
    pub fn normalize_with_report(&mut self) -> InlineNormalizeChange {
        let mut change = InlineNormalizeChange::default();
        let mut normalized = Vec::with_capacity(self.runs.len());

        for run in self.runs.drain(..) {
            if run.is_empty() {
                change.removed_empty_runs += 1;
                continue;
            }

            let mut merged = false;
            if let Some(Run::Text {
                text: previous,
                marks: previous_marks,
            }) = normalized.last_mut()
                && let Run::Text { text, marks } = &run
                && *previous_marks == *marks
            {
                previous.append(text.as_str());
                merged = true;
                change.merged_text_runs += 1;
            }

            if !merged {
                normalized.push(run);
            }
        }

        self.runs = normalized;
        self.recompute_len();
        change
    }

    /// Validates length, empty runs, and adjacent-run normalization.
    ///
    /// # Errors
    ///
    /// Returns an invariant error when a rule is violated.
    pub fn validate(&self) -> DocumentResult<()> {
        let mut total = 0;
        for run in &self.runs {
            if run.is_empty() {
                return Err(DocumentError::Invariant {
                    message: "inline content contains an empty text run".to_owned(),
                });
            }
            total += run.len();
        }

        for pair in self.runs.windows(2) {
            if let (
                Run::Text {
                    marks: previous, ..
                },
                Run::Text { marks: next, .. },
            ) = (&pair[0], &pair[1])
                && previous == next
            {
                return Err(DocumentError::Invariant {
                    message: "adjacent text runs with equal marks were not merged".to_owned(),
                });
            }
        }

        if total != self.len {
            return Err(DocumentError::Invariant {
                message: format!(
                    "inline length {} does not match run total {total}",
                    self.len
                ),
            });
        }

        Ok(())
    }

    fn recompute_len(&mut self) {
        self.len = self.runs.iter().map(Run::len).sum();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AtomKind, ImageRef, InlineContent, InlineNormalizeChange, LinkRef, MarkFlags, Marks, Run,
    };
    use crate::DocumentError;
    use md_core::TextOffset;

    fn offset(value: usize) -> TextOffset {
        TextOffset::new(value)
    }

    #[test]
    fn marks_combine_flags_and_link() {
        let marks = Marks::with_flags(MarkFlags::STRONG | MarkFlags::EMPHASIS)
            .with_link(LinkRef::new("https://example.com").with_title("example"));
        assert!(marks.flags.contains(MarkFlags::STRONG));
        assert!(marks.flags.contains(MarkFlags::EMPHASIS));
        assert_eq!(
            marks.link.as_ref().unwrap().destination(),
            "https://example.com"
        );
        assert_eq!(marks.link.as_ref().unwrap().title(), Some("example"));
        assert!(!marks.is_plain());
        assert!(Marks::plain().is_plain());
    }

    #[test]
    fn atom_run_has_length_one() {
        let run = Run::atom(AtomKind::HardBreak, Marks::plain());
        assert_eq!(run.len(), 1);
        assert!(!run.is_empty());
        assert!(run.atom_kind().is_some());
        assert!(run.text_ref().is_none());
    }

    #[test]
    fn inline_content_from_text_uses_utf8_length() {
        let content = InlineContent::from_text("你好");
        assert_eq!(content.len(), 6);
        assert_eq!(content.runs().len(), 1);
        assert!(content.validate().is_ok());
        assert!(InlineContent::from_text("").is_empty());
    }

    #[test]
    fn push_text_merges_equal_marks_and_splits_different_marks() {
        let mut content = InlineContent::new();
        content.push_text("a", Marks::plain());
        content.push_text("b", Marks::plain());
        content.push_text("c", Marks::with_flags(MarkFlags::STRONG));
        assert_eq!(content.runs().len(), 2);
        assert_eq!(content.len(), 3);
        assert_eq!(content.runs()[0].text_ref().unwrap().as_str(), "ab");
    }

    #[test]
    fn insert_and_remove_run_update_length() {
        let mut content = InlineContent::from_text("ac");
        content
            .insert_run(1, Run::text("b", Marks::plain()))
            .unwrap();
        assert_eq!(content.len(), 3);
        assert!(matches!(
            content.insert_run(9, Run::text("x", Marks::plain())),
            Err(DocumentError::Invariant { .. })
        ));

        let removed = content.remove_run(1).unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(content.len(), 2);
        assert!(content.remove_run(9).is_none());
    }

    #[test]
    fn split_text_run_preserves_length_and_marks() {
        let marks = Marks::with_flags(MarkFlags::CODE);
        let mut content = InlineContent::new();
        content.push_run(Run::text("你好", marks.clone())).unwrap();
        assert!(content.split_text_run(0, offset(3)).unwrap());
        assert_eq!(content.runs().len(), 2);
        assert_eq!(content.len(), 6);
        assert_eq!(content.runs()[0].marks(), &marks);
        assert_eq!(content.runs()[1].marks(), &marks);
        assert!(!content.split_text_run(0, offset(0)).unwrap());
        assert!(!content.split_text_run(0, offset(3)).unwrap());
        assert!(matches!(
            content.split_text_run(0, offset(1)),
            Err(DocumentError::Text(md_core::TextError::NotCharBoundary {
                offset: 1
            }))
        ));
    }

    #[test]
    fn split_atom_run_is_rejected() {
        let mut content = InlineContent::new();
        content.push_atom(AtomKind::SoftBreak, Marks::plain());
        assert!(matches!(
            content.split_text_run(0, offset(0)),
            Err(DocumentError::Invariant { .. })
        ));
    }

    #[test]
    fn normalize_merges_adjacent_text() {
        let mut content = InlineContent::new();
        content.push_run(Run::text("a", Marks::plain())).unwrap();
        content.push_run(Run::text("b", Marks::plain())).unwrap();
        content.push_run(Run::text("", Marks::plain())).unwrap_err();
        content.push_atom(AtomKind::HardBreak, Marks::plain());
        content.push_run(Run::text("c", Marks::plain())).unwrap();

        assert!(content.normalize());
        assert_eq!(content.runs().len(), 3);
        assert_eq!(content.len(), 4);
        assert!(content.validate().is_ok());
        assert!(!content.normalize());
    }

    #[test]
    fn normalize_does_not_merge_across_atom() {
        let mut content = InlineContent::new();
        content.push_text("a", Marks::plain());
        content.push_atom(AtomKind::SoftBreak, Marks::plain());
        content.push_text("b", Marks::plain());
        content.normalize();
        assert_eq!(content.runs().len(), 3);
    }

    #[test]
    fn validate_rejects_adjacent_equal_runs() {
        let mut content = InlineContent::from_text("a");
        content
            .insert_run(1, Run::text("b", Marks::plain()))
            .unwrap();
        assert!(matches!(
            content.validate(),
            Err(DocumentError::Invariant { .. })
        ));
    }

    #[test]
    fn image_ref_accessors_work() {
        let image = ImageRef::new("cover.png").with_title("Cover");
        assert_eq!(image.destination(), "cover.png");
        assert_eq!(image.title(), Some("Cover"));
        let atom = AtomKind::Image(image);
        let run = Run::atom(atom, Marks::plain());
        assert!(matches!(run.atom_kind(), Some(AtomKind::Image(_))));
    }

    #[test]
    fn insert_atom_at_index_updates_length() {
        let mut content = InlineContent::from_text("ab");
        content
            .insert_atom(0, AtomKind::HardBreak, Marks::plain())
            .unwrap();
        assert_eq!(content.len(), 3);
        assert_eq!(content.runs().len(), 2);
        assert!(matches!(
            content.runs()[0].atom_kind(),
            Some(AtomKind::HardBreak)
        ));
        assert!(
            content
                .insert_atom(9, AtomKind::SoftBreak, Marks::plain())
                .is_err()
        );
    }

    #[test]
    fn normalize_report_counts_merges() {
        let mut content = InlineContent::new();
        content.push_run(Run::text("a", Marks::plain())).unwrap();
        content.push_run(Run::text("b", Marks::plain())).unwrap();

        let report = content.normalize_with_report();
        assert_eq!(report.merged_text_runs, 1);
        assert_eq!(report.removed_empty_runs, 0);
        assert!(!report.is_empty());
        assert!(!content.normalize());
    }

    #[test]
    fn normalize_report_is_empty_when_clean() {
        let mut content = InlineContent::from_text("a");
        let report = content.normalize_with_report();
        assert_eq!(report, InlineNormalizeChange::default());
        assert!(report.is_empty());
    }

    #[test]
    fn valid_offsets_respect_text_and_atom_boundaries() {
        let mut content = InlineContent::from_text("你");
        content.push_atom(AtomKind::HardBreak, Marks::plain());
        content.push_text("a", Marks::plain());

        assert!(content.is_valid_offset(0));
        assert!(content.is_valid_offset(3));
        assert!(!content.is_valid_offset(1));
        assert!(content.is_valid_offset(4));
        assert!(content.is_valid_offset(5));
        assert!(!content.is_valid_offset(6));
    }

    #[test]
    fn prev_and_next_offsets_follow_graphemes_and_atoms() {
        let mut content = InlineContent::from_text("a你");
        content.push_atom(AtomKind::HardBreak, Marks::plain());
        content.push_text("b", Marks::plain());

        assert_eq!(content.next_offset(0), Some(1));
        assert_eq!(content.next_offset(1), Some(4));
        assert_eq!(content.next_offset(4), Some(5));
        assert_eq!(content.next_offset(5), Some(6));
        assert_eq!(content.next_offset(6), None);

        assert_eq!(content.prev_offset(6), Some(5));
        assert_eq!(content.prev_offset(5), Some(4));
        assert_eq!(content.prev_offset(4), Some(1));
        assert_eq!(content.prev_offset(1), Some(0));
        assert_eq!(content.prev_offset(0), None);
        assert_eq!(content.prev_offset(2), None);
    }
}
