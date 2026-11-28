//! Locking notes in the app: async PBKDF2 key derivation, unlock, relock, idle auto-lock, and
//! password removal.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use rune::Tx;

use crate::app::{App, Mode, Unlocked};
use crate::layer::{Layer, LockDialog};
use crate::lock;

/// Which background PBKDF2 operation is in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockJobKind {
    Locking,
    Unlocking,
}

enum LockResult {
    Locked { id: u64, rel: String, text: String, key: lock::Key, changing: bool },
    Unlocked { id: u64, rel: String, res: Result<(String, lock::Key), String> },
}

/// Off-UI-thread PBKDF2 key-derivation job (`quire-kdf`).
pub struct LockJob {
    pub kind: LockJobKind,
    rx: Receiver<LockResult>,
    started_at: f64,
    started_wall: Instant,
}

impl App {
    /// Non-`None` while PBKDF2 key derivation is running on the worker thread.
    pub fn lock_busy(&self) -> Option<LockJobKind> {
        self.lock_job.as_ref().map(|j| j.kind)
    }

    /// Aborts any in-flight `LockJobKind::Locking` PBKDF2 job and clears temporary lock-dialog
    /// password fields so a cancelled or dismissed lock dialog never seals the note later.
    pub(crate) fn cancel_lock_job(&mut self) {
        if self.lock_job.as_ref().is_some_and(|j| j.kind == LockJobKind::Locking) {
            self.lock_job = None;
            self.lock_job_seq = self.lock_job_seq.wrapping_add(1);
        }
        self.fields.remove("lock-pass");
        self.fields.remove("lock-repeat");
        self.reveal.remove("lock-pass");
        self.reveal.remove("lock-repeat");
    }

    /// Aborts any in-flight `LockJobKind::Unlocking` PBKDF2 job and clears the inline unlock
    /// prompt state when switching away to another note.
    pub(crate) fn cancel_unlock_job(&mut self) {
        if self.lock_job.as_ref().is_some_and(|j| j.kind == LockJobKind::Unlocking) {
            self.lock_job = None;
            self.lock_job_seq = self.lock_job_seq.wrapping_add(1);
        }
        self.unlock_error = None;
        self.fields.remove("unlock-pass");
        self.reveal.remove("unlock-pass");
    }

    /// Cancels the lock dialog (including any in-flight PBKDF2 job) and closes it.
    pub fn cancel_lock(&mut self, tx: &mut Tx) {
        self.cancel_lock_job();
        self.close_layer(tx);
    }

    /// Seals an unlocked note's new text into the folder copy, to save soon.
    pub(crate) fn store_unlocked(&mut self, rel: &str, text: String) {
        let old_key = self.active_text_key().to_owned();
        let Some(u) = self.unlocked.get_mut(rel) else { return };
        if u.text == text {
            return;
        }
        let old_text = std::mem::replace(&mut u.text, String::new());
        u.set_text(text);
        let sealed = lock::seal(&u.key, &u.text);
        if let Some(v) = &mut self.vault {
            v.put(rel.to_string(), sealed);
        }
        self.mark_dirty_pub(rel);
        if self.active.as_deref() == Some(rel) {
            let new_key = self.active_text_key().to_owned();
            self.on_single_line_edit(old_key, new_key, &old_text, self.active_text());
        }
    }

    /// An unlocked note changed outside the app, through sync or another app. The new text
    /// opens with the key in memory, or the note locks again.
    pub(crate) fn outside_locked_text(&mut self, rel: &str, text: String) {
        let opened = match self.unlocked.get(rel) {
            Some(u) if lock::is_locked(&text) => lock::open_with(&u.key, &text).map(Some),
            Some(_) => Ok(None),
            None => return,
        };
        if let Some(v) = &mut self.vault {
            v.put(rel.to_string(), text.clone());
        }
        match opened {
            Ok(Some(plain)) => {
                if let Some(u) = self.unlocked.get_mut(rel) {
                    u.set_text(plain.clone());
                }
                if let Some(e) = self.edits.get_mut(rel) {
                    let (a, b) = (e.anchor.min(plain.len()), e.focus.min(plain.len()));
                    e.replace_text(plain);
                    e.set_selection(a, b);
                }
            }
            // The password was removed elsewhere: the note is plain now.
            Ok(None) => {
                self.unlocked.remove(rel);
                if let Some(e) = self.edits.get_mut(rel) {
                    e.replace_text(text);
                }
            }
            Err(_) => {
                self.unlocked.remove(rel);
                self.edits.remove(rel);
                self.notice = Some(format!("{} has another password now. Unlock it again.", crate::md::base(rel)));
            }
        }
    }

    /// Opens the dialog that asks for a new password, for the active note.
    pub fn ask_lock(&mut self, tx: &mut Tx) {
        let Some(a) = self.active.clone() else { return };
        if self.note_unlocked(&a) {
            self.relock();
            tx.focus(crate::ui::lock::PASS, false);
            return;
        }
        if self.note_locked(&a) {
            return;
        }
        self.fields.remove("lock-pass");
        self.fields.remove("lock-repeat");
        self.reveal.remove("lock-pass");
        self.reveal.remove("lock-repeat");
        self.open_layer(Layer::Lock(LockDialog::Set(None)), tx);
        tx.focus("lock-pass", false);
    }

    /// Opens the dialog that changes the active unlocked note's password in place.
    pub fn ask_change_password(&mut self, tx: &mut Tx) {
        let Some(a) = self.active.as_deref() else { return };
        if !self.note_unlocked(a) {
            return;
        }
        self.fields.remove("lock-pass");
        self.fields.remove("lock-repeat");
        self.reveal.remove("lock-pass");
        self.reveal.remove("lock-repeat");
        self.open_layer(Layer::Lock(LockDialog::Change(None)), tx);
        tx.focus("lock-pass", false);
    }

    /// Locks (or re-encrypts in place for `Change`) the active note with the password in the
    /// dialog, running PBKDF2 off the UI thread.
    pub fn lock_note(&mut self, tx: &mut Tx) {
        if self.lock_job.is_some() {
            return;
        }
        let changing = matches!(self.layer(), Some(Layer::Lock(LockDialog::Change(_))));
        let pass = self.fields.get("lock-pass").map(|e| e.text.clone()).unwrap_or_default();
        let repeat = self.fields.get("lock-repeat").map(|e| e.text.clone()).unwrap_or_default();
        if pass.chars().count() < lock::MIN_PASSWORD {
            self.refuse_lock(format!("The password needs at least {} characters.", lock::MIN_PASSWORD));
            return;
        }
        if pass != repeat {
            self.refuse_lock("The two passwords are not the same.".into());
            return;
        }
        let Some(rel) = self.active.clone() else { return };
        if changing && !self.note_unlocked(&rel) {
            return;
        }
        let text = self.active_text().to_string();
        if let Some(Layer::Lock(LockDialog::Set(err) | LockDialog::Change(err))) = self.layers.last_mut().map(|o| &mut o.layer) {
            *err = None;
        }
        let (tx_res, rx) = mpsc::channel();
        let waker = self.waker.clone();
        let rel_w = rel.clone();
        self.lock_job_seq = self.lock_job_seq.wrapping_add(1);
        let id = self.lock_job_seq;
        let _ = std::thread::Builder::new().name("quire-kdf".into()).spawn(move || {
            let key = lock::new_key(&pass);
            let _ = tx_res.send(LockResult::Locked { id, rel: rel_w, text, key, changing });
            if let Some(w) = waker {
                w();
            }
        });
        self.lock_job = Some(LockJob { kind: LockJobKind::Locking, rx, started_at: self.now.get(), started_wall: Instant::now() });
        tx.wake_after(Duration::from_millis(16));
    }

    /// Locks `rel` again if unlocked; if `rel` is the active note, moves focus into `unlock-pass`.
    pub fn relock_path(&mut self, rel: &str, tx: &mut Tx) {
        if !self.unlocked.contains_key(rel) {
            return;
        }
        self.flush_notes();
        self.unlocked.remove(rel);
        self.edits.remove(rel);
        if self.active.as_deref() == Some(rel) {
            self.drop_find();
            self.unlock_error = None;
            self.fields.remove("unlock-pass");
            self.reveal.remove("unlock-pass");
            self.board_enforce_kanban(Some(tx));
            tx.focus(crate::ui::lock::PASS, false);
        }
    }

    /// Locks the active note again: its plain text leaves memory.
    pub fn relock(&mut self) {
        let Some(a) = self.active.clone() else { return };
        if !self.unlocked.contains_key(&a) {
            return;
        }
        self.flush_notes();
        self.unlocked.remove(&a);
        self.edits.remove(&a);
        self.drop_find();
        self.unlock_error = None;
        self.fields.remove("unlock-pass");
        self.reveal.remove("unlock-pass");
        self.board_enforce_kanban(None);
    }

    /// Clears any inline validation error on the Lock Password dialog once the user edits either field.
    pub(crate) fn clear_lock_dialog_error(&mut self) {
        if let Some(Layer::Lock(LockDialog::Set(err) | LockDialog::Change(err))) = self.layers.last_mut().map(|o| &mut o.layer) {
            *err = None;
        }
    }

    /// Locks every currently unlocked note (including parked Note-mode notes), clearing plain text
    /// and editor buffers from memory, and focuses `unlock-pass` if the active note is now locked.
    pub fn lock_all_notes(&mut self, tx: &mut Tx) {
        if self.unlocked.is_empty() && !self.parked_has_unlocked() {
            return;
        }
        self.flush_notes();
        let keys: Vec<String> = self.unlocked.keys().cloned().collect();
        self.unlocked.clear();
        for k in keys {
            self.edits.remove(&k);
        }
        self.clear_parked_unlocked();
        self.drop_find();
        self.unlock_error = None;
        self.fields.remove("unlock-pass");
        self.reveal.remove("unlock-pass");
        if self.active_locked() {
            self.board_enforce_kanban(Some(tx));
            tx.focus(crate::ui::lock::PASS, false);
        }
    }

    /// Opens the active locked note with the password in the lock panel, running PBKDF2 off the
    /// UI thread so frames stay responsive.
    pub fn unlock(&mut self, tx: &mut Tx) {
        if self.lock_job.is_some() {
            return;
        }
        let Some(rel) = self.active.clone() else { return };
        let Some(text) = self.files().get(&rel).cloned() else { return };
        let pass = self.fields.get("unlock-pass").map(|e| e.text.clone()).unwrap_or_default();
        self.unlock_error = None;
        let (tx_res, rx) = mpsc::channel();
        let waker = self.waker.clone();
        let rel_w = rel.clone();
        self.lock_job_seq = self.lock_job_seq.wrapping_add(1);
        let id = self.lock_job_seq;
        let _ = std::thread::Builder::new().name("quire-kdf".into()).spawn(move || {
            let res = lock::open(&text, &pass);
            let _ = tx_res.send(LockResult::Unlocked { id, rel: rel_w, res });
            if let Some(w) = waker {
                w();
            }
        });
        self.lock_job = Some(LockJob { kind: LockJobKind::Unlocking, rx, started_at: self.now.get(), started_wall: Instant::now() });
        tx.wake_after(Duration::from_millis(16));
    }

    /// Collects completed background PBKDF2 work without blocking real UI frames.
    pub(crate) fn poll_lock(&mut self, t: f64, tx: &mut Tx) -> bool {
        let Some(job) = self.lock_job.as_ref() else { return false };
        match job.rx.try_recv() {
            Ok(res) => {
                self.lock_job = None;
                self.finish_lock_job(res, tx);
                true
            }
            Err(TryRecvError::Disconnected) => {
                self.lock_job = None;
                true
            }
            Err(TryRecvError::Empty) => {
                // Only in non-realtime simulated clock replays where app time advances far faster
                // than wall time: wait once simulated time has passed 0.25 s so deterministic
                // scripted replays observe the real worker thread's completion.
                let app_dt = t - job.started_at;
                let wall_dt = job.started_wall.elapsed().as_secs_f64();
                if app_dt >= 0.25 && app_dt > wall_dt * 3.0 + 0.15 {
                    if let Some(job) = self.lock_job.take()
                        && let Ok(res) = job.rx.recv_timeout(Duration::from_secs(5))
                    {
                        self.finish_lock_job(res, tx);
                        return true;
                    }
                    return true;
                }
                tx.wake_after(Duration::from_millis(16));
                false
            }
        }
    }

    /// Waits for an in-flight background lock/unlock job to finish (for synchronous unit tests).
    #[cfg(test)]
    pub fn wait_lock(&mut self, tx: &mut Tx) {
        if let Some(job) = self.lock_job.take()
            && let Ok(res) = job.rx.recv_timeout(Duration::from_secs(10))
        {
            self.finish_lock_job(res, tx);
        }
    }

    fn finish_lock_job(&mut self, res: LockResult, tx: &mut Tx) {
        match res {
            LockResult::Locked { id, rel, text, key, changing } => {
                if id != self.lock_job_seq || !self.files().contains_key(&rel) {
                    return;
                }
                if changing && !self.note_unlocked(&rel) {
                    return;
                }
                if matches!(self.layer(), Some(Layer::Lock(LockDialog::Set(_) | LockDialog::Change(_)))) {
                    self.close_layer(tx);
                }
                self.fields.remove("lock-pass");
                self.fields.remove("lock-repeat");
                self.reveal.remove("lock-pass");
                self.reveal.remove("lock-repeat");
                // Seal directly with the new key so neither `vault.files` nor disk ever sees plain text.
                let sealed = lock::seal(&key, &text);
                if let Some(v) = &mut self.vault {
                    v.put(rel.clone(), sealed);
                }
                self.unlocked.insert(rel.clone(), Unlocked::new(key, text));
                self.mark_dirty_pub(&rel);
                self.flush_notes();
                if changing {
                    self.toast("Password changed", tx);
                }
                self.touch_activity();
                self.focus_editor(tx);
            }
            LockResult::Unlocked { id, rel, res } => {
                if id != self.lock_job_seq {
                    return;
                }
                let is_active = self.active.as_deref() == Some(rel.as_str());
                match res {
                    Ok((plain, key)) => {
                        if is_active {
                            self.unlock_error = None;
                            self.fields.remove("unlock-pass");
                            self.reveal.remove("unlock-pass");
                        }
                        self.edits.remove(&rel);
                        self.unlocked.insert(rel, Unlocked::new(key, plain));
                        self.rebuild_names_pub();
                        self.touch_activity();
                        if is_active {
                            if matches!(self.mode, Mode::Map | Mode::Board) {
                                self.keys_to_note(tx);
                            } else {
                                self.focus_editor(tx);
                            }
                        }
                    }
                    Err(e) => {
                        if is_active {
                            self.unlock_error = Some(e);
                            // Select the wrong password so the user can immediately type a fresh one.
                            if let Some(ed) = self.fields.get_mut(crate::ui::lock::PASS) {
                                let n = ed.text.len();
                                ed.set_selection(0, n);
                                ed.focused = true;
                            }
                            tx.focus(crate::ui::lock::PASS, false);
                        }
                    }
                }
            }
        }
    }

    /// Re-locks every unlocked note once inactivity exceeds `settings.auto_lock_mins`.
    pub(crate) fn poll_auto_lock(&mut self, t: f64, tx: &mut Tx) -> bool {
        let mins = self.settings().auto_lock_mins;
        if mins == 0 || (self.unlocked.is_empty() && !self.parked_has_unlocked()) {
            return false;
        }
        let limit = (mins as f64) * 60.0;
        let elapsed = (t - self.last_activity.get()).max(0.0);
        if elapsed >= limit {
            self.lock_all_notes(tx);
            self.toast("Locked notes after inactivity", tx);
            return true;
        }
        tx.wake_after(Duration::from_secs_f64((limit - elapsed).max(0.5)));
        false
    }

    /// Opens the confirmation dialog before removing the active note's password.
    pub fn ask_remove_password(&mut self, tx: &mut Tx) {
        let Some(a) = self.active.as_deref() else { return };
        if !self.note_unlocked(a) {
            return;
        }
        self.open_layer(Layer::Lock(LockDialog::ConfirmRemove), tx);
        tx.focus("remove-pass-cancel", tx.by_keys());
    }

    /// Confirms removing the password from the confirmation dialog.
    pub fn confirm_remove_password(&mut self, tx: &mut Tx) {
        self.close_layer(tx);
        self.remove_password(tx);
        self.toast("Password removed", tx);
    }

    /// Saves the active unlocked note as plain text again.
    pub fn remove_password(&mut self, tx: &mut Tx) {
        let Some(a) = self.active.clone() else { return };
        let Some(u) = self.unlocked.remove(&a) else { return };
        if let Some(v) = &mut self.vault {
            v.put(a.clone(), u.text);
        }
        self.mark_dirty_pub(&a);
        self.flush_notes();
        self.rebuild_names_pub();
        if self.mode == Mode::Source {
            self.focus_editor(tx);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn idle_auto_lock_locks_all_notes_after_timeout() {
        let dir = std::env::temp_dir().join(format!("quire-autolock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("secret.md"), "# Secret\n").unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(dir.clone(), tx);
            s.open_file("secret.md", None, tx);
            s.settings_mut().auto_lock_mins = 5;
            s.now.set(10.0);
            s.ask_lock(tx);
            s.field("lock-pass", String::new).replace_text("tiger lily");
            s.field("lock-repeat", String::new).replace_text("tiger lily");
            s.lock_note(tx);
            s.wait_lock(tx);
            assert!(s.note_unlocked("secret.md"));
            assert!(!s.poll_auto_lock(10.0 + 299.0, tx), "stays unlocked before timeout");
            assert!(s.note_unlocked("secret.md"));
            assert!(s.poll_auto_lock(10.0 + 300.0, tx), "locks at 5-minute threshold");
            assert!(s.unlocked.is_empty());
            assert!(s.note_locked("secret.md"));
            assert!(s.reader.toast.as_ref().is_some_and(|(m, _)| m.contains("inactivity")));
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancelling_in_flight_lock_aborts_encryption() {
        let dir = std::env::temp_dir().join(format!("quire-lock-cancel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plain = "# Secret\n\nKeep me plain.\n";
        std::fs::write(dir.join("secret.md"), plain).unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(dir.clone(), tx);
            s.open_file("secret.md", None, tx);
            s.ask_lock(tx);
            s.field("lock-pass", String::new).replace_text("tiger lily");
            s.field("lock-repeat", String::new).replace_text("tiger lily");
            s.lock_note(tx);
            assert_eq!(s.lock_busy(), Some(super::LockJobKind::Locking));
            s.cancel_lock(tx);
            assert_eq!(s.lock_busy(), None, "cancelling must clear lock_job immediately");
            assert!(s.layer().is_none(), "lock dialog should close");
        });
        // Give the detached worker time to finish and ensure polling ignores any late work.
        std::thread::sleep(std::time::Duration::from_millis(600));
        rt.update(|s, tx| {
            assert!(!s.poll_lock(10.0, tx));
            s.flush_all();
            assert!(!s.is_locked("secret.md"), "cancelled lock must not mark note locked");
            assert!(!s.note_unlocked("secret.md"), "cancelled lock must not register unlocked key");
        });
        let on_disk = std::fs::read_to_string(dir.join("secret.md")).unwrap();
        assert_eq!(on_disk, plain, "cancelled lock must leave note plain text on disk");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pointer_press_and_scroll_update_last_activity() {
        let dir = std::env::temp_dir().join(format!("quire-activity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut body = String::from("# Long Note\n\n");
        for i in 0..80 {
            body.push_str(&format!("Paragraph {i} with enough lines to scroll comfortably.\n\n"));
        }
        std::fs::write(dir.join("long.md"), &body).unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(dir.clone(), tx);
            s.open_file("long.md", None, tx);
            s.mode = crate::app::Mode::Preview;
        });
        rt.frame(0.0);
        // Advance clock to 50s and click inside Preview document -> last_activity updates.
        rt.frame(50.0);
        rt.update(|s, _| s.now.set(50.0));
        rt.pointer_move(500.0, 400.0);
        rt.pointer_down(500.0, 400.0);
        rt.pointer_up(500.0, 400.0);
        rt.frame(0.016);
        let after_click = rt.state.last_activity.get();
        assert!(after_click >= 50.0, "pointer press should update last_activity (got {after_click})");

        // Advance clock to 120s and wheel-scroll document -> last_activity updates without keyboard input.
        rt.frame(70.0);
        rt.update(|s, _| s.now.set(120.0));
        rt.pointer_move(640.0, 400.0);
        rt.wheel(640.0, 400.0, -240.0);
        rt.frame(0.016);
        let after_scroll = rt.state.last_activity.get();
        assert!(after_scroll > after_click, "wheel scroll should advance last_activity (before={after_click}, after={after_scroll})");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parked_note_mode_unlocked_notes_lock_on_lock_all_and_auto_lock() {
        let dir = std::env::temp_dir().join(format!("quire-park-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let key = crate::lock::new_key("parked secret");
        let sealed = crate::lock::seal(&key, "# Parked Secret\nHidden plaintext.\n");
        let note_a = dir.join("SecretA.md");
        let note_b = dir.join("PlainB.md");
        std::fs::write(&note_a, &sealed).unwrap();
        std::fs::write(&note_b, "# Plain B\n").unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.settings_mut().auto_lock_mins = 5;
            s.now.set(10.0);
            assert!(s.open_note(&note_a));
            s.field(crate::ui::lock::PASS, String::new).replace_text("parked secret");
            s.unlock(tx);
            s.wait_lock(tx);
            assert!(s.note_unlocked("SecretA.md"));

            // Switch to PlainB.md in Note mode so SecretA.md is parked while unlocked.
            assert!(s.open_note(&note_b));
            assert!(s.unlocked.is_empty(), "active vault has no unlocked note");
            assert!(s.parked_has_unlocked(), "parked Note-mode note must be tracked as unlocked");

            // Idle auto-lock must detect and lock parked Note-mode notes.
            assert!(s.poll_auto_lock(10.0 + 300.0, tx));
            assert!(!s.parked_has_unlocked(), "auto-lock must clear parked unlocked entries");

            // Switching back to SecretA.md must find it locked again.
            assert!(s.open_note(&note_a));
            assert!(s.note_locked("SecretA.md"));
            assert!(!s.note_unlocked("SecretA.md"));

            // Re-unlock, park again, and verify explicit lock_all_notes also clears parked state.
            s.field(crate::ui::lock::PASS, String::new).replace_text("parked secret");
            s.unlock(tx);
            s.wait_lock(tx);
            assert!(s.open_note(&note_b));
            assert!(s.parked_has_unlocked());
            s.lock_all_notes(tx);
            assert!(!s.parked_has_unlocked());
            assert!(s.open_note(&note_a));
            assert!(s.note_locked("SecretA.md"));
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn change_password_reseals_in_place_without_plaintext_on_disk() {
        let dir = std::env::temp_dir().join(format!("quire-change-pass-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plain = "# Secret\nTop secret line.\n";
        std::fs::write(dir.join("Secret.md"), plain).unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(dir.clone(), tx);
            s.open_file("Secret.md", None, tx);
            s.ask_lock(tx);
            s.field("lock-pass", String::new).replace_text("old password");
            s.field("lock-repeat", String::new).replace_text("old password");
            s.lock_note(tx);
            s.wait_lock(tx);
            assert!(s.note_unlocked("Secret.md"));

            s.ask_change_password(tx);
            assert!(matches!(s.layer(), Some(crate::layer::Layer::Lock(crate::layer::LockDialog::Change(None)))));
            s.field("lock-pass", String::new).replace_text("new password");
            s.field("lock-repeat", String::new).replace_text("new password");
            s.lock_note(tx);
            s.wait_lock(tx);
            s.flush_all();

            assert!(s.note_unlocked("Secret.md"));
            assert!(s.reader.toast.as_ref().is_some_and(|(m, _)| m == "Password changed"));
            let stored = s.files().get("Secret.md").cloned().unwrap_or_default();
            assert!(crate::lock::is_locked(&stored), "vault copy must stay encrypted");
            assert!(!stored.contains("Top secret line"), "vault copy must never contain plaintext");
            assert!(crate::lock::open(&stored, "old password").is_err(), "old password must no longer open note");
            assert_eq!(crate::lock::open(&stored, "new password").unwrap().0, plain);
        });
        let on_disk = std::fs::read_to_string(dir.join("Secret.md")).unwrap();
        assert!(crate::lock::is_locked(&on_disk), "disk file must stay encrypted");
        assert!(!on_disk.contains("Top secret line"));
        assert_eq!(crate::lock::open(&on_disk, "new password").unwrap().0, plain);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lock_all_notes_focuses_unlock_pass_when_active_locked() {
        let dir = std::env::temp_dir().join(format!("quire-lock-all-focus-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Secret.md"), "# Secret\n").unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(dir.clone(), tx);
            s.open_file("Secret.md", None, tx);
            s.ask_lock(tx);
            s.field("lock-pass", String::new).replace_text("tiger lily");
            s.field("lock-repeat", String::new).replace_text("tiger lily");
            s.lock_note(tx);
            s.wait_lock(tx);
            s.mode = crate::app::Mode::Preview;
        });
        rt.frame(0.016);
        rt.update(|s, tx| {
            s.lock_all_notes(tx);
        });
        rt.frame(0.032);
        let focused = rt.elements().into_iter().find(|e| e.focused && !e.exiting).and_then(|e| e.key);
        assert_eq!(focused.as_deref(), Some(crate::ui::lock::PASS), "lock_all_notes must focus unlock-pass");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_unlock_selects_password_and_retyping_clears_error() {
        let dir = std::env::temp_dir().join(format!("quire-unlock-err-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let key = crate::lock::new_key("right password");
        let sealed = crate::lock::seal(&key, "# Locked\n");
        std::fs::write(dir.join("Secret.md"), &sealed).unwrap();

        let app = crate::app::init();
        let mut rt = rune::Runtime::new(app, crate::ui::view, (1280.0, 800.0), rune::Env { system_dark: false, scale: 1.0 });
        rt.update(|s, tx| {
            s.open_vault(dir.clone(), tx);
            s.open_file("Secret.md", None, tx);
            s.field(crate::ui::lock::PASS, String::new).replace_text("wrong");
            s.unlock(tx);
            s.wait_lock(tx);
            assert!(s.unlock_error.is_some());
            let ed = s.fields.get(crate::ui::lock::PASS).expect("unlock-pass field");
            assert_eq!((ed.anchor, ed.focus), (0, 5), "wrong password should be fully selected");
        });
        rt.frame(0.016);
        // Typing a replacement character into unlock-pass clears the stale unlock error.
        rt.key(rune::KeyEvent { key: rune::Key::Text("r".into()), mods: rune::Mods::default() });
        rt.frame(0.032);
        assert!(rt.state.unlock_error.is_none(), "typing into unlock-pass must clear unlock_error");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
