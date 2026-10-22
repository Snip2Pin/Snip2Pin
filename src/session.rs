/// Track one active capture request and reject stale or duplicated results.
#[derive(Default)]
pub struct CaptureGate {
    next: u64,
    active: Option<u64>,
}

impl CaptureGate {
    pub fn active(&self) -> Option<u64> {
        self.active
    }

    pub fn cancel(&mut self) {
        self.active = None;
    }
    pub fn begin(&mut self) -> Option<u64> {
        if self.active.is_some() {
            return None;
        }
        self.next = self.next.wrapping_add(1);
        self.active = Some(self.next);
        self.active
    }

    pub fn complete(&mut self, id: u64) -> bool {
        if self.active != Some(id) {
            return false;
        }
        self.active = None;
        true
    }
}
