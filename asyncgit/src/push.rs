use crate::{
    error::{Error, Result},
    sync, AsyncNotification, CWD,
};
use crossbeam_channel::{unbounded, Receiver, Sender};
use git2::PackBuilderStage;
use std::{
    cmp,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use sync::ProgressNotification;
use thread::JoinHandle;

///
#[derive(Clone, Debug)]
pub enum PushProgressState {
    ///
    PackingAddingObject,
    ///
    PackingDeltafiction,
    ///
    Pushing,
}

///
#[derive(Clone, Debug)]
pub struct PushProgress {
    ///
    pub state: PushProgressState,
    ///
    pub progress: u8,
}

impl PushProgress {
    ///
    pub fn new(
        state: PushProgressState,
        current: usize,
        total: usize,
    ) -> Self {
        let total = cmp::max(current, total) as f32;
        let progress = current as f32 / total * 100.0;
        let progress = progress as u8;
        Self { state, progress }
    }
}

impl From<ProgressNotification> for PushProgress {
    fn from(progress: ProgressNotification) -> Self {
        match progress {
            ProgressNotification::Packing {
                stage,
                current,
                total,
            } => match stage {
                PackBuilderStage::AddingObjects => PushProgress::new(
                    PushProgressState::PackingAddingObject,
                    current,
                    total,
                ),
                PackBuilderStage::Deltafication => PushProgress::new(
                    PushProgressState::PackingDeltafiction,
                    current,
                    total,
                ),
            },
            ProgressNotification::PushTransfer {
                current,
                total,
                ..
            } => PushProgress::new(
                PushProgressState::Pushing,
                current,
                total,
            ),
            //ProgressNotification::Done |
            _ => PushProgress::new(PushProgressState::Pushing, 1, 1),
        }
    }
}

///
#[derive(Default, Clone, Debug)]
pub struct PushRequest {
    ///
    pub remote: String,
    ///
    pub branch: String,
}

#[derive(Default, Clone, Debug)]
struct PushState {
    request: PushRequest,
}

///
pub struct AsyncPush {
    state: Arc<Mutex<Option<PushState>>>,
    last_result: Arc<Mutex<Option<String>>>,
    progress: Arc<Mutex<Option<ProgressNotification>>>,
    sender: Sender<AsyncNotification>,
}

impl AsyncPush {
    ///
    pub fn new(sender: &Sender<AsyncNotification>) -> Self {
        Self {
            state: Arc::new(Mutex::new(None)),
            last_result: Arc::new(Mutex::new(None)),
            progress: Arc::new(Mutex::new(None)),
            sender: sender.clone(),
        }
    }

    ///
    pub fn is_pending(&self) -> Result<bool> {
        let state = self.state.lock()?;
        Ok(state.is_some())
    }

    ///
    pub fn last_result(&self) -> Result<Option<String>> {
        let res = self.last_result.lock()?;
        Ok(res.clone())
    }

    ///
    pub fn progress(&self) -> Result<Option<PushProgress>> {
        let res = self.progress.lock()?;
        Ok(res.as_ref().map(|progress| progress.clone().into()))
    }

    ///
    pub fn request(&mut self, params: PushRequest) -> Result<()> {
        log::trace!("request");

        if self.is_pending()? {
            return Ok(());
        }

        self.set_request(&params)?;
        Self::set_progress(self.progress.clone(), None)?;

        let arc_state = Arc::clone(&self.state);
        let arc_res = Arc::clone(&self.last_result);
        let arc_progress = Arc::clone(&self.progress);
        let sender = self.sender.clone();

        thread::spawn(move || {
            let (progress_sender, receiver) = unbounded();

            let handle = Self::spawn_receiver_thread(
                sender.clone(),
                receiver,
                arc_progress,
            );

            let res = sync::push(
                CWD,
                params.remote.as_str(),
                params.branch.as_str(),
                progress_sender.clone(),
            );

            progress_sender
                .send(ProgressNotification::Done)
                .expect("closing send failed");

            handle.join().expect("joining thread failed");

            Self::set_result(arc_res, res).expect("result error");

            Self::clear_request(arc_state).expect("clear error");

            sender
                .send(AsyncNotification::Push)
                .expect("error sending push");
        });

        Ok(())
    }

    fn spawn_receiver_thread(
        sender: Sender<AsyncNotification>,
        receiver: Receiver<ProgressNotification>,
        progress: Arc<Mutex<Option<ProgressNotification>>>,
    ) -> JoinHandle<()> {
        log::info!("push progress receiver spawned");

        thread::spawn(move || loop {
            let incoming = receiver.recv();
            match incoming {
                Ok(update) => {
                    Self::set_progress(
                        progress.clone(),
                        Some(update.clone()),
                    )
                    .expect("set prgoress failed");
                    sender
                        .send(AsyncNotification::Push)
                        .expect("error sending push");

                    //NOTE: for better debugging
                    thread::sleep(Duration::from_millis(300));

                    if let ProgressNotification::Done = update {
                        break;
                    }
                }
                Err(e) => {
                    log::error!(
                        "push progress receiver error: {}",
                        e
                    );
                    break;
                }
            }
        })
    }

    fn set_request(&self, params: &PushRequest) -> Result<()> {
        let mut state = self.state.lock()?;

        if state.is_some() {
            return Err(Error::Generic("pending request".into()));
        }

        *state = Some(PushState {
            request: params.clone(),
        });

        Ok(())
    }

    fn clear_request(
        state: Arc<Mutex<Option<PushState>>>,
    ) -> Result<()> {
        let mut state = state.lock()?;

        *state = None;

        Ok(())
    }

    fn set_progress(
        progress: Arc<Mutex<Option<ProgressNotification>>>,
        state: Option<ProgressNotification>,
    ) -> Result<()> {
        let simple_progress: Option<PushProgress> =
            state.as_ref().map(|prog| prog.clone().into());
        log::info!("push progress: {:?}", simple_progress);
        let mut progress = progress.lock()?;

        *progress = state;

        Ok(())
    }

    fn set_result(
        arc_result: Arc<Mutex<Option<String>>>,
        res: Result<()>,
    ) -> Result<()> {
        let mut last_res = arc_result.lock()?;

        *last_res = match res {
            Ok(_) => None,
            Err(e) => {
                log::error!("push error: {}", e);
                Some(e.to_string())
            }
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_zero_total() {
        let prog =
            PushProgress::new(PushProgressState::Pushing, 1, 0);

        assert_eq!(prog.progress, 100);
    }

    #[test]
    fn test_progress_rounding() {
        let prog =
            PushProgress::new(PushProgressState::Pushing, 2, 10);

        assert_eq!(prog.progress, 20);
    }
}
