use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum WorkerState {
    Runnable,
    WaitingMessage,
    Sleeping,
    Halted,
    Error(VmError),
}

/// Cooperative scheduler for VM workers.
pub struct Scheduler {
    workers: BTreeMap<WorkerId, Vm>,
    runnable: VecDeque<WorkerId>,
    waiting: BTreeSet<WorkerId>,
    sleeping: BTreeSet<WorkerId>,
    halted: BTreeSet<WorkerId>,
    errors: BTreeMap<WorkerId, VmError>,
    next_worker_id: WorkerId,
}

/// Serializable snapshot of the scheduler and all workers.
#[derive(Clone, Debug, PartialEq)]
pub struct SchedulerSnapshot {
    pub workers: BTreeMap<WorkerId, VmSnapshot>,
    pub runnable: VecDeque<WorkerId>,
    pub waiting: BTreeSet<WorkerId>,
    pub sleeping: BTreeSet<WorkerId>,
    pub halted: BTreeSet<WorkerId>,
    pub errors: BTreeMap<WorkerId, VmError>,
    pub next_worker_id: WorkerId,
}

impl Scheduler {
    /// Creates an empty scheduler.
    pub fn new() -> Self {
        Self {
            workers: BTreeMap::new(),
            runnable: VecDeque::new(),
            waiting: BTreeSet::new(),
            sleeping: BTreeSet::new(),
            halted: BTreeSet::new(),
            errors: BTreeMap::new(),
            next_worker_id: 1,
        }
    }

    /// Spawns a worker VM and returns its id.
    pub fn spawn(&mut self, mut vm: Vm) -> WorkerId {
        let worker_id = self.next_worker_id;
        self.next_worker_id = self.next_worker_id.saturating_add(1);
        vm.set_worker_id(worker_id);
        self.workers.insert(worker_id, vm);
        self.runnable.push_back(worker_id);
        worker_id
    }

    /// Returns a worker VM by id.
    pub fn worker(&self, worker_id: WorkerId) -> Option<&Vm> {
        self.workers.get(&worker_id)
    }

    /// Returns a mutable worker VM by id.
    pub fn worker_mut(&mut self, worker_id: WorkerId) -> Option<&mut Vm> {
        self.workers.get_mut(&worker_id)
    }

    /// Returns all worker ids currently known to the scheduler.
    pub fn worker_ids(&self) -> impl Iterator<Item = WorkerId> + '_ {
        self.workers.keys().copied()
    }

    /// Returns a serializable snapshot of the scheduler and all workers.
    pub fn snapshot(&self) -> SchedulerSnapshot {
        SchedulerSnapshot {
            workers: self
                .workers
                .iter()
                .map(|(worker_id, vm)| (*worker_id, vm.snapshot()))
                .collect(),
            runnable: self.runnable.clone(),
            waiting: self.waiting.clone(),
            sleeping: self.sleeping.clone(),
            halted: self.halted.clone(),
            errors: self.errors.clone(),
            next_worker_id: self.next_worker_id,
        }
    }

    /// Restores a scheduler from a snapshot.
    pub fn from_snapshot(
        snapshot: SchedulerSnapshot,
        mut host_api_factory: impl FnMut(&VmConfig) -> Box<dyn HostApi>,
    ) -> Self {
        let workers = snapshot
            .workers
            .into_iter()
            .map(|(worker_id, vm_snapshot)| {
                let host_api = host_api_factory(&vm_snapshot.config);
                (worker_id, Vm::from_snapshot(vm_snapshot, host_api))
            })
            .collect();
        Self {
            workers,
            runnable: snapshot.runnable,
            waiting: snapshot.waiting,
            sleeping: snapshot.sleeping,
            halted: snapshot.halted,
            errors: snapshot.errors,
            next_worker_id: snapshot.next_worker_id,
        }
    }

    /// Returns the current state of a worker.
    pub fn worker_state(&self, worker_id: WorkerId) -> Option<WorkerState> {
        self.workers.get(&worker_id).map(|vm| match vm.state() {
            VmState::Idle | VmState::Running => {
                if self.runnable.contains(&worker_id) {
                    WorkerState::Runnable
                } else {
                    WorkerState::Runnable
                }
            }
            VmState::WaitingMessage => WorkerState::WaitingMessage,
            VmState::Sleeping => WorkerState::Sleeping,
            VmState::Halted => WorkerState::Halted,
            VmState::Error(error) => WorkerState::Error(error.clone()),
        })
    }

    /// Wakes a worker that is waiting or sleeping.
    pub fn wake(&mut self, worker_id: WorkerId) -> bool {
        self.waiting.remove(&worker_id);
        self.sleeping.remove(&worker_id);
        self.halted.remove(&worker_id);
        self.errors.remove(&worker_id);
        let Some(worker) = self.workers.get_mut(&worker_id) else {
            return false;
        };
        worker.wake();
        if !self.runnable.contains(&worker_id) {
            self.runnable.push_back(worker_id);
        }
        true
    }

    /// Delivers a message to the target worker.
    pub fn deliver(&mut self, message: Message) {
        if let Some(worker) = self.workers.get_mut(&message.to) {
            worker.push_message(message);
            if self.waiting.remove(&worker.worker_id()) {
                self.runnable.push_back(worker.worker_id());
            }
        }
    }

    /// Runs one scheduling round over the currently runnable workers.
    pub fn run_round(&mut self, step_limit: usize) -> Vec<(WorkerId, RunOutcome)> {
        let mut outcomes = Vec::new();
        let runnable_count = self.runnable.len();
        for _ in 0..runnable_count {
            let Some(worker_id) = self.runnable.pop_front() else {
                break;
            };
            let Some(vm) = self.workers.get_mut(&worker_id) else {
                continue;
            };

            let outcome = vm.run_frame(step_limit);
            self.route_outbox(worker_id);
            self.reconcile(worker_id, &outcome);
            outcomes.push((worker_id, outcome));
        }
        outcomes
    }

    fn reconcile(&mut self, worker_id: WorkerId, outcome: &RunOutcome) {
        self.waiting.remove(&worker_id);
        self.sleeping.remove(&worker_id);
        self.halted.remove(&worker_id);
        self.errors.remove(&worker_id);

        match outcome {
            RunOutcome::StepLimitReached { .. } | RunOutcome::Yielded { .. } => {
                self.runnable.push_back(worker_id);
            }
            RunOutcome::WaitingMessage { .. } => {
                self.waiting.insert(worker_id);
            }
            RunOutcome::Sleeping { .. } => {
                self.sleeping.insert(worker_id);
            }
            RunOutcome::Halted { .. } => {
                self.halted.insert(worker_id);
            }
            RunOutcome::Error { error, .. } => {
                self.errors.insert(worker_id, error.clone());
            }
        }
    }

    fn route_outbox(&mut self, worker_id: WorkerId) {
        let messages = self
            .workers
            .get_mut(&worker_id)
            .map(Vm::drain_outbox)
            .unwrap_or_default();
        for message in messages {
            self.deliver(message);
        }
    }

    /// Returns `true` when no worker is runnable.
    pub fn is_idle(&self) -> bool {
        self.runnable.is_empty()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
