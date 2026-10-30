//! Generic tree that both XMind flavours are loaded into.

/// An XMind "summary" or "boundary": brackets children `start..=end` of a parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    Summary,
    Boundary,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub kind: GroupKind,
    pub start: i64,
    pub end: i64,
    pub title: String,
    pub note: String,
}

#[derive(Debug, Clone, Default)]
pub struct Topic {
    pub title: String,
    pub note: String,
    pub children: Vec<Topic>,
    pub groups: Vec<Group>,
    pub markers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Sheet {
    pub title: String,
    pub root: Topic,
}

impl Topic {
    /// Title with markers appended, as used by the graph emitters.
    pub fn label(&self) -> String {
        if self.markers.is_empty() {
            self.title.clone()
        } else {
            format!("{} [{}]", self.title, self.markers.join(", "))
        }
    }

    /// Indices of children covered by `group`, clamped to the child list.
    pub fn covered(&self, group: &Group) -> impl Iterator<Item = usize> + '_ {
        let last = self.children.len() as i64 - 1;
        let (start, end) = (group.start, group.end.min(last));
        (start..=end).filter(|i| *i >= 0).map(|i| i as usize)
    }

    pub fn covered_names(&self, group: &Group) -> String {
        self.covered(group)
            .map(|i| self.children[i].title.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn count(&self) -> Counts {
        let mut c = Counts {
            topics: 1,
            notes: usize::from(!self.note.is_empty()),
            ..Counts::default()
        };
        for g in &self.groups {
            match g.kind {
                GroupKind::Summary => c.summaries += 1,
                GroupKind::Boundary => c.boundaries += 1,
            }
        }
        for child in &self.children {
            c += child.count();
        }
        c
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Counts {
    pub topics: usize,
    pub notes: usize,
    pub summaries: usize,
    pub boundaries: usize,
}

impl std::ops::AddAssign for Counts {
    fn add_assign(&mut self, o: Counts) {
        self.topics += o.topics;
        self.notes += o.notes;
        self.summaries += o.summaries;
        self.boundaries += o.boundaries;
    }
}
