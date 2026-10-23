use std::collections::HashSet;

use od_core::EntityId;

/// What an [`Action`](crate::Action) did to a drawing: the ids it added,
/// removed and changed, and whether it touched the layer table or the unit
/// settings. Panels refresh
/// from this, and plugins will subscribe to it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Changes {
    pub added: Vec<EntityId>,
    pub removed: Vec<EntityId>,
    pub changed: Vec<EntityId>,
    pub layers: bool,
    pub units: bool,
    pub dim_style: bool,
    pub layouts: bool,
    /// The block table changed (a block defined, redefined or purged).
    pub blocks: bool,
    /// The linetype table or `LTSCALE` changed.
    pub linetypes: bool,
    /// The text-style table or the current text style changed.
    pub text_styles: bool,
    /// The multileader-style table or the current multileader style changed.
    pub mleader_styles: bool,
    /// `PDMODE` or `PDSIZE` changed.
    pub point_style: bool,
}

impl Changes {
    pub fn added(ids: Vec<EntityId>) -> Self {
        Self {
            added: ids,
            ..Self::default()
        }
    }

    pub fn removed(ids: Vec<EntityId>) -> Self {
        Self {
            removed: ids,
            ..Self::default()
        }
    }

    pub fn changed(ids: Vec<EntityId>) -> Self {
        Self {
            changed: ids,
            ..Self::default()
        }
    }

    pub fn layers() -> Self {
        Self {
            layers: true,
            ..Self::default()
        }
    }

    pub fn units() -> Self {
        Self {
            units: true,
            ..Self::default()
        }
    }

    pub fn dim_style() -> Self {
        Self {
            dim_style: true,
            ..Self::default()
        }
    }

    pub fn layouts() -> Self {
        Self {
            layouts: true,
            ..Self::default()
        }
    }

    pub fn blocks() -> Self {
        Self {
            blocks: true,
            ..Self::default()
        }
    }

    pub fn linetypes() -> Self {
        Self {
            linetypes: true,
            ..Self::default()
        }
    }

    pub fn text_styles() -> Self {
        Self {
            text_styles: true,
            ..Self::default()
        }
    }

    pub fn mleader_styles() -> Self {
        Self {
            mleader_styles: true,
            ..Self::default()
        }
    }

    pub fn point_style() -> Self {
        Self {
            point_style: true,
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && !self.layers
            && !self.units
            && !self.dim_style
            && !self.layouts
            && !self.blocks
            && !self.linetypes
            && !self.text_styles
            && !self.mleader_styles
            && !self.point_style
    }

    /// The combined report for changes made one after another. An object
    /// added and then removed within the sequence is not reported at all, and
    /// one that was added or removed is not also reported as changed.
    pub fn merged(parts: impl IntoIterator<Item = Changes>) -> Self {
        let mut all = Changes::default();
        for part in parts {
            all.added.extend(part.added);
            all.removed.extend(part.removed);
            all.changed.extend(part.changed);
            all.layers |= part.layers;
            all.units |= part.units;
            all.dim_style |= part.dim_style;
            all.layouts |= part.layouts;
            all.blocks |= part.blocks;
            all.linetypes |= part.linetypes;
            all.text_styles |= part.text_styles;
            all.mleader_styles |= part.mleader_styles;
            all.point_style |= part.point_style;
        }
        let removed: HashSet<EntityId> = all.removed.iter().copied().collect();
        let cancelled: HashSet<EntityId> = all
            .added
            .iter()
            .filter(|id| removed.contains(id))
            .copied()
            .collect();
        all.added.retain(|id| !cancelled.contains(id));
        all.removed.retain(|id| !cancelled.contains(id));
        let added: HashSet<EntityId> = all.added.iter().copied().collect();
        let removed: HashSet<EntityId> = all.removed.iter().copied().collect();
        all.changed
            .retain(|id| !added.contains(id) && !removed.contains(id));
        all.changed.sort_unstable();
        all.changed.dedup();
        all
    }

    /// The report for undoing the change: what was added is now removed.
    pub fn inverted(self) -> Self {
        Self {
            added: self.removed,
            removed: self.added,
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverting_swaps_added_and_removed() {
        let mut changes = Changes::added(vec![EntityId(1)]);
        changes.changed = vec![EntityId(2)];
        let inverted = changes.inverted();
        assert_eq!(inverted.removed, vec![EntityId(1)]);
        assert!(inverted.added.is_empty());
        assert_eq!(inverted.changed, vec![EntityId(2)]);
    }

    #[test]
    fn merging_cancels_add_then_remove_and_folds_changes_into_adds() {
        let merged = Changes::merged([
            Changes::added(vec![EntityId(1), EntityId(2)]),
            Changes::changed(vec![EntityId(1), EntityId(3), EntityId(3)]),
            Changes::removed(vec![EntityId(2)]),
            Changes::layers(),
        ]);
        assert_eq!(merged.added, vec![EntityId(1)]);
        assert!(merged.removed.is_empty());
        assert_eq!(merged.changed, vec![EntityId(3)]);
        assert!(merged.layers);
    }

    #[test]
    fn nothing_reported_is_empty() {
        assert!(Changes::default().is_empty());
        assert!(!Changes::layers().is_empty());
        assert!(!Changes::units().is_empty());
        assert!(!Changes::layouts().is_empty());
    }
}
