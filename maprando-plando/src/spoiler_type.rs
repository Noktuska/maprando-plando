#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum SpoilerType {
    None,
    Hub,
    Item(usize),
    Flag(usize)
}

pub struct SpoilerTypeTracker {
    spoiler_type: SpoilerType,
    changed: bool,
}

impl SpoilerTypeTracker {
    pub const SALT_PREFIX: &'static str = "details_";

    pub fn new() -> Self {
        SpoilerTypeTracker {
            spoiler_type: SpoilerType::None,
            changed: false
        }
    }

    pub fn get(&self) -> SpoilerType {
        self.spoiler_type
    }

    pub fn set(&mut self, new_type: SpoilerType) {
        self.changed = self.spoiler_type != new_type;
        self.spoiler_type = new_type;
    }

    pub fn reset(&mut self) -> bool {
        let res = self.changed;
        self.changed = false;
        res
    }
}