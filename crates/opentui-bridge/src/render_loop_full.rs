#![forbid(unsafe_code)]

//! Dirty-gated full render loop over [`RenderQueue`] + [`assemble_frame`].
//!
//! ponytail: first queued note becomes toast; join policy when caller needs it.

use crate::frame_assemble::assemble_frame;
use crate::render_queue::RenderQueue;

/// Render loop: dirty gate, frame counter, take-reset.
pub struct RenderLoop {
    pub queue: RenderQueue,
    pub frames: u64,
}

impl RenderLoop {
    /// Clean loop, zero frames.
    pub fn new() -> Self {
        Self {
            queue: RenderQueue::new(),
            frames: 0,
        }
    }

    /// Mark view dirty.
    pub fn request(&mut self) {
        self.queue.mark_dirty();
    }

    /// None when clean; else Some frame, frames bump, queue reset.
    pub fn render(
        &mut self,
        title: &str,
        status: &str,
        transcript: &[String],
        draft: &str,
        w: usize,
        h: usize,
    ) -> Option<Vec<String>> {
        let (dirty, notes) = self.queue.take();
        if !dirty {
            return None;
        }
        let toast = notes.first().cloned();
        let frame = assemble_frame(title, status, transcript, draft, toast.as_deref(), w, h);
        self.frames += 1;
        Some(frame)
    }
}

impl Default for RenderLoop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn new_renders_none_when_clean() {
        let mut l = RenderLoop::new();
        assert!(l.render("T", "S", &t(&["hi"]), "d", 40, 8).is_none());
        assert_eq!(l.frames, 0);
    }

    #[test]
    fn request_renders_some_and_bumps() {
        let mut l = RenderLoop::new();
        l.request();
        let f = l.render("T", "S", &t(&["hi"]), "d", 40, 8).unwrap();
        assert_eq!(f.len(), 8);
        assert_eq!(l.frames, 1);
    }

    #[test]
    fn render_resets_so_second_is_none() {
        let mut l = RenderLoop::new();
        l.request();
        assert!(l.render("T", "S", &[], "", 40, 8).is_some());
        assert!(l.render("T", "S", &[], "", 40, 8).is_none());
        assert_eq!(l.frames, 1);
    }

    #[test]
    fn re_request_renders_again() {
        let mut l = RenderLoop::new();
        l.request();
        assert!(l.render("T", "S", &[], "", 40, 8).is_some());
        l.request();
        assert!(l.render("T", "S", &[], "", 40, 8).is_some());
        assert_eq!(l.frames, 2);
    }

    #[test]
    fn first_note_becomes_toast() {
        let mut l = RenderLoop::new();
        l.request();
        l.queue.push_note("saved");
        let f = l.render("T", "S", &t(&["hi"]), "d", 40, 8).unwrap();
        assert_eq!(f[7], "saved");
    }

    #[test]
    fn height_clamps_to_eight() {
        let mut l = RenderLoop::new();
        l.request();
        let f = l.render("T", "S", &[], "", 40, 2).unwrap();
        assert_eq!(f.len(), 8);
    }
}
