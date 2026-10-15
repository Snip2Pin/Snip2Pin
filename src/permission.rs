//! Permission decisions happen only in response to a user's capture action.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScreenPermission {
    #[default]
    Unchecked,
    Required,
    Granted,
}

impl ScreenPermission {
    pub fn authorize(
        &mut self,
        mut preflight: impl FnMut() -> bool,
        request: impl FnOnce() -> bool,
    ) -> bool {
        if preflight() {
            *self = Self::Granted;
            return true;
        }
        if *self == Self::Unchecked {
            let _ = request();
        }
        *self = if preflight() {
            Self::Granted
        } else {
            Self::Required
        };
        *self == Self::Granted
    }
}
