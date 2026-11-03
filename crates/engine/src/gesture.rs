//! Transient cancellation of one touch gesture, including its previous redo branch.

use std::sync::Arc;

use crate::{EditorState, History, Session};
use filmcraft_project::Project;

/// Async imports/opens keep their order and stay queued until the gesture's final UI pass.
pub fn drain_pending<T>(pending: &mut Vec<T>, input_owned: bool) -> Vec<T> {
    if input_owned { Vec::new() } else { std::mem::take(pending) }
}

pub struct Checkpoint {
    project: Arc<Project>,
    history: History,
    state: EditorState,
    document_epoch: u64,
    clean: bool,
}

impl Checkpoint {
    pub fn capture(session: &Session) -> Self {
        Self {
            project: session.project.clone(),
            history: session.history.clone(),
            state: session.state.clone(),
            document_epoch: session.document_epoch,
            clean: !session.is_dirty(),
        }
    }

    pub fn cancel(self, session: &mut Session) -> bool {
        if session.document_epoch != self.document_epoch {
            return false;
        }
        let changed = !Arc::ptr_eq(&self.project, &session.project);
        session.project = self.project;
        session.history = self.history;
        session.state = self.state;
        session.fix_state();
        if changed {
            session.bump();
            if self.clean {
                session.saved_revision = session.revision;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_touch_restores_content_redo_and_clean_state() {
        let mut s = Session::default();
        let _ = s.edit("One", |p, _| {
            p.name = "One".into();
            Ok(())
        });
        let _ = s.edit("Two", |p, _| {
            p.name = "Two".into();
            Ok(())
        });
        s.undo();
        s.saved_revision = s.revision;
        let redo = s.history.redo.len();
        let before = s.project.clone();
        let checkpoint = Checkpoint::capture(&s);
        let _ = s.edit("Interrupted", |p, _| {
            p.name = "Interrupted".into();
            Ok(())
        });
        assert!(checkpoint.cancel(&mut s));
        assert!(Arc::ptr_eq(&before, &s.project));
        assert_eq!(s.history.redo.len(), redo);
        assert!(!s.is_dirty());
    }

    #[test]
    fn cancellation_cannot_replace_another_document() {
        let mut s = Session::default();
        let checkpoint = Checkpoint::capture(&s);
        s.document_epoch += 1;
        let replacement = s.project.clone();
        assert!(!checkpoint.cancel(&mut s));
        assert!(Arc::ptr_eq(&replacement, &s.project));
    }

    #[test]
    fn queued_import_survives_touch_cancel_and_runs_after_the_boundary() {
        let mut s = Session::default();
        let original = s.project.clone();
        let checkpoint = Checkpoint::capture(&s);
        s.edit("Touch", |project, _| {
            project.name = "Uncommitted touch".into();
            Ok(())
        })
        .expect("valid gesture edit");
        let mut pending = vec!["Imported A", "Imported B"];
        assert!(drain_pending(&mut pending, true).is_empty());
        assert!(checkpoint.cancel(&mut s));
        assert!(Arc::ptr_eq(&original, &s.project));
        assert!(drain_pending(&mut pending, true).is_empty());
        assert_eq!(pending, ["Imported A", "Imported B"]);
        for name in drain_pending(&mut pending, false) {
            s.edit("Async import", |project, _| {
                project.name = name.into();
                Ok(())
            })
            .expect("queued import applies");
        }
        assert_eq!(s.project.name, "Imported B");
        assert!(pending.is_empty());
    }
}
